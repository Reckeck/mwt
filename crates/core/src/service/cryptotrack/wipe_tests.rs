//! What a wipe of the module must and must not do (spec E12).

use std::collections::BTreeMap;

use super::*;
use crate::model::chain::Chain;
use crate::model::cryptotrack::{CryptoAccount, CryptoPricePoint, TokenId};
use crate::model::store::StoreKind;
use crate::service::peer_sync::PeerRef;
use crate::service::security_level::doc_carries_crypto_data;
use crate::service::test_support::empty_vault;
use crate::sync::state::RecordState;

const ADDRESS: &str = "0x00000000000000000000000000000000000000aa";

fn account(id: &str) -> CryptoAccount {
    CryptoAccount::new(
        id,
        "Cold EVM",
        Chain::Ethereum,
        ADDRESS,
        "2026-11-10T00:00:00Z",
    )
}

fn price(at: &str) -> CryptoPricePoint {
    CryptoPricePoint {
        token_key: TokenId::new(Chain::Ethereum, ADDRESS).key(),
        value_usd: crate::service::test_support::amount("1800.25"),
        ts: at.to_string(),
        source: "test".into(),
    }
}

fn peer(name: &str) -> PeerRef {
    PeerRef {
        device_id: format!("{name}-id"),
        name: name.into(),
    }
}

/// A level-3 vault that follows one address, with a wallet of that address in
/// the tree, one operation of the module and one cached quote.
fn followed() -> (tempfile::TempDir, UnlockedVault, String) {
    let (dir, mut vault) = empty_vault();
    vault.raise_to_level3_for_tests();
    let wallet = crate::usecases::money::stores::create_store(
        &mut vault,
        "Cold EVM",
        StoreKind::Crypto,
        "#111",
        "",
        None,
    )
    .expect("the wallet is made")
    .id;
    vault
        .doc
        .stores
        .iter_mut()
        .find(|store| store.id == wallet)
        .expect("the wallet is in the tree")
        .crypto_account_id = Some("cacc-1".into());
    vault.doc.crypto_accounts.push(account("cacc-1"));
    vault.doc.crypto_events = crate::model::fixtures::full_doc().crypto_events;
    vault.doc.crypto_prices.push(price("2026-01-01T00:00:00Z"));
    vault.persist().expect("the state before the wipe is saved");
    (dir, vault, wallet)
}

#[test]
fn a_wipe_needs_the_word_and_changes_nothing_without_it() {
    let (_dir, mut vault, _wallet) = followed();
    let before = vault.doc.crypto_accounts.clone();

    assert!(matches!(
        crate::usecases::cryptotrack::wipe::crypto_wipe_module(&mut vault, ""),
        Err(Error::InvalidInput(_))
    ));
    assert!(matches!(
        crate::usecases::cryptotrack::wipe::crypto_wipe_module(&mut vault, "wipe"),
        Err(Error::InvalidInput(_)),
    ));

    assert_eq!(vault.doc.crypto_accounts, before);
    assert!(vault.doc.crypto_wiped_at.is_empty());
    assert!(
        vault
            .list_backups()
            .expect("the folder is readable")
            .is_empty(),
        "a refused wipe does not even take a backup"
    );
}

/// The acceptance line of the spec: without a checked backup nothing is
/// deleted. The backup step is handed in, so the test sees exactly the state
/// a failing copy leaves behind.
#[test]
fn without_a_backup_that_opens_nothing_is_wiped() {
    let (_dir, mut vault, wallet) = followed();
    let before = vault.doc.clone();

    let refused = crate::usecases::cryptotrack::wipe::wipe_inner(&mut vault, WIPE_CONFIRM, |_| {
        Err(Error::Io("the disk is full".into()))
    });

    assert!(matches!(refused, Err(Error::Io(_))));
    assert_eq!(vault.doc.crypto_accounts, before.crypto_accounts);
    assert_eq!(vault.doc.crypto_events, before.crypto_events);
    assert_eq!(vault.doc.crypto_prices, before.crypto_prices);
    assert!(vault.doc.crypto_wiped_at.is_empty());
    let store =
        crate::money::tree::find_store(&vault.doc, &wallet).expect("the wallet is still there");
    assert_eq!(store.crypto_account_id.as_deref(), Some("cacc-1"));
    assert!(!store.archived);
    assert!(doc_carries_crypto_data(&vault.doc));
    assert_eq!(vault.min_allowed_level(), SecurityLevel::Crypto);
}

