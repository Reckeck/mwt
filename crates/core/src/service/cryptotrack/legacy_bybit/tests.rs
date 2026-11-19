use crate::Amount;
use crate::cryptotrack::sources::bybit::exchange_asset;
use crate::exchange::{Execution, InterTransfer};
use crate::model::store::ExchangeProvider;
use crate::model::store::StoreKind;
use crate::model::vault::PendingImport;
use crate::service::StoreExtras;
use crate::service::UnlockedVault;

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
            StoreExtras {
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

/// Everything the conversion writes, as the sync would compare it between
/// two devices.
fn written(vault: &UnlockedVault) -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "accounts": vault.doc.crypto_accounts,
        "events": vault.doc.crypto_events,
        "tokens": vault.doc.crypto_tokens,
        "prices": vault.doc.crypto_prices,
    }))
    .unwrap()
}

#[test]
fn storing_the_same_records_again_rewrites_them() {
    let (_dir, mut vault, wallet) = bybit_wallet();
    let records = vec![
        bought(&wallet, "exec-1", "60000", DAY_MS),
        bought(&wallet, "exec-2", "70000", DAY_MS + 60_000),
        moved(&wallet, "t-1", DAY_MS + 120_000),
    ];
    let first = vault.bybit_store(&wallet, &records).unwrap();
    assert_eq!(first.recorded(), 3);
    let after_first = written(&vault);

    let again = vault.bybit_store(&wallet, &records).unwrap();
    assert_eq!(again.recorded(), 0, "all three were settled already");
    assert_eq!(again.known, 3);
    assert_eq!(written(&vault), after_first);
}

#[test]
fn the_cost_pool_costs_what_the_fills_cost() {
    let (_dir, mut vault, wallet) = bybit_wallet();
    // Two buys of one bitcoin on one day, at different prices. The engine
    // can only price a day, so the day's point is their weighted mean —
    // and the pool must still cost 60000 + 70000.
    vault
        .bybit_store(
            &wallet,
            &[
                bought(&wallet, "exec-1", "60000", DAY_MS),
                bought(&wallet, "exec-2", "70000", DAY_MS + 60_000),
            ],
        )
        .unwrap();

    let totals = vault.crypto_totals(AT);
    let bitcoin = totals
        .assets
        .iter()
        .find(|total| total.asset == exchange_asset("BTC"))
        .expect("the coin is counted");
    assert_eq!(bitcoin.quantity, Amount::parse("2").unwrap());
    assert_eq!(bitcoin.cost_usd, Amount::parse("130000").unwrap());
    assert_eq!(bitcoin.average_usd, Some(Amount::parse("65000").unwrap()));
    // Nothing about the coin is in doubt: every leg was priced from the
    // records themselves. (The dollar stablecoin the buys were paid with
    // goes negative, because the history does not say where it came
    // from, and the engine says so about that asset — not about this
    // one.)
    assert!(!bitcoin.imprecise, "doubts: {:?}", bitcoin.doubts);
    assert!(bitcoin.doubts.is_empty());
}

#[test]
fn a_wallet_the_engine_drives_is_not_archivable() {
    let (_dir, mut vault, wallet) = bybit_wallet();
    // Before anything is followed, nothing claims the wallet.
    assert!(!vault.exchange_is_engine_driven(&wallet));
    vault
        .bybit_store(&wallet, &[bought(&wallet, "exec-1", "60000", DAY_MS)])
        .unwrap();
    assert!(vault.exchange_is_engine_driven(&wallet));
    // A sub-account of it answers the same, because archiving reaches the
    // whole wallet.
    let unified = vault
        .doc
        .stores
        .iter()
        .find(|store| store.parent_id.as_deref() == Some(wallet.as_str()))
        .expect("the wallet has its exchange accounts")
        .id
        .clone();
    assert!(vault.exchange_is_engine_driven(&unified));
    // A wallet that is not an exchange never does.
    let plain = vault
        .doc
        .stores
        .iter()
        .find(|store| store.kind != StoreKind::Exchange && store.parent_id.is_none())
        .expect("the new vault has an ordinary wallet")
        .id
        .clone();
    assert!(!vault.exchange_is_engine_driven(&plain));
}
