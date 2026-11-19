//! The test cases of `adopt_legacy_bybit`, copied: the same records and the same expectations,
//! with the v4 document in place of the vault. The old queue is staged in a live vault first (the
//! only way to produce a queue today), and the vault's document is read back as a v4 document.

use super::*;
use crate::cryptotrack::sources::bybit::exchange_asset;
use crate::exchange::{Execution, InterTransfer};
use crate::model::store::ExchangeProvider;
use crate::model::vault::VaultDoc;
use crate::service::{StoreExtras, UnlockedVault};

const ACCOUNT_NAMES: [&str; 3] = ["Funding", "Unified", "Earn"];
/// The fills happen on 2023-11-14 UTC.
const DAY_MS: u64 = 1_700_000_000_000;
const AT: &str = "2023-11-15T00:00:00Z";

/// A Bybit wallet as an older build left it: `(dir, vault, wallet)`.
fn bybit_wallet() -> (tempfile::TempDir, UnlockedVault, String) {
    let dir = tempfile::tempdir().unwrap();
    let mut vault = UnlockedVault::create(dir.path().join("test.wallet"), "pw").unwrap();
    let wallet = vault
        .create_store_with(
            "Bybit",
            StoreKind::Exchange,
            "#f0b90b",
            "",
            None,
            crate::service::StoreExtras {
                provider: Some(ExchangeProvider::Bybit),
                ..StoreExtras::default()
            },
        )
        .unwrap()
        .id;
    vault.ensure_bybit_accounts(&wallet, ACCOUNT_NAMES).unwrap();
    (dir, vault, wallet)
}

fn fill(exec_id: &str, side: &str, qty: &str, price: &str, value: &str, at: u64) -> Execution {
    Execution {
        exec_id: exec_id.into(),
        symbol: "BTCUSDT".into(),
        side: side.into(),
        order_type: "Market".into(),
        exec_price: price.into(),
        exec_qty: qty.into(),
        exec_value: value.into(),
        exec_fee: "0".into(),
        exec_time: at.to_string(),
        is_maker: false,
        closed_size: None,
        category: "spot".into(),
    }
}

/// A bought-one-bitcoin fill, as the old import staged it.
fn bought(store: &str, exec_id: &str, price: &str, at: u64) -> PendingImport {
    PendingImport::from_execution(store, "spot", &fill(exec_id, "Buy", "1", price, price, at))
        .expect("a readable fill")
}

fn moved(store: &str, transfer_id: &str, at: u64) -> PendingImport {
    PendingImport::from_transfer(
        store,
        &InterTransfer {
            transfer_id: transfer_id.into(),
            coin: "USDT".into(),
            amount: "250".into(),
            from_account_type: "FUND".into(),
            to_account_type: "UNIFIED".into(),
            timestamp: at.to_string(),
            status: "SUCCESS".into(),
        },
    )
    .expect("a readable transfer")
}

/// The vault's document as a v4 file would hold it.
fn v4_of(vault: &UnlockedVault) -> VaultDocV4 {
    serde_json::from_value(serde_json::to_value(&vault.doc).unwrap()).unwrap()
}

/// A vault that holds the converted document, to ask the engine about it.
fn vault_of(old: VaultDocV4) -> (tempfile::TempDir, UnlockedVault) {
    let dir = tempfile::tempdir().unwrap();
    let mut vault = UnlockedVault::create(dir.path().join("converted.wallet"), "pw").unwrap();
    vault.replace_doc(VaultDoc::from(old)).unwrap();
    (dir, vault)
}

/// Everything the conversion writes, as the sync would compare it between two devices.
fn written(doc: &VaultDocV4) -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "accounts": doc.crypto_accounts,
        "events": doc.crypto_events,
        "tokens": doc.crypto_tokens,
        "prices": doc.crypto_prices,
        "queue": doc.pending_imports.iter().map(|record| record.exec_id.as_str()).collect::<Vec<_>>(),
    }))
    .unwrap()
}

fn event_id_list(doc: &VaultDocV4) -> Vec<String> {
    doc.crypto_events
        .iter()
        .map(|event| event.id.clone())
        .collect()
}

