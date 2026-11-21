//! The endpoints of each network, kept on this device (spec E17): the address
//! rules, what the reader of a network uses and how an edit changes the stored
//! entry. The views, the connection test and the one write (the device file)
//! are in `usecases::cryptotrack::network`.
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

use reqwest::Url;

use crate::cryptotrack::chains;
use crate::cryptotrack::tracking::{ExplorerSettings, normalize_explorer};
use crate::error::{Error, Result};
use crate::model::chain::Chain;
use crate::model::device_secrets::{BlockscoutKey, DeviceSecrets, NetworkEndpoints};
use crate::money::books::Books;

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
pub fn same_instance(left: &str, right: &str) -> bool {
    match (normalize_blockscout(left), normalize_blockscout(right)) {
        (Ok(one), Ok(two)) => one == two,
        _ => false,
    }
}

/// Only the host and port of an address.
pub fn host_of(url: &str) -> String {
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

// -- the reads ------------------------------------------------------------------

/// The secrets of the networks, or none below level 3 (the presets then).
pub fn network_secrets(books: &Books<'_>) -> Result<DeviceSecrets> {
    if !books.keys_on_device() {
        return Ok(DeviceSecrets::default());
    }
    books.device_secrets_for_request()
}

/// What the reader of `chain` uses for one request. Never fails: a vault
/// below level 3, a damaged file or a bad value mean the presets and no key.
pub fn crypto_network_for_request(books: &Books<'_>, chain: Chain) -> NetworkRequest {
    let secrets = network_secrets(books).unwrap_or_default();
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
pub fn crypto_explorer_settings(books: &Books<'_>) -> ExplorerSettings {
    let mut out = ExplorerSettings::default();
    for entry in network_secrets(books).unwrap_or_default().crypto_networks {
        if let Some(url) = entry.explorer.as_deref() {
            let _ = out.set(entry.chain, url);
        }
    }
    out
}

// -- the edit -----------------------------------------------------------------

/// One network's new endpoints, every value already checked.
#[derive(Debug)]
pub struct EndpointEdit {
    rpc: Option<String>,
    blockscout: Option<String>,
    explorer: Option<String>,
    /// `None` leaves the stored key, `Some(None)` removes it.
    key_change: Option<Option<BlockscoutKey>>,
}

/// Check everything the user typed before anything is written. An empty text
/// is "the preset".
pub fn check_edit(
    chain: Chain,
    rpc: &str,
    blockscout: &str,
    explorer: &str,
    key: Option<&str>,
) -> Result<EndpointEdit> {
    chains::evm_chain(chain).ok_or(Error::Unsupported)?;
    let rpc = optional(rpc, normalize_rpc)?;
    let blockscout = optional(blockscout, normalize_blockscout)?;
    let explorer = optional(explorer, normalize_explorer)?;
    let key_change = match key {
        None => None,
        Some(text) if text.trim().is_empty() => Some(None),
        Some(text) => Some(Some(BlockscoutKey::parse(text)?)),
    };
    Ok(EndpointEdit {
        rpc,
        blockscout,
        explorer,
        key_change,
    })
}

/// Put the checked endpoints of `chain` into the secrets, replacing its old
/// entry; an entry left blank is removed.
pub fn apply_edit(secrets: &mut DeviceSecrets, chain: Chain, edit: EndpointEdit) {
    let stored = secrets.network_for(chain);
    let kept_key = stored.and_then(|entry| entry.blockscout_key.clone());
    let kept_blockscout = stored.and_then(|entry| entry.blockscout.clone());
    // A key is issued by one instance for itself. When the instance changes
    // and the request brings no new key, the old key is dropped rather than
    // kept and sent to a host that never gave it out (both values are
    // already normalized, and `None` on both sides is the same preset).
    let blockscout_key = match edit.key_change {
        Some(change) => change,
        None if edit.blockscout == kept_blockscout => kept_key,
        None => None,
    };
    let entry = NetworkEndpoints {
        chain,
        rpc: edit.rpc,
        blockscout: edit.blockscout,
        explorer: edit.explorer,
        blockscout_key,
    };
    secrets
        .crypto_networks
        .retain(|existing| existing.chain != chain);
    if !entry.is_blank() {
        secrets.crypto_networks.push(entry);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn save(
        secrets: &mut DeviceSecrets,
        chain: Chain,
        rpc: &str,
        blockscout: &str,
        key: Option<&str>,
    ) {
        let edit = check_edit(chain, rpc, blockscout, "", key).expect("the request is valid");
        apply_edit(secrets, chain, edit);
    }

    fn key_of(secrets: &DeviceSecrets) -> Option<String> {
        secrets
            .network_for(Chain::Base)
            .and_then(|entry| entry.blockscout_key.as_ref().map(BlockscoutKey::expose))
            .map(str::to_owned)
    }

    #[test]
    fn nothing_is_applied_before_every_value_is_checked() {
        assert!(
            check_edit(
                Chain::Base,
                "https://node.example.org",
                "http://scout.example.org",
                "",
                None
            )
            .is_err()
        );
        assert!(matches!(
            check_edit(Chain::Bitcoin, "", "", "", None),
            Err(Error::Unsupported)
        ));
    }

    #[test]
    fn a_key_follows_its_instance_and_a_blank_entry_is_removed() {
        let mut secrets = DeviceSecrets::default();
        save(
            &mut secrets,
            Chain::Base,
            "",
            "https://scout.example.org",
            Some("a-key-without-spaces"),
        );
        assert_eq!(key_of(&secrets).as_deref(), Some("a-key-without-spaces"));

        save(
            &mut secrets,
            Chain::Base,
            "https://node.example.org",
            "https://scout.example.org",
            None,
        );
        assert!(
            key_of(&secrets).is_some(),
            "the same instance keeps its key"
        );

        save(
            &mut secrets,
            Chain::Base,
            "",
            "https://other.example.org",
            None,
        );
        assert!(key_of(&secrets).is_none(), "another instance never gets it");

        save(&mut secrets, Chain::Base, "", "", None);
        assert!(
            secrets.network_for(Chain::Base).is_none(),
            "a blank entry is gone"
        );
    }

    #[test]
    fn only_the_same_normalized_instance_is_the_same_instance() {
        assert!(same_instance(
            "https://a.example.org/",
            "https://a.example.org"
        ));
        assert!(!same_instance(
            "https://a.example.org",
            "https://a.example.org:8443"
        ));
        assert!(!same_instance("not an address", "not an address"));
    }
}
