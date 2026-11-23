use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::{Value, json};

use crate::cryptotrack::chains;
use crate::cryptotrack::ports::{ChainCall, ChainSource};
use crate::cryptotrack::service::network::{normalize_blockscout, normalize_rpc};
use crate::error::{Error, Result};
use crate::ipc_contract::cryptotrack::network::{
    EndpointKind, NetworkTestOutcome, SetNetworkSettingsInput, TestNetworkInput,
};
use crate::model::chain::Chain;
use crate::service::UnlockedVault;
use crate::service::test_support::empty_vault;
use crate::usecases::cryptotrack::network::{Probe, test_endpoint};
use crate::vault_io::device_secrets::BlockscoutKey;

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

impl ChainSource for Stub {
    async fn get_json(&self, _base: &str, _path: &str, _subject: &str) -> Result<Value> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(json!({"total_blocks": "1"}))
    }

    async fn call(
        &self,
        _url: &str,
        _subject: &str,
        method: &str,
        _params: Value,
    ) -> Result<Value> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(match method {
            "eth_chainId" => json!(self.chain_id),
            _ => json!("0x1676c1e"),
        })
    }

    async fn batch(
        &self,
        _url: &str,
        _subject: &str,
        _calls: &[ChainCall],
    ) -> Result<Vec<Result<Value>>> {
        Ok(Vec::new())
    }
}

impl Probe for Stub {
    async fn get(&self, _base: &str, _path: &str, key: Option<&BlockscoutKey>) -> Result<Value> {
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
    let settings = crate::usecases::cryptotrack::network::crypto_network_settings(&vault.books());
    assert!(!settings.editable);
    assert_eq!(settings.networks.len(), chains::all().len());
    assert!(matches!(
        crate::usecases::cryptotrack::network::crypto_set_network_settings(
            &mut vault,
            &input(Chain::Base)
        ),
        Err(Error::SecurityLevelRequired)
    ));
}

#[test]
fn saved_endpoints_are_used_and_an_empty_one_goes_back_to_the_preset() {
    let (_dir, mut vault) = level3();
    let mut save = input(Chain::Base);
    save.rpc = "http://localhost:8545".into();
    save.explorer = "https://explorer.example.org/".into();
    crate::usecases::cryptotrack::network::crypto_set_network_settings(&mut vault, &save).unwrap();
    let settings = crate::usecases::cryptotrack::network::crypto_network_settings(&vault.books());
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
        crate::cryptotrack::service::network::crypto_network_for_request(
            &vault.books(),
            Chain::Base
        )
        .rpc
        .as_deref(),
        Some("http://localhost:8545")
    );
    assert!(
        crate::cryptotrack::service::network::crypto_network_for_request(
            &vault.books(),
            Chain::Ethereum
        )
        .rpc
        .is_none()
    );
    let links = crate::cryptotrack::service::network::crypto_explorer_settings(&vault.books());
    assert_eq!(
        links.custom.get(&Chain::Base).map(String::as_str),
        Some("https://explorer.example.org")
    );
    // Reset: all empty.
    crate::usecases::cryptotrack::network::crypto_set_network_settings(
        &mut vault,
        &input(Chain::Base),
    )
    .unwrap();
    assert!(
        crate::cryptotrack::service::network::crypto_network_for_request(
            &vault.books(),
            Chain::Base
        )
        .rpc
        .is_none()
    );
}

#[test]
fn a_bad_address_changes_nothing() {
    let (_dir, mut vault) = level3();
    let mut good = input(Chain::Base);
    good.rpc = "https://node.example.org".into();
    crate::usecases::cryptotrack::network::crypto_set_network_settings(&mut vault, &good).unwrap();
    let mut bad = input(Chain::Base);
    bad.rpc = "https://other.example.org".into();
    bad.blockscout = "http://scout.example.org".into();
    assert!(
        crate::usecases::cryptotrack::network::crypto_set_network_settings(&mut vault, &bad)
            .is_err()
    );
    assert_eq!(
        crate::cryptotrack::service::network::crypto_network_for_request(
            &vault.books(),
            Chain::Base
        )
        .rpc
        .as_deref(),
        Some("https://node.example.org")
    );
}