#[test]
fn converting_twice_writes_the_same_records() {
    let (_dir, mut vault, wallet) = bybit_wallet();
    vault
        .stage_imports(vec![
            bought(&wallet, "exec-1", "60000", DAY_MS),
            bought(&wallet, "exec-2", "70000", DAY_MS + 60_000),
            moved(&wallet, "t-1", DAY_MS + 120_000),
        ])
        .unwrap();
    let mut doc = v4_of(&vault);

    let first = adopt_bybit(&mut doc);
    assert_eq!(
        first,
        LegacyAdoption {
            converted: 3,
            known: 0,
            kept: 0
        }
    );
    let after_first = written(&doc);

    // The second pass has nothing left to convert and changes nothing.
    assert_eq!(adopt_bybit(&mut doc), LegacyAdoption::default());
    assert_eq!(written(&doc), after_first);

    // Storing the very same records again, as a later sweep would, rewrites them instead of
    // adding a second copy.
    let again = store_batch(
        &mut doc,
        &wallet,
        &[
            bought(&wallet, "exec-1", "60000", DAY_MS),
            bought(&wallet, "exec-2", "70000", DAY_MS + 60_000),
            moved(&wallet, "t-1", DAY_MS + 120_000),
        ],
    );
    assert_eq!(again.recorded(), 0, "all three were settled already");
    assert_eq!(again.known, 3);
    assert_eq!(written(&doc), after_first);
}

#[test]
fn two_devices_convert_the_same_file_to_the_same_records() {
    let (_dir, mut vault, wallet) = bybit_wallet();
    vault
        .stage_imports(vec![
            bought(&wallet, "exec-1", "60000", DAY_MS),
            bought(&wallet, "exec-2", "70000", DAY_MS + 60_000),
            moved(&wallet, "t-1", DAY_MS + 120_000),
        ])
        .unwrap();
    // The same file read on two devices.
    let mut first = v4_of(&vault);
    let mut second = v4_of(&vault);

    adopt_bybit(&mut first);
    adopt_bybit(&mut second);
    // Byte for byte: ids, times, prices and token records. Had any of them come from a clock or
    // a counter, the two would differ here and the sync would see one operation as two.
    assert_eq!(written(&first), written(&second));
}

#[test]
fn an_old_queue_loses_not_one_operation() {
    let (_dir, mut vault, wallet) = bybit_wallet();
    // The file an older build left: one record booked into the ledger, one refused, three still
    // waiting, and one the module cannot read.
    let mut broken = bought(&wallet, "exec-broken", "60000", DAY_MS);
    broken.occurred_at = String::new();
    vault
        .stage_imports(vec![
            bought(&wallet, "exec-booked", "50000", DAY_MS),
            bought(&wallet, "exec-refused", "55000", DAY_MS),
            bought(&wallet, "exec-1", "60000", DAY_MS),
            bought(&wallet, "exec-2", "70000", DAY_MS + 60_000),
            moved(&wallet, "t-1", DAY_MS + 120_000),
            broken,
        ])
        .unwrap();
    vault
        .confirm_pending_imports(&["exec-booked".to_owned()])
        .unwrap();
    vault
        .discard_pending_imports(&["exec-refused".to_owned()])
        .unwrap();
    let mut doc = v4_of(&vault);
    let origins = |doc: &VaultDocV4| -> Vec<String> {
        doc.transactions
            .iter()
            .map(|tx| format!("{:?}", tx.meta().origin))
            .collect()
    };
    let booked = origins(&doc);

    let out = adopt_bybit(&mut doc);

    // Operation by operation: the three readable ones became operations of the module, under the
    // id their facts give them.
    assert_eq!(
        event_id_list(&doc),
        vec![
            format!("cev:bybit:{wallet}:bybit:exec-1"),
            format!("cev:bybit:{wallet}:bybit:exec-2"),
            format!("cev:bybit:{wallet}:bybit:transfer:t-1"),
        ]
    );
    assert_eq!(
        out,
        LegacyAdoption {
            converted: 3,
            known: 0,
            kept: 1
        }
    );
    // The one that cannot be read is still in the queue, not thrown away.
    assert_eq!(
        doc.pending_imports
            .iter()
            .map(|record| record.exec_id.as_str())
            .collect::<Vec<_>>(),
        vec!["exec-broken"]
    );
    // The booked one is still the ledger transaction it was, untouched.
    assert_eq!(origins(&doc), booked);
    // The refused one stays refused.
    assert_eq!(doc.discarded_imports, vec!["exec-refused"]);
    // Six records in, six accounted for, none twice.
    assert_eq!(out.converted + out.kept + 1 + 1, 6);
}

