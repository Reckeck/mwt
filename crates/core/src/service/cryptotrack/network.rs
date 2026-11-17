//! The endpoints of each network, kept on this device (spec E17).
//!
//! What the user may set per network: their own RPC node, their own
//! Blockscout instance, their own explorer, and an optional Blockscout key
//! (decision R8 of 2026-11-12). All of it lives in the device secrets file
//! (`vault_io::device_secrets`), so none of it is in the vault document, in a
//! backup, in a sync batch or in the JSON editor. Empty means the preset of
//! `cryptotrack::chains`.
//!
//! Rules this file keeps:
//!
//! - `https` only. The single exception is `http` to `localhost`, `127.0.0.1`
//!   or `[::1]`, for the user's own node. The check runs on save and again on
//!   every use, so a value that got into the file some other way is ignored.
//! - The Blockscout key is never returned to the screen, never logged and
//!   never part of an error; it travels in an `x-api-key` header over
//!   `https` only (`RpcClient::with_api_key`).
//! - The connection test is a network call, so it needs the same consent as
//!   the background tracking and does nothing without it.
//! - Changing the settings needs protection level 3 (where the device file
//!   lives); below it the presets are shown and used.

use std::future::Future;
use std::time::{Duration, Instant};

use reqwest::Url;
use serde_json::{Value, json};

use super::super::UnlockedVault;
use crate::cryptotrack::chains::{self, EvmChain};
use crate::cryptotrack::rpc::{RpcClient, RpcConfig};
use crate::cryptotrack::tracking::{ExplorerSettings, normalize_explorer};
use crate::error::{Error, Result};
use crate::ipc_contract::cryptotrack::network::{
    EndpointKind, EndpointView, NetworkSettings, NetworkTestOutcome, NetworkTestResult,
    NetworkView, SetNetworkSettingsInput, TestNetworkInput,
};
use crate::model::chain::Chain;
use crate::vault_io::device_secrets::{self, BlockscoutKey, DeviceSecrets, NetworkEndpoints};

/// What a reader of a network uses; `None` is the preset.
#[derive(Debug, Default)]
pub struct NetworkRequest {
    pub rpc: Option<String>,
    pub blockscout: Option<String>,
    pub key: Option<BlockscoutKey>,
}

// -- the address rules --------------------------------------------------------

fn is_own_node(url: &Url) -> bool {
    let Some(host) = url.host_str() else {
        return false;
    };
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    host.trim_matches(['[', ']'])
        .parse::<std::net::IpAddr>()
        .is_ok_and(|ip| ip.is_loopback())
}

/// An address of a node or of a Blockscout instance. No echo of the text in
/// any error: a node URL can carry a provider key.
fn normalize_endpoint(raw: &str, allow_query: bool) -> Result<String> {
    let text = raw.trim();
    let reject = |why: &str| Err(Error::InvalidInput(format!("network address: {why}")));
    if text.is_empty() {
        return reject("it is empty");
    }
    if text
        .chars()
        .any(|ch| ch.is_control() || ch.is_whitespace() || ch == '\\')
    {
        return reject("it contains whitespace or control characters");
    }
    let Ok(url) = Url::parse(text) else {
        return reject("it is not a valid address");
    };
    match url.scheme() {
        "https" => {}
        "http" if is_own_node(&url) => {}
        _ => return reject("only https:// is accepted (http:// only for localhost)"),
    }
    if url.host().is_none() {
        return reject("the host is missing");
    }
    if !url.username().is_empty() || url.password().is_some() {
        return reject("it must not carry a user name or password");
    }
    if url.fragment().is_some() {
        return reject("a fragment (#) is not allowed");
    }
    if !allow_query && url.query().is_some() {
        return reject("a query (?) is not allowed");
    }
    let mut out = url.as_str().to_owned();
    if url.query().is_none() {
        out = out.trim_end_matches('/').to_owned();
    }
    Ok(out)
}

/// The address of a JSON-RPC node.
pub fn normalize_rpc(raw: &str) -> Result<String> {
    normalize_endpoint(raw, true)
}

/// The base address of a Blockscout instance (the API is `<base>/api`).
pub fn normalize_blockscout(raw: &str) -> Result<String> {
    normalize_endpoint(raw, false)
}

