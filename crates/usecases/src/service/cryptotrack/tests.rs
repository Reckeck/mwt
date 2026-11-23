//! What the crypto module's data does to the vault as a whole: it ties the
//! vault to security level 3, and it must survive a save and a reopen.

use crate::error::Error;
use crate::ipc_contract::security::SecurityLevel;
use crate::model::chain::Chain;
use crate::model::cryptotrack::CryptoAccount;
use crate::model::vault::VaultDoc;
use crate::service::Session;
use crate::service::security_level::doc_carries_crypto_data;

fn account() -> CryptoAccount {
    CryptoAccount::new(
        "cacc-1",
        "Cold EVM",
        Chain::Ethereum,
        "0xAbCd00000000000000000000000000000000Ef01",
        "2026-11-10T00:00:00Z",
    )
}

#[test]
fn an_empty_document_carries_no_crypto_data() {
    let vault = Session::in_memory(VaultDoc::default());
    assert!(!doc_carries_crypto_data(&vault.doc));
    // The mark a wipe leaves behind is not the module's data: it is what
    // tells the other devices about the wipe, and a wiped vault must be
    // allowed back below level 3 (spec E12).
    let mut wiped = vault.doc.clone();
    wiped.crypto_wiped_at = crate::model::fixtures::full_doc().crypto_wiped_at;
    assert!(!doc_carries_crypto_data(&wiped));
}

/// Fills one crypto collection of a document from the full fixture.
type Fill = fn(&mut VaultDoc, &VaultDoc);

#[test]
fn every_crypto_collection_ties_the_vault_to_level_three() {
    let full = crate::model::fixtures::full_doc();
    let vault = Session::in_memory(VaultDoc::default());
    let cases: [(&str, Fill); 8] = [
        ("crypto_accounts", |doc, full| {
            doc.crypto_accounts = full.crypto_accounts.clone()
        }),
        ("crypto_events", |doc, full| {
            doc.crypto_events = full.crypto_events.clone()
        }),
        ("crypto_overrides", |doc, full| {
            doc.crypto_overrides = full.crypto_overrides.clone()
        }),
        ("crypto_tokens", |doc, full| {
            doc.crypto_tokens = full.crypto_tokens.clone()
        }),
        ("crypto_positions", |doc, full| {
            doc.crypto_positions = full.crypto_positions.clone()
        }),
        ("crypto_snapshots", |doc, full| {
            doc.crypto_snapshots = full.crypto_snapshots.clone()
        }),
        ("crypto_cursors", |doc, full| {
            doc.crypto_cursors = full.crypto_cursors.clone()
        }),
        ("crypto_prices", |doc, full| {
            doc.crypto_prices = full.crypto_prices.clone()
        }),
    ];
    for (name, fill) in cases {
        let mut doc = vault.doc.clone();
        fill(&mut doc, &full);
        assert!(
            doc_carries_crypto_data(&doc),
            "a non-empty {name} must tie the vault to level 3"
        );
    }
}

/// A plain vault holding crypto data refuses a PIN and refuses to lose its
/// password: the module's addresses and operations may only sit behind
/// level 3.
#[test]
fn crypto_data_refuses_a_downgrade() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("crypto.wallet");
    let mut vault = Session::create_with(path, None).unwrap();
    assert_eq!(vault.min_allowed_level(), SecurityLevel::Ledger);

    vault.doc.crypto_accounts = vec![account()];
    vault.persist().unwrap();
    vault.flush_pending().unwrap();
    assert_eq!(vault.min_allowed_level(), SecurityLevel::Crypto);

    let before = std::fs::read(&vault.path).unwrap();
    assert!(matches!(
        vault.set_password("482913", false),
        Err(Error::SecurityDowngradeRefused)
    ));
    assert_eq!(
        std::fs::read(&vault.path).unwrap(),
        before,
        "a refused downgrade must not touch the file"
    );
}

/// The crypto collections survive a save and a reopen: the file is written at
/// the current payload version with them in it.
#[test]
fn the_crypto_section_survives_a_save_and_a_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("crypto.wallet");
    let full = crate::model::fixtures::full_doc();
    {
        let mut vault = Session::create(path.clone(), "pw").unwrap();
        vault.doc.crypto_accounts = full.crypto_accounts.clone();
        vault.doc.crypto_events = full.crypto_events.clone();
        vault.doc.crypto_tokens = full.crypto_tokens.clone();
        vault.doc.crypto_prices = full.crypto_prices.clone();
        vault.persist().unwrap();
        vault.flush_pending().unwrap();
    }
    let again = Session::open(path, "pw").unwrap();
    assert_eq!(again.doc.version, crate::VAULT_FORMAT_VERSION);
    assert_eq!(again.doc.crypto_accounts, full.crypto_accounts);
    assert_eq!(again.doc.crypto_events, full.crypto_events);
    assert_eq!(again.doc.crypto_tokens, full.crypto_tokens);
    assert_eq!(again.doc.crypto_prices, full.crypto_prices);
}