#[test]
fn the_key_is_optional_kept_hidden_and_never_leaves_the_device_file() {
    let (dir, mut vault) = level3();
    let key = key_text();
    // Keyless by default.
    assert!(
        crate::cryptotrack::service::network::crypto_network_for_request(
            &vault.books(),
            Chain::Base
        )
        .key
        .is_none()
    );
    let mut save = input(Chain::Base);
    save.blockscout_key = Some(key.clone());
    crate::usecases::cryptotrack::network::crypto_set_network_settings(&mut vault, &save).unwrap();
    vault.flush_pending().unwrap();

    // It reaches the reader, and the screen only learns that it is set.
    let request = crate::cryptotrack::service::network::crypto_network_for_request(
        &vault.books(),
        Chain::Base,
    );
    assert_eq!(
        request.key.as_ref().map(BlockscoutKey::expose),
        Some(key.as_str())
    );
    let shown = crate::usecases::cryptotrack::network::crypto_network_settings(&vault.books());
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
    crate::usecases::cryptotrack::network::crypto_set_network_settings(&mut vault, &again).unwrap();
    assert!(
        crate::cryptotrack::service::network::crypto_network_for_request(
            &vault.books(),
            Chain::Base
        )
        .key
        .is_some()
    );
    let mut clear = input(Chain::Base);
    clear.blockscout_key = Some(String::new());
    crate::usecases::cryptotrack::network::crypto_set_network_settings(&mut vault, &clear).unwrap();
    assert!(
        crate::cryptotrack::service::network::crypto_network_for_request(
            &vault.books(),
            Chain::Base
        )
        .key
        .is_none()
    );

    // A key a header cannot carry is refused without echo.
    let mut broken = input(Chain::Base);
    broken.blockscout_key = Some("has space".into());
    assert!(
        crate::usecases::cryptotrack::network::crypto_set_network_settings(&mut vault, &broken)
            .is_err()
    );

    // On disk: not in the vault file, not in the mirror of the document,
    // not in the plain device file.
    crate::usecases::cryptotrack::network::crypto_set_network_settings(&mut vault, &save).unwrap();
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
    crate::usecases::cryptotrack::network::crypto_set_network_settings(&mut vault, &save).unwrap();

    let target = |url: Option<&str>, kind| {
        crate::usecases::cryptotrack::network::crypto_test_target(
            &vault.books(),
            &TestNetworkInput {
                chain: Chain::Base,
                kind,
                url: url.map(str::to_owned),
                blockscout_key: None,
            },
        )
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
    let (_, typed) = crate::usecases::cryptotrack::network::crypto_test_target(
        &vault.books(),
        &TestNetworkInput {
            chain: Chain::Base,
            kind: EndpointKind::Blockscout,
            url: Some("https://other-scout.example.org".into()),
            blockscout_key: Some(key.clone()),
        },
    )
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
    crate::usecases::cryptotrack::network::crypto_set_network_settings(&mut vault, &save).unwrap();
    assert!(
        crate::cryptotrack::service::network::crypto_network_for_request(
            &vault.books(),
            Chain::Base
        )
        .key
        .is_some()
    );

    // Another field of the same instance: the key stays.
    let mut same = input(Chain::Base);
    same.blockscout = "https://scout.example.org".into();
    same.rpc = "https://node.example.org".into();
    crate::usecases::cryptotrack::network::crypto_set_network_settings(&mut vault, &same).unwrap();
    assert!(
        crate::cryptotrack::service::network::crypto_network_for_request(
            &vault.books(),
            Chain::Base
        )
        .key
        .is_some()
    );

    // The instance changes and no new key comes with it: the key is gone,
    // so no background pass can send it to the new host.
    let mut moved = input(Chain::Base);
    moved.blockscout = "https://other-scout.example.org".into();
    moved.rpc = "https://node.example.org".into();
    crate::usecases::cryptotrack::network::crypto_set_network_settings(&mut vault, &moved).unwrap();
    assert!(
        crate::cryptotrack::service::network::crypto_network_for_request(
            &vault.books(),
            Chain::Base
        )
        .key
        .is_none()
    );

    // Back to the presets counts as a change as well.
    crate::usecases::cryptotrack::network::crypto_set_network_settings(&mut vault, &save).unwrap();
    assert!(
        crate::cryptotrack::service::network::crypto_network_for_request(
            &vault.books(),
            Chain::Base
        )
        .key
        .is_some()
    );
    crate::usecases::cryptotrack::network::crypto_set_network_settings(
        &mut vault,
        &input(Chain::Base),
    )
    .unwrap();
    assert!(
        crate::cryptotrack::service::network::crypto_network_for_request(
            &vault.books(),
            Chain::Base
        )
        .key
        .is_none()
    );

    // A new instance with its own key keeps that one.
    let mut pair = input(Chain::Base);
    pair.blockscout = "https://third-scout.example.org".into();
    pair.blockscout_key = Some(key.clone());
    crate::usecases::cryptotrack::network::crypto_set_network_settings(&mut vault, &pair).unwrap();
    assert_eq!(
        crate::cryptotrack::service::network::crypto_network_for_request(
            &vault.books(),
            Chain::Base
        )
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