/// Whether two typed addresses mean the same Blockscout instance. Both are
/// normalized first, so a trailing slash or a different case of the host does
/// not read as a different instance; anything unparsable is "not the same",
/// because a key must never go out on a guess.
fn same_instance(left: &str, right: &str) -> bool {
    match (normalize_blockscout(left), normalize_blockscout(right)) {
        (Ok(one), Ok(two)) => one == two,
        _ => false,
    }
}

/// Only the host and port of an address.
fn host_of(url: &str) -> String {
    Url::parse(url)
        .ok()
        .and_then(|parsed| {
            parsed.host_str().map(|host| match parsed.port() {
                Some(port) => format!("{host}:{port}"),
                None => host.to_owned(),
            })
        })
        .unwrap_or_default()
}

// -- the views ----------------------------------------------------------------

fn view(custom: Option<String>, presets: &[&str]) -> EndpointView {
    let value = custom
        .clone()
        .or_else(|| presets.first().map(|first| (*first).to_owned()))
        .unwrap_or_default();
    EndpointView {
        host: host_of(&value),
        custom: custom.is_some(),
        presets: presets.iter().map(|preset| (*preset).to_owned()).collect(),
        value,
    }
}

fn network_view(row: &EvmChain, stored: Option<&NetworkEndpoints>) -> NetworkView {
    // Stored values are checked again: a bad one is ignored, never used.
    let rpc = stored
        .and_then(|entry| entry.rpc.as_deref())
        .and_then(|url| normalize_rpc(url).ok());
    let blockscout = stored
        .and_then(|entry| entry.blockscout.as_deref())
        .and_then(|url| normalize_blockscout(url).ok());
    let explorer = stored
        .and_then(|entry| entry.explorer.as_deref())
        .and_then(|url| normalize_explorer(url).ok());
    NetworkView {
        chain: row.chain,
        label: row.chain.label().to_owned(),
        chain_id: row.chain_id,
        rpc: view(rpc, row.rpc),
        blockscout: view(blockscout, row.blockscout),
        explorer: view(explorer, &[row.explorer]),
        blockscout_key_set: stored.is_some_and(|entry| entry.blockscout_key.is_some()),
    }
}

/// An empty text is "the preset"; anything else must pass `check`.
fn optional(raw: &str, check: fn(&str) -> Result<String>) -> Result<Option<String>> {
    if raw.trim().is_empty() {
        Ok(None)
    } else {
        check(raw).map(Some)
    }
}

// -- the vault ----------------------------------------------------------------

impl UnlockedVault {
    /// The secrets of the networks, or none below level 3 (the presets then).
    fn network_secrets(&self) -> Result<DeviceSecrets> {
        if !self.keys_on_device() {
            return Ok(DeviceSecrets::default());
        }
        self.device_secrets_for_request()
    }

    /// The endpoints of every network and whether they can be changed here.
    pub fn crypto_network_settings(&self) -> NetworkSettings {
        // A damaged file shows the presets, read-only; saving is refused
        // (`crypto_set_network_settings`) so it is never overwritten blindly.
        let secrets = self.network_secrets();
        let editable = self.keys_on_device() && secrets.is_ok();
        let secrets = secrets.unwrap_or_default();
        NetworkSettings {
            editable,
            networks: chains::all()
                .iter()
                .map(|row| network_view(row, secrets.network_for(row.chain)))
                .collect(),
            consent_at: None,
            consent_device: None,
        }
    }

    /// Save one network. Everything is checked before anything is written.
    pub fn crypto_set_network_settings(&mut self, input: &SetNetworkSettingsInput) -> Result<()> {
        if self.sandbox {
            return Err(Error::Sandbox(
                "network settings are not kept in a sandbox".into(),
            ));
        }
        if !self.keys_on_device() {
            return Err(Error::SecurityLevelRequired);
        }
        chains::evm_chain(input.chain).ok_or(Error::Unsupported)?;
        let rpc = optional(&input.rpc, normalize_rpc)?;
        let blockscout = optional(&input.blockscout, normalize_blockscout)?;
        let explorer = optional(&input.explorer, normalize_explorer)?;
        let key_change = match input.blockscout_key.as_deref() {
            None => None,
            Some(text) if text.trim().is_empty() => Some(None),
            Some(text) => Some(Some(BlockscoutKey::parse(text)?)),
        };
        // A damaged file is `Secret`: it is not replaced behind the user's back.
        let mut secrets = self.device_secrets_for_request()?;
        let stored = secrets.network_for(input.chain);
        let kept_key = stored.and_then(|entry| entry.blockscout_key.clone());
        let kept_blockscout = stored.and_then(|entry| entry.blockscout.clone());
        // A key is issued by one instance for itself. When the instance changes
        // and the request brings no new key, the old key is dropped rather than
        // kept and sent to a host that never gave it out (both values are
        // already normalized, and `None` on both sides is the same preset).
        let blockscout_key = match key_change {
            Some(change) => change,
            None if blockscout == kept_blockscout => kept_key,
            None => None,
        };
        let entry = NetworkEndpoints {
            chain: input.chain,
            rpc,
            blockscout,
            explorer,
            blockscout_key,
        };
        secrets
            .crypto_networks
            .retain(|existing| existing.chain != input.chain);
        if !entry.is_blank() {
            secrets.crypto_networks.push(entry);
        }
        let key = self
            .key
            .clone()
            .ok_or_else(|| Error::Crypto("the vault has no key".into()))?;
        device_secrets::save(&self.path, &key, &secrets)
    }

