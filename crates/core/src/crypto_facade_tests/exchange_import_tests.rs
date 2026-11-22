//! Pins of the pure rules of `exchange_import`.

use crate::cryptotrack::exchange_import::{
    clamp_interval, normalize_watchlist, plan_staging, set_drawings, terminal_symbol,
    transaction_for_import,
};
use crate::exchange::Execution;
use crate::exchange::staging::PendingImportExt;
use crate::model::store::{StoreKind, TerminalState};
use crate::model::transaction::{Transaction, TxOrigin};
use crate::model::vault::PendingImport;
use crate::service::UnlockedVault;

fn vault_with_wallet() -> (tempfile::TempDir, UnlockedVault, String) {
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
                provider: Some(crate::model::store::ExchangeProvider::Bybit),
                ..Default::default()
            },
        )
        .unwrap()
        .id;
    (dir, vault, wallet)
}

fn fill(store: &str, exec_id: &str) -> PendingImport {
    let exec = Execution {
        exec_id: exec_id.into(),
        symbol: "SOLUSDT".into(),
        side: "Buy".into(),
        order_type: "Market".into(),
        exec_price: "100".into(),
        exec_qty: "1".into(),
        exec_value: "100".into(),
        exec_fee: "0".into(),
        exec_time: "1700000000000".into(),
        is_maker: false,
        closed_size: None,
        category: String::new(),
    };
    PendingImport::from_execution(store, "spot", &exec).unwrap()
}

#[test]
fn a_fill_becomes_a_spot_trade_marked_with_its_exec_id() {
    let (_dir, vault, wallet) = vault_with_wallet();
    let tx = transaction_for_import(&vault.books(), &fill(&wallet, "a"));
    let Transaction::TradeSpot(trade) = &tx else {
        panic!("a spot trade");
    };
    assert_eq!(trade.bought.asset.symbol, "SOL");
    assert!(matches!(&tx.meta().origin, TxOrigin::Import { exec_id } if exec_id == "a"));
}

#[test]
fn staging_takes_each_exec_id_once_and_respects_the_cap() {
    let (_dir, vault, wallet) = vault_with_wallet();
    let batch = vec![fill(&wallet, "a"), fill(&wallet, "a"), fill(&wallet, "b")];
    let staged = plan_staging(&vault.books(), batch.clone(), 0, 10, |_| true);
    assert_eq!(staged.len(), 2, "the repeated id counts once");
    assert!(plan_staging(&vault.books(), batch.clone(), 0, 10, |_| false).is_empty());
    assert_eq!(plan_staging(&vault.books(), batch, 0, 1, |_| true).len(), 1);
}

#[test]
fn the_terminal_rules_normalize_what_they_keep() {
    assert_eq!(clamp_interval(0), 0);
    assert_eq!(clamp_interval(5), 60);
    assert_eq!(clamp_interval(1_000_000), 86_400);
    assert_eq!(
        normalize_watchlist(vec![" btc ".into(), "BTC".into(), "".into()]),
        ["BTC"]
    );
    assert_eq!(terminal_symbol(" eth "), Some("ETH".into()));
    assert_eq!(terminal_symbol("  "), None);

    let mut terminal = TerminalState::default();
    assert!(set_drawings(&mut terminal, "BTC", "[{}]"));
    assert!(
        !set_drawings(&mut terminal, "BTC", "[{}]"),
        "same drawings, no change"
    );
    assert!(
        set_drawings(&mut terminal, "BTC", "[]"),
        "empty removes the entry"
    );
    assert!(terminal.drawings.is_empty());
}
