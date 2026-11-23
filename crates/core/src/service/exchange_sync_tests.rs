use crate::Amount;
use crate::exchange::staging::PendingImportExt;
use crate::exchange::{CoinBalance, Execution};
use crate::model::category::EXCHANGE_IMPORT_CATEGORY;
use crate::model::store::ImportAccountRoute;
use crate::model::store::ImportSettings;
use crate::model::store::StoreKind;
use crate::model::transaction::TxKind;
use crate::model::transaction::{Transaction, TxOrigin};
use crate::model::vault::{ImportSource, PendingImport};
use crate::service::StoreExtras;
use crate::service::*;

const ACCOUNT_NAMES: [&str; 3] = ["Funding", "Unified", "Earn"];

/// A Bybit wallet with its three accounts: `(dir, vault, wallet, unified)`.
fn bybit() -> (tempfile::TempDir, UnlockedVault, String, String) {
    let dir = tempfile::tempdir().unwrap();
    let mut vault = UnlockedVault::create(dir.path().join("test.wallet"), "pw").unwrap();
    let wallet = crate::usecases::money::stores::create_store_with(
        &mut vault,
        "Bybit",
        StoreKind::Exchange,
        "#f0b90b",
        "",
        None,
        StoreExtras {
            provider: Some(crate::model::store::ExchangeProvider::Bybit),
            ..StoreExtras::default()
        },
    )
    .unwrap()
    .id;
    let [_, unified, _] = crate::usecases::cryptotrack::exchange_import::ensure_bybit_accounts(
        &mut vault,
        &wallet,
        ACCOUNT_NAMES,
    )
    .unwrap();
    (dir, vault, wallet, unified)
}

fn buy(store: &str, exec_id: &str, symbol: &str, time_ms: &str) -> PendingImport {
    let exec = Execution {
        exec_id: exec_id.into(),
        symbol: symbol.into(),
        side: "Buy".into(),
        order_type: "Market".into(),
        exec_price: "100".into(),
        exec_qty: "1".into(),
        exec_value: "100".into(),
        exec_fee: "0".into(),
        exec_time: time_ms.into(),
        is_maker: false,
        closed_size: None,
        category: String::new(),
    };
    PendingImport::from_execution(store, "spot", &exec).unwrap()
}

fn balance(account: &str, coins: &[(&str, &str)]) -> (String, Vec<CoinBalance>) {
    (
        account.to_string(),
        coins
            .iter()
            .map(|(coin, amount)| CoinBalance {
                coin: (*coin).into(),
                amount: (*amount).into(),
            })
            .collect(),
    )
}

fn now() -> i64 {
    crate::calendar::now_secs()
}

#[test]
fn empty_polls_keep_one_run_and_write_nothing() {
    let (_dir, mut vault, wallet, _) = bybit();
    let before = std::fs::read(&vault.path).unwrap();
    for _ in 0..3 {
        let run = crate::usecases::cryptotrack::exchange_sync::sync_exchange(
            &mut vault,
            &wallet,
            Vec::new(),
            ImportSource::Auto,
            now(),
        )
        .unwrap();
        assert_eq!(run.staged, 0);
    }
    assert_eq!(
        crate::cryptotrack::exchange_sync::import_runs(&vault.books(), &wallet).len(),
        1,
        "one «checked at» row, not one per poll"
    );
    assert_eq!(
        std::fs::read(&vault.path).unwrap(),
        before,
        "nothing written for an empty poll"
    );
    assert!(!vault.has_unsaved_changes());
}