/// The backup is taken and read back **before** anything goes, so it still
/// holds the module after the vault does not.
#[test]
fn the_wipe_leaves_a_backup_that_still_holds_the_module() {
    let (_dir, mut vault, _wallet) = followed();

    let report = crate::usecases::cryptotrack::wipe::crypto_wipe_module(&mut vault, WIPE_CONFIRM)
        .expect("the wipe runs");

    let name = report
        .backup
        .expect("a wipe that removed something took a copy");
    let path = backups::resolve(&vault.path, &name).expect("the copy is next to the vault");
    let (saved, _) = VaultFile::open(&path, &vault.password).expect("the copy opens");
    assert_eq!(
        saved.crypto_accounts.len(),
        1,
        "the copy is the state before"
    );
    assert_eq!(saved.crypto_events.len(), 1);
    assert!(
        vault.doc.crypto_accounts.is_empty(),
        "and the vault is empty"
    );
}

#[test]
fn a_wipe_unlinks_and_archives_the_wallets_and_lifts_the_ban() {
    let (dir, mut vault, wallet) = followed();
    let path = dir.path().join("test.wallet");

    let report = crate::usecases::cryptotrack::wipe::crypto_wipe_module(&mut vault, WIPE_CONFIRM)
        .expect("the wipe runs");

    assert_eq!(report.counts.accounts, 1);
    assert_eq!(report.counts.events, 1);
    assert_eq!(report.counts.prices, 1);
    assert_eq!(report.wallets.len(), 1, "the report names what it archived");
    assert_eq!(report.wallets[0].store_id, wallet);
    assert_eq!(report.wallets[0].name, "Cold EVM");
    assert!(report.can_lower_level);

    // The wallet is still a wallet: nothing of its history was deleted.
    let store =
        crate::money::tree::find_store(&vault.doc, &wallet).expect("the wallet is not deleted");
    assert!(
        store.crypto_account_id.is_none(),
        "it is no longer led by the module"
    );
    assert!(store.archived);
    assert!(!doc_carries_crypto_data(&vault.doc));
    assert_eq!(vault.min_allowed_level(), SecurityLevel::Ledger);
    assert_eq!(
        vault.doc.crypto_wiped_at.len(),
        1,
        "the mark the devices read"
    );

    // And it is on the disk, not only in memory.
    drop(vault);
    let reopened = UnlockedVault::open(path, "pw").expect("the vault opens");
    assert!(!doc_carries_crypto_data(&reopened.doc));
    assert!(
        crate::money::tree::find_store(&reopened.doc, &wallet)
            .expect("the wallet survived the reopen")
            .archived
    );
}

#[test]
fn a_vault_without_the_module_is_wiped_without_risking_anything() {
    let (_dir, mut vault) = empty_vault();

    let report = crate::usecases::cryptotrack::wipe::crypto_wipe_module(&mut vault, WIPE_CONFIRM)
        .expect("the call is allowed");

    assert!(
        report.backup.is_none(),
        "nothing to lose, so nothing is copied"
    );
    assert!(report.wiped_at.is_none(), "and no mark is left behind");
    assert!(report.wallets.is_empty());
    assert!(vault.doc.crypto_wiped_at.is_empty());
    assert!(
        vault
            .list_backups()
            .expect("the folder is readable")
            .is_empty(),
        "an empty module does not fill the folder with copies"
    );
}

/// The exchange keys, the per-network endpoints and the Blockscout key live
/// in one file next to the vault, and the wipe removes that file whole.
#[test]
fn the_device_secrets_file_goes_with_the_module() {
    let (_dir, mut vault, _wallet) = followed();
    let secrets_file = crate::vault_io::device_secrets::secrets_path(&vault.path);
    // The file itself is what the wipe removes; what is sealed inside it is
    // the key module's business and never this one's.
    std::fs::write(&secrets_file, b"sealed bytes").expect("the device keeps a file");
    assert!(crate::usecases::cryptotrack::wipe::crypto_wipe_preview(&vault).device_secrets);

    let report = crate::usecases::cryptotrack::wipe::crypto_wipe_module(&mut vault, WIPE_CONFIRM)
        .expect("the wipe runs");

    assert!(report.device_secrets_removed);
    assert!(
        !secrets_file.exists(),
        "the keys of this device went with it"
    );
}