    /// What the reader of `chain` uses for one request. Never fails: a vault
    /// below level 3, a damaged file or a bad value mean the presets and no key.
    pub fn crypto_network_for_request(&self, chain: Chain) -> NetworkRequest {
        let secrets = self.network_secrets().unwrap_or_default();
        let Some(entry) = secrets.network_for(chain) else {
            return NetworkRequest::default();
        };
        NetworkRequest {
            rpc: entry.rpc.as_deref().and_then(|url| normalize_rpc(url).ok()),
            blockscout: entry
                .blockscout
                .as_deref()
                .and_then(|url| normalize_blockscout(url).ok()),
            key: entry.blockscout_key.clone(),
        }
    }

    /// The user's own explorers, for the links of an operation. Bad values are
    /// dropped here and checked again when a link is built.
    pub fn crypto_explorer_settings(&self) -> ExplorerSettings {
        let mut out = ExplorerSettings::default();
        for entry in self.network_secrets().unwrap_or_default().crypto_networks {
            if let Some(url) = entry.explorer.as_deref() {
                let _ = out.set(entry.chain, url);
            }
        }
        out
    }

    /// What a connection test sends: the address typed or in use, and the key
    /// typed or stored.
    pub fn crypto_test_target(
        &self,
        input: &TestNetworkInput,
    ) -> Result<(String, Option<BlockscoutKey>)> {
        let row = chains::evm_chain(input.chain).ok_or(Error::Unsupported)?;
        let view = network_view(row, self.network_secrets()?.network_for(input.chain));
        let instance = view.blockscout.value.clone();
        let url = match input.url.as_deref().map(str::trim) {
            Some(text) if !text.is_empty() => text.to_owned(),
            _ => match input.kind {
                EndpointKind::Rpc => view.rpc.value,
                EndpointKind::Blockscout => view.blockscout.value,
            },
        };
        if url.is_empty() {
            return Err(Error::InvalidInput(
                "this network has no Blockscout instance to test".into(),
            ));
        }
        // The key typed into this very request is tested against the address of
        // the same request — that is what the user asked for. The stored key is
        // issued by the instance in use and goes out only when the test really
        // goes to that instance: trying a new address must not hand it the key
        // of the old one, and a node is never sent a Blockscout key at all.
        let key = match input.blockscout_key.as_deref() {
            Some(text) if !text.trim().is_empty() => Some(BlockscoutKey::parse(text)?),
            _ if matches!(input.kind, EndpointKind::Blockscout)
                && same_instance(&url, &instance) =>
            {
                self.network_secrets()?
                    .network_for(input.chain)
                    .and_then(|entry| entry.blockscout_key.clone())
            }
            _ => None,
        };
        Ok((url, key))
    }
}

// -- the connection test --------------------------------------------------------

/// The two requests a test makes. Production: [`NetworkProbe`]; the tests use
/// a stub that counts calls.
pub trait Probe: Send + Sync {
    /// One JSON-RPC call without parameters.
    fn rpc(&self, url: &str, method: &str) -> impl Future<Output = Result<Value>> + Send;
    /// A GET on a Blockscout instance, the key in a header.
    fn get(
        &self,
        base: &str,
        path: &str,
        key: Option<&BlockscoutKey>,
    ) -> impl Future<Output = Result<Value>> + Send;
}