#[test]
fn a_sync_records_at_once_and_never_what_was_settled() {
    let (_dir, mut vault, wallet, _) = bybit();
    let first = crate::usecases::cryptotrack::exchange_sync::sync_exchange(
        &mut vault,
        &wallet,
        vec![
            buy(&wallet, "a", "SOLUSDT", "1700000000000"),
            buy(&wallet, "b", "SOLUSDT", "1700000000000"),
        ],
        ImportSource::Manual,
        now(),
    )
    .unwrap();
    // Nothing waits for confirmation any more: both fills are operations
    // of the module, and `staged` is not written (rule 5).
    assert_eq!((first.fetched, first.staged, first.confirmed), (2, 0, 2));
    assert_eq!((first.trades, first.transfers), (2, 0), "both are fills");
    assert!(crate::cryptotrack::exchange_import::pending_imports(&vault.doc).is_empty());
    assert_eq!(vault.doc.crypto_events.len(), 2);

    // The exchange answers with the same two fills on the next poll.
    let again = crate::usecases::cryptotrack::exchange_sync::sync_exchange(
        &mut vault,
        &wallet,
        vec![
            buy(&wallet, "a", "SOLUSDT", "1700000000000"),
            buy(&wallet, "b", "SOLUSDT", "1700000000000"),
        ],
        ImportSource::Auto,
        now(),
    )
    .unwrap();
    assert_eq!((again.confirmed, again.known), (0, 2));
    assert_eq!(vault.doc.crypto_events.len(), 2, "two, not four");
    assert_eq!(
        crate::cryptotrack::exchange_sync::import_runs(&vault.books(), &wallet).len(),
        2
    );

    // A record the user refused before stays refused and never becomes an
    // operation.
    vault.remember_discarded(["c".to_owned()]);
    let refused = crate::usecases::cryptotrack::exchange_sync::sync_exchange(
        &mut vault,
        &wallet,
        vec![buy(&wallet, "c", "SOLUSDT", "1700000000000")],
        ImportSource::Auto,
        now(),
    )
    .unwrap();
    assert_eq!((refused.confirmed, refused.known), (0, 1));
    assert_eq!(vault.doc.crypto_events.len(), 2);
}

/// The queue an older build wrote is still confirmed the way it was: a
/// user's file may hold one, and rule 5 keeps it readable.
#[test]
fn imported_operations_carry_the_import_mark_and_a_known_coin_id() {
    let (_dir, mut vault, wallet, _) = bybit();
    crate::usecases::cryptotrack::exchange_import::stage_imports(
        &mut vault,
        vec![buy(&wallet, "a", "SOLUSDT", "1700000000000")],
    )
    .unwrap();
    crate::usecases::cryptotrack::exchange_import::confirm_pending_imports(&mut vault, &[])
        .unwrap();
    let tx = vault.doc.transactions.last().unwrap();
    assert!(
        tx.meta()
            .categories
            .contains(&EXCHANGE_IMPORT_CATEGORY.to_string())
    );
    assert!(
        vault
            .doc
            .categories
            .iter()
            .any(|category| category.key == EXCHANGE_IMPORT_CATEGORY)
    );
    let Transaction::TradeSpot(trade) = tx else {
        panic!("a spot trade");
    };
    assert_eq!(trade.bought.asset.coingecko_id.as_deref(), Some("solana"));
    assert_eq!(
        crate::cryptotrack::exchange_sync::imported_transactions(&vault.books(), &wallet, 10, 0)
            .total,
        1
    );
}

#[test]
fn the_settings_leave_out_kinds_coins_dates_and_accounts() {
    let (_dir, mut vault, wallet, _) = bybit();
    let settings = ImportSettings {
        coins: vec!["sol".into()],
        since: Some("2023-11-15T00:00:00Z".into()),
        ..ImportSettings::default()
    };
    crate::usecases::cryptotrack::exchange_sync::set_import_settings(&mut vault, &wallet, settings)
        .unwrap();
    let run = crate::usecases::cryptotrack::exchange_sync::sync_exchange(
        &mut vault,
        &wallet,
        vec![
            buy(&wallet, "sol-new", "SOLUSDT", "1700100000000"),
            buy(&wallet, "sol-old", "SOLUSDT", "1690000000000"),
            buy(&wallet, "eth-new", "ETHUSDT", "1700100000000"),
        ],
        ImportSource::Manual,
        now(),
    )
    .unwrap();
    assert_eq!((run.confirmed, run.filtered), (1, 2));

    // Unified not imported at all: fills are left out.
    let mut off =
        crate::cryptotrack::exchange_sync::import_settings(&vault.books(), &wallet).unwrap();
    off.coins.clear();
    off.since = None;
    off.accounts = vec![ImportAccountRoute {
        account: "UNIFIED".into(),
        store_id: None,
    }];
    crate::usecases::cryptotrack::exchange_sync::set_import_settings(&mut vault, &wallet, off)
        .unwrap();
    let run = crate::usecases::cryptotrack::exchange_sync::sync_exchange(
        &mut vault,
        &wallet,
        vec![buy(&wallet, "x", "SOLUSDT", "1700100000000")],
        ImportSource::Manual,
        now(),
    )
    .unwrap();
    assert_eq!((run.confirmed, run.filtered), (0, 1));
}