/// Two devices, one of which was offline while the module was wiped. It must
/// not hand the module back — including the records the wiped device never
/// saw, which leave no tombstone to lose against — and what it makes *after*
/// the wipe must still arrive.
#[test]
fn an_offline_device_does_not_bring_the_module_back() {
    let dir = tempfile::tempdir().expect("a folder");
    let (_left_dir, mut left, _wallet) = followed();
    left.set_device("aaaaaaaa-left");
    left.persist().expect("saved");
    left.flush_pending().expect("written");
    let copy = dir.path().join("copy.wallet");
    std::fs::copy(&left.path, &copy).expect("the second device gets the vault");
    let mut right = UnlockedVault::open(copy, "pw").expect("the copy opens");
    right.set_device("bbbbbbbb-right");
    assert_eq!(right.security_level(), SecurityLevel::Crypto);
    assert_eq!(right.doc.crypto_accounts.len(), 1, "both hold the module");

    // Offline, `right` follows another address of its own and caches a quote.
    // `left` never hears of either, so a tombstone cannot answer them later.
    right.doc.crypto_accounts.push(account("cacc-right"));
    right.doc.crypto_prices.push(price("2026-02-02T00:00:00Z"));
    right.persist().expect("saved");

    let report = crate::usecases::cryptotrack::wipe::crypto_wipe_module(&mut left, WIPE_CONFIRM)
        .expect("the wipe runs");
    assert!(report.wiped_at.is_some());

    // `right` comes back. What it sends is what it holds, the lot.
    let from_right = exchange(&mut right, &mut left);
    assert!(
        from_right
            .keys()
            .any(|key| key.starts_with("crypto_accounts:")),
        "the offline device really did offer the module back"
    );
    assert!(
        left.doc.crypto_accounts.is_empty(),
        "and none of it was taken: not even the address the wiped device never saw"
    );
    assert!(
        left.doc.crypto_prices.is_empty(),
        "nor the quotes, which union silently"
    );
    assert!(!doc_carries_crypto_data(&left.doc));
    assert_eq!(left.min_allowed_level(), SecurityLevel::Ledger);

    // The other direction of the same exchange carries the mark over, and with
    // it the tombstones of what `left` held. What `right` made on its own
    // stays: a mark is not a remote wipe — the wipe destroys only on the
    // device that was asked for it, where it could take a backup first.
    exchange(&mut left, &mut right);
    assert_eq!(right.doc.crypto_wiped_at.len(), 1, "the mark travelled");
    assert_eq!(
        right
            .doc
            .crypto_accounts
            .iter()
            .map(|acc| acc.id.as_str())
            .collect::<Vec<_>>(),
        vec!["cacc-right"],
        "what `left` held is gone by its tombstone; what `right` made is its own"
    );

    // What `right` makes now is after the wipe, and travels as usual.
    right.doc.crypto_accounts.push(account("cacc-after"));
    right.persist().expect("saved");
    exchange(&mut right, &mut left);
    assert_eq!(
        left.doc
            .crypto_accounts
            .iter()
            .map(|acc| acc.id.as_str())
            .collect::<Vec<_>>(),
        vec!["cacc-after"],
        "a record made after the wipe is not what the wipe removed"
    );
}

/// One direction of a sync: everything `to` is missing from `from`, as a
/// session would hand it over. Returns what was sent.
fn exchange(from: &mut UnlockedVault, to: &mut UnlockedVault) -> BTreeMap<String, RecordState> {
    let mine = crate::usecases::sync::read::sync_manifest(to, None).expect("a manifest");
    let theirs = crate::usecases::sync::read::sync_manifest(from, None).expect("a manifest");
    let (wants, _) = crate::usecases::sync::read::sync_diff(to, &mine, &theirs);
    let sent = crate::usecases::sync::read::sync_records(from, &wants).expect("the records");
    to.sync_apply(&sent, &peer("other"))
        .expect("the batch applies");
    sent
}