/// The subject a test goes out under: no address is ever sent.
const SUBJECT: &str = "network-test";

/// The production [`Probe`]: one try, a short timeout, loopback `http` allowed
/// (the user's own node).
pub struct NetworkProbe(RpcClient);

impl NetworkProbe {
    pub fn new() -> Result<Self> {
        RpcClient::new(RpcConfig {
            timeout: Duration::from_secs(8),
            max_retries: 0,
            allow_loopback_http: true,
            ..RpcConfig::default()
        })
        .map(Self)
    }
}

impl Probe for NetworkProbe {
    fn rpc(&self, url: &str, method: &str) -> impl Future<Output = Result<Value>> + Send {
        self.0.call(url, SUBJECT, method, json!([]))
    }

    async fn get(&self, base: &str, path: &str, key: Option<&BlockscoutKey>) -> Result<Value> {
        let client = self.0.with_api_key(key.map(BlockscoutKey::expose));
        client.get_json(base, path, SUBJECT).await
    }
}

fn hex_u64(value: &Value) -> Option<u64> {
    let text = value.as_str()?.strip_prefix("0x")?;
    u64::from_str_radix(text, 16).ok()
}

/// Ask one endpoint whether it answers. Without `consent` nothing is sent.
/// The text of an answer or of an error never leaves this function.
pub async fn test_endpoint<P: Probe>(
    probe: &P,
    consent: bool,
    chain: Chain,
    kind: EndpointKind,
    url: &str,
    key: Option<&BlockscoutKey>,
) -> Result<NetworkTestResult> {
    if !consent {
        return Err(Error::InvalidInput(
            "reading the addresses is turned off".into(),
        ));
    }
    let row = chains::evm_chain(chain).ok_or(Error::Unsupported)?;
    let started = Instant::now();
    let finish = |outcome, block| NetworkTestResult {
        chain,
        kind,
        outcome,
        latency_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        block,
    };
    match kind {
        EndpointKind::Rpc => {
            let url = normalize_rpc(url)?;
            let id = match probe.rpc(&url, "eth_chainId").await {
                Ok(value) => value,
                Err(_) => return Ok(finish(NetworkTestOutcome::Unreachable, None)),
            };
            let Some(id) = hex_u64(&id) else {
                return Ok(finish(NetworkTestOutcome::BadAnswer, None));
            };
            if id != row.chain_id {
                return Ok(finish(NetworkTestOutcome::WrongNetwork, None));
            }
            let block = probe
                .rpc(&url, "eth_blockNumber")
                .await
                .ok()
                .as_ref()
                .and_then(hex_u64);
            Ok(match block {
                Some(block) => finish(NetworkTestOutcome::Ok, Some(block)),
                None => finish(NetworkTestOutcome::BadAnswer, None),
            })
        }
        EndpointKind::Blockscout => {
            let base = normalize_blockscout(url)?;
            Ok(match probe.get(&base, "/api/v2/stats", key).await {
                Ok(Value::Object(_)) => finish(NetworkTestOutcome::Ok, None),
                Ok(_) => finish(NetworkTestOutcome::BadAnswer, None),
                Err(_) => finish(NetworkTestOutcome::Unreachable, None),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::super::super::test_support::empty_vault;
    use super::*;

    /// Built at run time, so no key-shaped literal sits in the source.
    fn key_text() -> String {
        ["proapi", "_", "Zx81QvLmN203"].concat()
    }

    struct Stub {
        calls: AtomicUsize,
        chain_id: &'static str,
        seen_key: Mutex<Option<String>>,
    }

    impl Stub {
        fn new(chain_id: &'static str) -> Self {
            Self {
                calls: AtomicUsize::new(0),
                chain_id,
                seen_key: Mutex::new(None),
            }
        }
    }

    impl Probe for Stub {
        async fn rpc(&self, _url: &str, method: &str) -> Result<Value> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(match method {
                "eth_chainId" => json!(self.chain_id),
                _ => json!("0x1676c1e"),
            })
        }

        async fn get(
            &self,
            _base: &str,
            _path: &str,
            key: Option<&BlockscoutKey>,
        ) -> Result<Value> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            *self.seen_key.lock().unwrap() = key.map(|found| found.expose().to_owned());
            Ok(json!({"total_blocks": "1"}))
        }
    }

    fn input(chain: Chain) -> SetNetworkSettingsInput {
        SetNetworkSettingsInput {
            chain,
            rpc: String::new(),
            blockscout: String::new(),
            explorer: String::new(),
            blockscout_key: None,
        }
    }

    fn level3() -> (tempfile::TempDir, UnlockedVault) {
        let (dir, mut vault) = empty_vault();
        vault.raise_to_level3_for_tests();
        (dir, vault)
    }

    #[test]
    fn only_https_and_localhost_are_accepted() {
        for good in [
            "https://node.example.org",
            "https://node.example.org:8545/path",
            "https://node.example.org/v1/abc?x=1",
            "http://localhost:8545",
            "http://127.0.0.1:8545",
            "http://[::1]:8545",
            "https://203.0.113.5",
        ] {
            assert!(normalize_rpc(good).is_ok(), "{good}");
        }
        for bad in [
            "http://node.example.org",
            "http://localhost.example.org",
            "ftp://node.example.org",
            "javascript:alert(1)",
            "data:text/plain,x",
            "node.example.org",
            "https://",
            "https://user:pw@node.example.org",
            "https://node.example.org/#frag",
            "https://node .example.org",
            "https://node.example.org\\x",
            "http://10.0.0.5:8545",
            "",
        ] {
            assert!(normalize_rpc(bad).is_err(), "{bad}");
        }
        // A Blockscout base takes no query.
        assert!(normalize_blockscout("https://scout.example.org/?a=1").is_err());
        assert_eq!(
            normalize_blockscout("https://scout.example.org/").unwrap(),
            "https://scout.example.org"
        );
    }

    #[test]
    fn an_error_never_echoes_the_address() {
        let secretive = "http://node.example.org/v1/SECRETPATHKEY";
        let err = normalize_rpc(secretive).unwrap_err().to_string();
        assert!(!err.contains("SECRETPATHKEY"));
    }

    #[test]
    fn below_level_3_the_presets_are_shown_and_saving_is_refused() {
        let (_dir, mut vault) = empty_vault();
        let settings = vault.crypto_network_settings();
        assert!(!settings.editable);
        assert_eq!(settings.networks.len(), chains::all().len());
        assert!(matches!(
            vault.crypto_set_network_settings(&input(Chain::Base)),
            Err(Error::SecurityLevelRequired)
        ));
    }

    #[test]
    fn saved_endpoints_are_used_and_an_empty_one_goes_back_to_the_preset() {
        let (_dir, mut vault) = level3();
        let mut save = input(Chain::Base);
        save.rpc = "http://localhost:8545".into();
        save.explorer = "https://explorer.example.org/".into();
        vault.crypto_set_network_settings(&save).unwrap();
        let settings = vault.crypto_network_settings();
        assert!(settings.editable);
        let base = settings
            .networks
            .iter()
            .find(|net| net.chain == Chain::Base)
            .unwrap();
        assert_eq!(base.rpc.value, "http://localhost:8545");
        assert!(base.rpc.custom);
        assert_eq!(base.rpc.host, "localhost:8545");
        assert_eq!(base.explorer.value, "https://explorer.example.org");
        assert!(!base.blockscout.custom);
        assert_eq!(
            vault.crypto_network_for_request(Chain::Base).rpc.as_deref(),
            Some("http://localhost:8545")
        );
        assert!(
            vault
                .crypto_network_for_request(Chain::Ethereum)
                .rpc
                .is_none()
        );
        let links = vault.crypto_explorer_settings();
        assert_eq!(
            links.custom.get(&Chain::Base).map(String::as_str),
            Some("https://explorer.example.org")
        );
        // Reset: all empty.
        vault
            .crypto_set_network_settings(&input(Chain::Base))
            .unwrap();
        assert!(vault.crypto_network_for_request(Chain::Base).rpc.is_none());
    }

    #[test]
    fn a_bad_address_changes_nothing() {
        let (_dir, mut vault) = level3();
        let mut good = input(Chain::Base);
        good.rpc = "https://node.example.org".into();
        vault.crypto_set_network_settings(&good).unwrap();
        let mut bad = input(Chain::Base);
        bad.rpc = "https://other.example.org".into();
        bad.blockscout = "http://scout.example.org".into();
        assert!(vault.crypto_set_network_settings(&bad).is_err());
        assert_eq!(
            vault.crypto_network_for_request(Chain::Base).rpc.as_deref(),
            Some("https://node.example.org")
        );
    }

    #[test]
    fn the_key_is_optional_kept_hidden_and_never_leaves_the_device_file() {
        let (dir, mut vault) = level3();
        let key = key_text();
        // Keyless by default.
        assert!(vault.crypto_network_for_request(Chain::Base).key.is_none());
        let mut save = input(Chain::Base);
        save.blockscout_key = Some(key.clone());
        vault.crypto_set_network_settings(&save).unwrap();
        vault.flush_pending().unwrap();

        // It reaches the reader, and the screen only learns that it is set.
        let request = vault.crypto_network_for_request(Chain::Base);
        assert_eq!(
            request.key.as_ref().map(BlockscoutKey::expose),
            Some(key.as_str())
        );
        let shown = vault.crypto_network_settings();
        let base = shown
            .networks
            .iter()
            .find(|net| net.chain == Chain::Base)
            .unwrap();
        assert!(base.blockscout_key_set);
        let wire = serde_json::to_string(&shown).unwrap();
        assert!(!wire.contains(&key));
        assert!(!format!("{shown:?}{request:?}").contains(&key));

        // Saving other fields keeps it; an empty text removes it.
        let mut again = input(Chain::Base);
        again.explorer = "https://explorer.example.org".into();
        vault.crypto_set_network_settings(&again).unwrap();
        assert!(vault.crypto_network_for_request(Chain::Base).key.is_some());
        let mut clear = input(Chain::Base);
        clear.blockscout_key = Some(String::new());
        vault.crypto_set_network_settings(&clear).unwrap();
        assert!(vault.crypto_network_for_request(Chain::Base).key.is_none());

        // A key a header cannot carry is refused without echo.
        let mut broken = input(Chain::Base);
        broken.blockscout_key = Some("has space".into());
        assert!(vault.crypto_set_network_settings(&broken).is_err());

        // On disk: not in the vault file, not in the mirror of the document,
        // not in the plain device file.
        vault.crypto_set_network_settings(&save).unwrap();
        vault.flush_pending().unwrap();
        for entry in std::fs::read_dir(dir.path()).unwrap() {
            let bytes = std::fs::read(entry.unwrap().path()).unwrap();
            assert!(
                !bytes.windows(key.len()).any(|win| win == key.as_bytes()),
                "the key is in a file in clear text"
            );
        }
        let doc = serde_json::to_string(&vault.doc).unwrap();
        assert!(!doc.contains(&key));
    }

    #[test]
    fn the_stored_key_is_only_offered_to_the_instance_that_issued_it() {
        let (_dir, mut vault) = level3();
        let key = key_text();
        let mine = "https://scout.example.org";
        let mut save = input(Chain::Base);
        save.blockscout = mine.into();
        save.blockscout_key = Some(key.clone());
        vault.crypto_set_network_settings(&save).unwrap();

        let target = |url: Option<&str>, kind| {
            vault
                .crypto_test_target(&TestNetworkInput {
                    chain: Chain::Base,
                    kind,
                    url: url.map(str::to_owned),
                    blockscout_key: None,
                })
                .unwrap()
        };

        // The instance the key belongs to: typed out in full, or left empty.
        for url in [Some(mine), Some("https://scout.example.org/"), None] {
            let (_, found) = target(url, EndpointKind::Blockscout);
            assert_eq!(
                found.as_ref().map(BlockscoutKey::expose),
                Some(key.as_str()),
                "{url:?}"
            );
        }

        // Any other address is tested without the key, however it is written.
        for url in [
            "https://other-scout.example.org",
            "https://scout.example.org.evil.test",
            "https://scout.example.org:8443",
            "not an address",
        ] {
            let (_, found) = target(Some(url), EndpointKind::Blockscout);
            assert!(found.is_none(), "the key went out to {url}");
        }

        // A node is never offered a Blockscout key, not even its own instance.
        assert!(target(Some(mine), EndpointKind::Rpc).1.is_none());
        assert!(target(None, EndpointKind::Rpc).1.is_none());

        // A key typed into the request is used as typed, on any address.
        let (_, typed) = vault
            .crypto_test_target(&TestNetworkInput {
                chain: Chain::Base,
                kind: EndpointKind::Blockscout,
                url: Some("https://other-scout.example.org".into()),
                blockscout_key: Some(key.clone()),
            })
            .unwrap();
        assert_eq!(
            typed.as_ref().map(BlockscoutKey::expose),
            Some(key.as_str())
        );
    }

    #[test]
    fn a_key_does_not_survive_a_change_of_the_instance_it_was_issued_for() {
        let (_dir, mut vault) = level3();
        let key = key_text();
        let mut save = input(Chain::Base);
        save.blockscout = "https://scout.example.org".into();
        save.blockscout_key = Some(key.clone());
        vault.crypto_set_network_settings(&save).unwrap();
        assert!(vault.crypto_network_for_request(Chain::Base).key.is_some());

        // Another field of the same instance: the key stays.
        let mut same = input(Chain::Base);
        same.blockscout = "https://scout.example.org".into();
        same.rpc = "https://node.example.org".into();
        vault.crypto_set_network_settings(&same).unwrap();
        assert!(vault.crypto_network_for_request(Chain::Base).key.is_some());

        // The instance changes and no new key comes with it: the key is gone,
        // so no background pass can send it to the new host.
        let mut moved = input(Chain::Base);
        moved.blockscout = "https://other-scout.example.org".into();
        moved.rpc = "https://node.example.org".into();
        vault.crypto_set_network_settings(&moved).unwrap();
        assert!(vault.crypto_network_for_request(Chain::Base).key.is_none());

        // Back to the presets counts as a change as well.
        vault.crypto_set_network_settings(&save).unwrap();
        assert!(vault.crypto_network_for_request(Chain::Base).key.is_some());
        vault
            .crypto_set_network_settings(&input(Chain::Base))
            .unwrap();
        assert!(vault.crypto_network_for_request(Chain::Base).key.is_none());

        // A new instance with its own key keeps that one.
        let mut pair = input(Chain::Base);
        pair.blockscout = "https://third-scout.example.org".into();
        pair.blockscout_key = Some(key.clone());
        vault.crypto_set_network_settings(&pair).unwrap();
        assert_eq!(
            vault
                .crypto_network_for_request(Chain::Base)
                .key
                .as_ref()
                .map(BlockscoutKey::expose),
            Some(key.as_str())
        );
    }

    #[tokio::test]
    async fn without_consent_the_test_makes_no_call_and_with_it_the_key_goes_in_the_header() {
        let probe = Stub::new("0x2105");
        let key = BlockscoutKey::parse(&key_text()).unwrap();
        for kind in [EndpointKind::Rpc, EndpointKind::Blockscout] {
            let url = "https://node.example.org";
            let refused = test_endpoint(&probe, false, Chain::Base, kind, url, Some(&key)).await;
            assert!(refused.is_err());
        }
        assert_eq!(probe.calls.load(Ordering::SeqCst), 0, "no consent, no call");

        let ok = test_endpoint(
            &probe,
            true,
            Chain::Base,
            EndpointKind::Rpc,
            "https://node.example.org",
            None,
        )
        .await
        .unwrap();
        assert_eq!(ok.outcome, NetworkTestOutcome::Ok);
        assert_eq!(ok.block, Some(0x1676c1e));
        let scout = test_endpoint(
            &probe,
            true,
            Chain::Base,
            EndpointKind::Blockscout,
            "https://scout.example.org",
            Some(&key),
        )
        .await
        .unwrap();
        assert_eq!(scout.outcome, NetworkTestOutcome::Ok);
        assert_eq!(
            probe.seen_key.lock().unwrap().as_deref(),
            Some(key.expose())
        );
        // The result carries no text of the provider and no key.
        assert!(
            !serde_json::to_string(&scout)
                .unwrap()
                .contains(key.expose())
        );
    }

    #[tokio::test]
    async fn a_node_of_another_chain_is_called_wrong_network() {
        let probe = Stub::new("0x1");
        let result = test_endpoint(
            &probe,
            true,
            Chain::Base,
            EndpointKind::Rpc,
            "https://node.example.org",
            None,
        )
        .await
        .unwrap();
        assert_eq!(result.outcome, NetworkTestOutcome::WrongNetwork);
    }

    #[tokio::test]
    async fn a_refused_address_is_not_even_tried() {
        let probe = Stub::new("0x2105");
        let result = test_endpoint(
            &probe,
            true,
            Chain::Base,
            EndpointKind::Rpc,
            "http://node.example.org",
            None,
        )
        .await;
        assert!(result.is_err());
        assert_eq!(probe.calls.load(Ordering::SeqCst), 0);
    }
}