#[test]
fn a_booked_record_can_go_into_a_chosen_sub_account() {
    let (_dir, mut vault, wallet, _) = bybit();
    let [funding, _, _] = crate::usecases::cryptotrack::exchange_import::ensure_bybit_accounts(
        &mut vault,
        &wallet,
        ACCOUNT_NAMES,
    )
    .unwrap();
    let settings = ImportSettings {
        accounts: vec![ImportAccountRoute {
            account: "unified".into(),
            store_id: Some(funding.clone()),
        }],
        auto_confirm: true,
        ..ImportSettings::default()
    };
    crate::usecases::cryptotrack::exchange_sync::set_import_settings(&mut vault, &wallet, settings)
        .unwrap();
    // The route still decides where a record from an older build's queue
    // is booked.
    crate::usecases::cryptotrack::exchange_import::stage_imports(
        &mut vault,
        vec![buy(&wallet, "a", "SOLUSDT", "1700000000000")],
    )
    .unwrap();
    assert_eq!(
        crate::usecases::cryptotrack::exchange_import::confirm_pending_imports(&mut vault, &[])
            .unwrap(),
        1
    );
    assert!(crate::cryptotrack::exchange_import::pending_imports(&vault.doc).is_empty());
    let Transaction::TradeSpot(trade) = vault.doc.transactions.last().unwrap() else {
        panic!("a spot trade");
    };
    assert_eq!(trade.store_id, funding);

    // A wallet outside this exchange is refused.
    let other = crate::usecases::money::stores::create_store_with(
        &mut vault,
        "Card",
        StoreKind::Bank,
        "#000",
        "",
        None,
        StoreExtras::default(),
    )
    .unwrap()
    .id;
    let bad = ImportSettings {
        accounts: vec![ImportAccountRoute {
            account: "FUND".into(),
            store_id: Some(other),
        }],
        ..ImportSettings::default()
    };
    assert!(
        crate::usecases::cryptotrack::exchange_sync::set_import_settings(&mut vault, &wallet, bad)
            .is_err()
    );
}

#[test]
fn the_live_balance_is_set_once_everything_is_booked() {
    let (_dir, mut vault, wallet, unified) = bybit();
    crate::usecases::cryptotrack::exchange_import::stage_imports(
        &mut vault,
        vec![buy(&wallet, "a", "SOLUSDT", "1700000000000")],
    )
    .unwrap();
    let live = vec![balance("UNIFIED", &[("SOL", "1.5"), ("USDT", "400")])];
    // Something waits for confirmation: it explains the difference.
    assert!(
        crate::usecases::cryptotrack::exchange_sync::reconcile_exchange(
            &mut vault,
            &wallet,
            &live,
            now()
        )
        .unwrap()
        .is_empty()
    );

    crate::usecases::cryptotrack::exchange_import::confirm_pending_imports(&mut vault, &[])
        .unwrap();
    let made = crate::usecases::cryptotrack::exchange_sync::reconcile_exchange(
        &mut vault,
        &wallet,
        &live,
        now(),
    )
    .unwrap();
    assert_eq!(made.len(), 1);
    let holding = |symbol: &str| {
        vault
            .doc
            .holdings
            .iter()
            .find(|holding| holding.store_id == unified && holding.asset.symbol == symbol)
            .map(|holding| holding.quantity)
            .unwrap_or_default()
    };
    assert_eq!(holding("SOL"), Amount::parse("1.5").unwrap());
    assert_eq!(holding("USDT"), Amount::parse("400").unwrap());
    let correction = vault.doc.transactions.last().unwrap();
    assert_eq!(correction.kind(), TxKind::Adjustment);
    assert!(matches!(correction.meta().origin, TxOrigin::Import { .. }));

    // In step now: nothing more to set.
    assert!(
        crate::usecases::cryptotrack::exchange_sync::reconcile_exchange(
            &mut vault,
            &wallet,
            &live,
            now() + 60
        )
        .unwrap()
        .is_empty()
    );

    // Turned off in the settings: the ledger is left alone.
    let mut settings =
        crate::cryptotrack::exchange_sync::import_settings(&vault.books(), &wallet).unwrap();
    settings.auto_reconcile = false;
    crate::usecases::cryptotrack::exchange_sync::set_import_settings(&mut vault, &wallet, settings)
        .unwrap();
    let moved = vec![balance("UNIFIED", &[("SOL", "2"), ("USDT", "400")])];
    assert!(
        crate::usecases::cryptotrack::exchange_sync::reconcile_exchange(
            &mut vault,
            &wallet,
            &moved,
            now() + 120
        )
        .unwrap()
        .is_empty()
    );
}

