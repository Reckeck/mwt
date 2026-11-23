//! Scenarios of the networks' endpoints (spec E17): the views the settings
//! screen shows, the connection test and the one write, saving a network. What
//! is allowed (the address rules, the key's life) and what a reader of a network
//! uses are in `cryptotrack::service::network`.
//!
//! The endpoints live in the device secrets file, not in the vault document, so
//! saving writes that file and no `transact` is involved: nothing in the
//! document changes. The connection test is a network call; it goes through the
//! [`ChainSource`] port, so a test runs it against a fake.

use std::future::Future;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::chain::{RpcClient, RpcConfig};
use crate::cryptotrack::chains::{self, EvmChain};
use crate::cryptotrack::ports::{ChainCall, ChainSource};
use crate::cryptotrack::service::network::{
    self as rules, host_of, network_secrets, normalize_blockscout, normalize_rpc, same_instance,
};
use crate::cryptotrack::tracking::normalize_explorer;
use crate::error::{Error, Result};
use crate::ipc_contract::cryptotrack::network::{
    EndpointKind, EndpointView, NetworkSettings, NetworkTestOutcome, NetworkTestResult,
    NetworkView, SetNetworkSettingsInput, TestNetworkInput,
};
use crate::model::chain::Chain;
use crate::money::books::Books;
use crate::service::{RpcChain, UnlockedVault};
use crate::vault_io::device_secrets::{self, BlockscoutKey, NetworkEndpoints};

pub use crate::cryptotrack::service::network::NetworkRequest;

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

/// The endpoints of every network and whether they can be changed here.
pub fn crypto_network_settings(books: &Books<'_>) -> NetworkSettings {
    // A damaged file shows the presets, read-only; saving is refused
    // (`usecases::cryptotrack::network::crypto_set_network_settings`) so it is
    // never overwritten blindly.
    let secrets = network_secrets(books);
    let editable = books.keys_on_device() && secrets.is_ok();
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

/// What a connection test sends: the address typed or in use, and the key
/// typed or stored.
pub fn crypto_test_target(
    books: &Books<'_>,
    input: &TestNetworkInput,
) -> Result<(String, Option<BlockscoutKey>)> {
    let row = chains::evm_chain(input.chain).ok_or(Error::Unsupported)?;
    let view = network_view(row, network_secrets(books)?.network_for(input.chain));
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
        _ if matches!(input.kind, EndpointKind::Blockscout) && same_instance(&url, &instance) => {
            network_secrets(books)?
                .network_for(input.chain)
                .and_then(|entry| entry.blockscout_key.clone())
        }
        _ => None,
    };
    Ok((url, key))
}

/// Save one network. Everything is checked before anything is written.
pub fn crypto_set_network_settings(
    vault: &mut UnlockedVault,
    input: &SetNetworkSettingsInput,
) -> Result<()> {
    if vault.sandbox {
        return Err(Error::Sandbox(
            "network settings are not kept in a sandbox".into(),
        ));
    }
    if !vault.keys_on_device() {
        return Err(Error::SecurityLevelRequired);
    }
    let edit = rules::check_edit(
        input.chain,
        &input.rpc,
        &input.blockscout,
        &input.explorer,
        input.blockscout_key.as_deref(),
    )?;
    // A damaged file is `Secret`: it is not replaced behind the user's back.
    let mut secrets = vault.device_secrets_for_request()?;
    rules::apply_edit(&mut secrets, input.chain, edit);
    let key = vault.device_seal_key()?;
    device_secrets::save(&vault.path, &key, &secrets)
}

// -- the connection test --------------------------------------------------------

/// What a connection test needs beyond a [`ChainSource`]: a GET on a
/// Blockscout instance with the key in a header (the port has no key). The JSON-RPC
/// call of the test is the port's own `call`. Production: [`NetworkProbe`]; the
/// tests use a stub that counts calls.
pub trait Probe: ChainSource {
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
pub struct NetworkProbe(RpcChain);

impl NetworkProbe {
    pub fn new() -> Result<Self> {
        RpcClient::new(RpcConfig {
            timeout: Duration::from_secs(8),
            max_retries: 0,
            allow_loopback_http: true,
            ..RpcConfig::default()
        })
        .map(|client| Self(RpcChain::new(client)))
    }
}

impl ChainSource for NetworkProbe {
    fn get_json(
        &self,
        base: &str,
        path_and_query: &str,
        subject: &str,
    ) -> impl Future<Output = Result<Value>> + Send {
        ChainSource::get_json(&self.0, base, path_and_query, subject)
    }

    fn call(
        &self,
        url: &str,
        subject: &str,
        method: &str,
        params: Value,
    ) -> impl Future<Output = Result<Value>> + Send {
        ChainSource::call(&self.0, url, subject, method, params)
    }

    fn batch(
        &self,
        url: &str,
        subject: &str,
        calls: &[ChainCall],
    ) -> impl Future<Output = Result<Vec<Result<Value>>>> + Send {
        ChainSource::batch(&self.0, url, subject, calls)
    }
}

impl Probe for NetworkProbe {
    async fn get(&self, base: &str, path: &str, key: Option<&BlockscoutKey>) -> Result<Value> {
        let client = self.0.with_api_key(key.map(BlockscoutKey::expose));
        ChainSource::get_json(&client, base, path, SUBJECT).await
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
            let id = match probe.call(&url, SUBJECT, "eth_chainId", json!([])).await {
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
                .call(&url, SUBJECT, "eth_blockNumber", json!([]))
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