#[test]
fn a_record_is_never_in_the_ledger_and_in_the_module_at_once() {
    let (_dir, mut vault, wallet) = bybit_wallet();
    vault
        .stage_imports(vec![
            bought(&wallet, "exec-booked", "50000", DAY_MS),
            bought(&wallet, "exec-1", "60000", DAY_MS),
        ])
        .unwrap();
    vault
        .confirm_pending_imports(&["exec-booked".to_owned()])
        .unwrap();
    let mut doc = v4_of(&vault);
    adopt_bybit(&mut doc);

    let in_ledger: BTreeSet<String> = doc
        .transactions
        .iter()
        .filter_map(|tx| match &tx.meta().origin {
            TxOrigin::Import { exec_id } => Some(exec_id.clone()),
            _ => None,
        })
        .collect();
    let in_module = event_ids(&doc);
    assert_eq!(in_ledger, BTreeSet::from(["exec-booked".to_owned()]));
    assert_eq!(in_module, BTreeSet::from(["exec-1".to_owned()]));
    assert!(in_ledger.is_disjoint(&in_module));

    // In the totals: the module counts the one bitcoin it was given and not the one the ledger
    // already holds.
    let (_dir_converted, converted) = vault_of(doc);
    let totals = converted.crypto_totals(AT);
    let bitcoin = totals
        .assets
        .iter()
        .find(|total| total.asset == exchange_asset("BTC"))
        .expect("the coin is counted, not quarantined");
    assert_eq!(bitcoin.quantity, Amount::parse("1").unwrap());
    assert_eq!(bitcoin.cost_usd, Amount::parse("60000").unwrap());
    // And the profit is not touched by the booked record either.
    assert_eq!(bitcoin.realized_usd, Amount::ZERO);
    assert_eq!(totals.realized_usd, Amount::ZERO);
}

#[test]
fn the_cost_pool_costs_what_the_fills_cost() {
    let (_dir, mut vault, wallet) = bybit_wallet();
    // Two buys of one bitcoin on one day, at different prices. The engine can only price a day,
    // so the day's point is their weighted mean, and the pool must still cost 60000 + 70000.
    vault
        .stage_imports(vec![
            bought(&wallet, "exec-1", "60000", DAY_MS),
            bought(&wallet, "exec-2", "70000", DAY_MS + 60_000),
        ])
        .unwrap();
    let mut doc = v4_of(&vault);
    adopt_bybit(&mut doc);

    let (_dir_converted, converted) = vault_of(doc);
    let totals = converted.crypto_totals(AT);
    let bitcoin = totals
        .assets
        .iter()
        .find(|total| total.asset == exchange_asset("BTC"))
        .expect("the coin is counted");
    assert_eq!(bitcoin.quantity, Amount::parse("2").unwrap());
    assert_eq!(bitcoin.cost_usd, Amount::parse("130000").unwrap());
    assert_eq!(bitcoin.average_usd, Some(Amount::parse("65000").unwrap()));
    // Nothing about the coin is in doubt: every leg was priced from the records themselves.
    assert!(!bitcoin.imprecise, "doubts: {:?}", bitcoin.doubts);
    assert!(bitcoin.doubts.is_empty());
}

#[test]
fn a_wallet_the_engine_drives_is_not_archivable() {
    let (_dir, mut vault, wallet) = bybit_wallet();
    // Before anything is followed, nothing claims the wallet.
    assert!(!vault.exchange_is_engine_driven(&wallet));
    vault
        .stage_imports(vec![bought(&wallet, "exec-1", "60000", DAY_MS)])
        .unwrap();
    let mut doc = v4_of(&vault);
    adopt_bybit(&mut doc);
    let (_dir_converted, converted) = vault_of(doc);
    assert!(converted.exchange_is_engine_driven(&wallet));
    // A sub-account of it answers the same, because archiving reaches the whole wallet.
    let unified = converted
        .doc
        .stores
        .iter()
        .find(|store| store.parent_id.as_deref() == Some(wallet.as_str()))
        .expect("the wallet has its exchange accounts")
        .id
        .clone();
    assert!(converted.exchange_is_engine_driven(&unified));
}

#[test]
fn a_record_of_a_wallet_that_is_gone_stays_in_the_queue() {
    let (_dir, mut vault, wallet) = bybit_wallet();
    vault
        .stage_imports(vec![bought(&wallet, "exec-1", "60000", DAY_MS)])
        .unwrap();
    let mut doc = v4_of(&vault);
    doc.stores
        .retain(|store| store.id != wallet && store.parent_id.as_deref() != Some(wallet.as_str()));
    let out = adopt_bybit(&mut doc);
    assert_eq!(
        out,
        LegacyAdoption {
            converted: 0,
            known: 0,
            kept: 1
        }
    );
    assert!(doc.crypto_events.is_empty());
    assert_eq!(doc.pending_imports.len(), 1);
}