/// Once the module drives the wallet, the ledger is not corrected towards
/// the exchange any more: the same coins would then be counted both as
/// operations of the module and as a ledger holding.
#[test]
fn a_wallet_on_the_engine_is_not_corrected_into_the_ledger() {
    let (_dir, mut vault, wallet, _) = bybit();
    crate::usecases::cryptotrack::exchange_sync::sync_exchange(
        &mut vault,
        &wallet,
        vec![buy(&wallet, "a", "SOLUSDT", "1700000000000")],
        ImportSource::Manual,
        now(),
    )
    .unwrap();
    assert!(
        crate::usecases::cryptotrack::legacy_bybit::exchange_is_engine_driven(&vault.doc, &wallet)
    );
    let before = vault.doc.transactions.len();
    let live = vec![balance("UNIFIED", &[("SOL", "1.5"), ("USDT", "400")])];
    assert!(
        crate::usecases::cryptotrack::exchange_sync::reconcile_exchange(
            &mut vault,
            &wallet,
            &live,
            now()
        )
        .unwrap()
        .is_empty()
    );
    assert_eq!(
        vault.doc.transactions.len(),
        before,
        "no correction written"
    );
    // Whatever the wallet holds is derived from the module's operations;
    // nothing was written into the ledger to match the exchange.
    assert!(
        !vault
            .doc
            .transactions
            .iter()
            .any(|tx| tx.kind() == TxKind::Adjustment)
    );
}

#[test]
fn a_failed_sync_is_kept_with_its_reason() {
    let (_dir, mut vault, wallet, _) = bybit();
    crate::usecases::cryptotrack::exchange_sync::record_import_error(
        &mut vault,
        &wallet,
        ImportSource::Auto,
        "rate_limited",
        now(),
    )
    .unwrap();
    let runs = crate::cryptotrack::exchange_sync::import_runs(&vault.books(), &wallet);
    assert_eq!(runs[0].error.as_deref(), Some("rate_limited"));
}

#[test]
fn a_stablecoin_without_a_price_is_worth_a_dollar() {
    assert!(crate::exchange::is_usd_stablecoin("asset:USDT"));
    assert!(crate::exchange::is_usd_stablecoin("coin:tether"));
    assert!(!crate::exchange::is_usd_stablecoin("coin:bitcoin"));
    assert!(!crate::exchange::is_usd_stablecoin("fiat:USD"));
}

#[test]
fn a_preview_follows_unsaved_settings_and_keeps_nothing() {
    let (_dir, mut vault, wallet, _) = bybit();
    crate::usecases::cryptotrack::exchange_sync::sync_exchange(
        &mut vault,
        &wallet,
        vec![buy(&wallet, "seen", "SOLUSDT", "1700000000000")],
        ImportSource::Manual,
        now(),
    )
    .unwrap();
    let fills = vec![
        buy(&wallet, "seen", "SOLUSDT", "1700000000000"),
        buy(&wallet, "sol", "SOLUSDT", "1700100000000"),
        buy(&wallet, "eth", "ETHUSDT", "1700100000000"),
    ];
    let draft = ImportSettings {
        coins: vec!["SOL".into()],
        ..ImportSettings::default()
    };
    let preview = crate::cryptotrack::exchange_sync::preview_exchange_sync(
        &vault.books(),
        fills.clone(),
        &draft,
    );
    assert_eq!(
        (
            preview.fetched,
            preview.known,
            preview.filtered,
            preview.new
        ),
        (3, 1, 1, 1),
        "the one already recorded as an operation counts as known"
    );
    assert_eq!(preview.items[0].exec_id, "sol");
    // Not one record kept, the saved settings untouched.
    assert!(crate::cryptotrack::exchange_import::pending_imports(&vault.doc).is_empty());
    assert_eq!(vault.doc.crypto_events.len(), 1);
    assert!(
        crate::cryptotrack::exchange_sync::import_settings(&vault.books(), &wallet)
            .unwrap()
            .coins
            .is_empty()
    );
    // Skipping the unified account in the draft leaves every fill out.
    let skip = ImportSettings {
        accounts: vec![ImportAccountRoute {
            account: "UNIFIED".into(),
            store_id: None,
        }],
        ..ImportSettings::default()
    };
    assert_eq!(
        crate::cryptotrack::exchange_sync::preview_exchange_sync(&vault.books(), fills, &skip).new,
        0
    );
}
