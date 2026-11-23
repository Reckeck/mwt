use std::time::Instant;

use super::*;
use crate::Amount;
use crate::cryptotrack::engine;
use crate::cryptotrack::engine::Doubt;
use crate::cryptotrack::engine::{Inputs, Prices, Quote};
use crate::cryptotrack::prices;
use crate::cryptotrack::service::recompute::day_of;
use crate::cryptotrack::service::recompute::{CryptoAssetTotal, CryptoTotals, PriceIndex};
use crate::cryptotrack::sources::evm::RawAsset;
use crate::cryptotrack::sources::evm::{BalanceSource, RawBalance, RawEvent, RawKind, Uint256};
use crate::error::Error;
use crate::model::chain::Chain;
use crate::model::cryptotrack::CryptoAsset;
use crate::model::cryptotrack::CryptoTx;
use crate::model::cryptotrack::TokenId;
use crate::model::cryptotrack::{
    CryptoAccount, CryptoDirection, CryptoOverride, CryptoPricePoint, CryptoTxKind, TokenVerdict,
};
use crate::model::store::StoreKind;
use crate::model::transaction::{PaymentTx, PriceStamp, Transaction, TxMeta};
use crate::model::vault::VaultDoc;
use crate::service::test_support::empty_vault;
use crate::storage::{SnapshotFormat, encode};
use chrono::NaiveDate;
use std::collections::BTreeSet;

const ME: &str = "0xabcd00000000000000000000000000000000ef01";
const OTHER: &str = "0x2222222222222222222222222222222222222222";
const USDC: &str = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48";
const READ_AT: &str = "2026-03-09T12:00:00Z";
const START: &str = "2026-03-01T00:00:00Z";

fn amount(text: &str) -> Amount {
    Amount::parse(text).expect("a test amount")
}

fn eth() -> CryptoAsset {
    CryptoAsset::Native {
        chain: Chain::Ethereum,
    }
}

fn usdc() -> CryptoAsset {
    CryptoAsset::Token(TokenId::new(Chain::Ethereum, USDC))
}

/// A vault with one followed address and a price for every day of March.
fn followed() -> (tempfile::TempDir, UnlockedVault) {
    let (dir, mut vault) = empty_vault();
    vault.doc.crypto_accounts.push(CryptoAccount::new(
        "cacc-1",
        "Cold EVM",
        Chain::Ethereum,
        ME,
        "2026-02-01T00:00:00Z",
    ));
    for day in 1..=31 {
        for (asset, price) in [(eth(), "2000"), (usdc(), "1")] {
            vault.doc.crypto_prices.push(CryptoPricePoint {
                token_key: asset.key(),
                value_usd: amount(price),
                ts: format!("2026-03-{day:02}T00:00:00Z"),
                source: "test".into(),
            });
        }
    }
    (dir, vault)
}

fn units(text: &str) -> Uint256 {
    Uint256::from_decimal(text).expect("a test quantity")
}

fn native() -> RawAsset {
    RawAsset::Native { decimals: 18 }
}

fn token() -> RawAsset {
    RawAsset::Token {
        contract: USDC.into(),
        symbol: "USDC".into(),
        name: "USD Coin".into(),
        decimals: 6,
        suspect: false,
    }
}

/// One transfer of the history: `index` makes its hash and its day.
fn transfer(index: u64, day: u32, asset: RawAsset, value: Uint256, inbound: bool) -> RawEvent {
    let hash = format!("0x{index:064x}");
    RawEvent {
        id: format!("evm:1:{hash}:tx"),
        chain_id: 1,
        hash,
        kind: RawKind::Transaction,
        block: 21_000_000 + index,
        time: format!("2026-03-{day:02}T10:00:00Z"),
        from: if inbound { OTHER.into() } else { ME.into() },
        to: if inbound { ME.into() } else { OTHER.into() },
        asset,
        value,
        method: None,
        succeeded: true,
        fee: None,
        payload: "{}".into(),
    }
}

fn balance(asset: RawAsset, value: Uint256) -> RawBalance {
    RawBalance {
        asset,
        value,
        source: BalanceSource::Node,
    }
}

fn batch(raw_events: Vec<RawEvent>, balances: Vec<RawBalance>) -> SourceBatch {
    SourceBatch {
        raw_events,
        balances,
        next_cursor: None,
        rejected: Vec::new(),
    }
}

fn request() -> CryptoBatch<'static> {
    CryptoBatch {
        account_id: "cacc-1",
        read_at: READ_AT,
        start: Some(START),
        history_done: true,
        balances_complete: true,
    }
}

fn eth_total(totals: &CryptoTotals) -> CryptoAssetTotal {
    totals
        .assets
        .iter()
        .find(|asset| asset.key == eth().key())
        .cloned()
        .expect("the coin is in the totals")
}

#[test]
fn a_batch_leaves_the_capital_already_counted() {
    // `persist` does not re-derive `doc.holdings` (spec E8), so a batch
    // that changed what the module holds must re-derive them itself:
    // otherwise the wallet shows the new operations beside the old total
    // until the next poll.
    let (_dir, mut vault) = followed();
    let wallet = crate::model::store::Store::new("Cold EVM", StoreKind::Crypto, "#000000");
    let wallet_id = wallet.id.clone();
    vault.doc.stores.push(wallet);
    vault
        .doc
        .stores
        .iter_mut()
        .find(|store| store.id == wallet_id)
        .expect("the wallet is there")
        .crypto_account_id = Some("cacc-1".to_string());

    let seen = batch(
        vec![transfer(1, 5, native(), units("2000000000000000000"), true)],
        vec![balance(native(), units("2000000000000000000"))],
    );
    crate::usecases::cryptotrack::recompute::crypto_apply_batch(&mut vault, &request(), &seen)
        .expect("applied");

    let held = vault
        .doc
        .holdings
        .iter()
        .find(|holding| holding.store_id == wallet_id)
        .expect("the batch left the balance in the holdings");
    assert_eq!(held.quantity, amount("2"));
}

#[test]
fn the_starting_amount_and_the_operations_give_the_network_balance() {
    let (_dir, mut vault) = followed();
    // The module saw 1 ETH arrive; the node says the address holds 3.
    let seen = batch(
        vec![transfer(1, 5, native(), units("1000000000000000000"), true)],
        vec![balance(native(), units("3000000000000000000"))],
    );
    let applied =
        crate::usecases::cryptotrack::recompute::crypto_apply_batch(&mut vault, &request(), &seen)
            .expect("applied");
    assert_eq!(applied.added, 1);
    assert!(applied.corrected.contains(&eth().key()));

    let opening = vault
        .doc
        .crypto_events
        .iter()
        .find(|event| event.id == "copen:cacc-1:ethereum|native")
        .expect("an opening adjustment");
    assert_eq!(opening.legs[0].amount, amount("2"));
    assert_eq!(opening.legs[0].direction, CryptoDirection::In);
    assert_eq!(opening.time, "2026-03-01T00:00:00Z");

    let totals = crate::cryptotrack::service::recompute::crypto_totals(&vault.books(), READ_AT);
    let coin = eth_total(&totals);
    assert_eq!(
        coin.quantity,
        amount("3"),
        "the module agrees with the node"
    );
    assert_eq!(coin.value_usd, Some(amount("6000")));
    assert!(
        coin.imprecise,
        "an opening holding is a guess about its cost"
    );
    assert!(coin.doubts.contains(&Doubt::OpeningAtMarket));
}

#[test]
fn a_correction_survives_a_resync_and_the_balance_still_matches() {
    let (_dir, mut vault) = followed();
    let seen = batch(
        vec![transfer(1, 5, native(), units("1000000000000000000"), true)],
        vec![balance(native(), units("3000000000000000000"))],
    );
    crate::usecases::cryptotrack::recompute::crypto_apply_batch(&mut vault, &request(), &seen)
        .expect("applied");
    let event_id =
        "cev:cacc-1:ethereum:0x0000000000000000000000000000000000000000000000000000000000000001";
    assert!(
        vault
            .doc
            .crypto_events
            .iter()
            .any(|event| event.id == event_id)
    );

    // The user says the arrival was a reward, not a purchase.
    vault.doc.crypto_overrides.push(CryptoOverride {
        id: "cov-1".into(),
        event_id: event_id.into(),
        kind: Some(CryptoTxKind::Claim),
        comment: Some("staking reward".into()),
        categories: Vec::new(),
        flow_id: None,
        contact_id: None,
        not_exchange: false,
        hidden: false,
        fiat_tx_id: None,
        updated_at: "2026-03-10T00:00:00Z".into(),
    });
    let before = crate::cryptotrack::service::recompute::crypto_totals(&vault.books(), READ_AT);
    assert_eq!(eth_total(&before).income_usd, amount("2000"));

    // The same pages arrive again: the events are rewritten, the
    // correction is not.
    let applied =
        crate::usecases::cryptotrack::recompute::crypto_apply_batch(&mut vault, &request(), &seen)
            .expect("resynced");
    assert_eq!(applied.added, 0, "nothing new");
    assert_eq!(applied.rewritten, 1);
    assert_eq!(vault.doc.crypto_overrides.len(), 1);
    assert_eq!(
        vault.doc.crypto_overrides[0].comment.as_deref(),
        Some("staking reward")
    );
    let after = crate::cryptotrack::service::recompute::crypto_totals(&vault.books(), READ_AT);
    assert_eq!(eth_total(&after).income_usd, amount("2000"));
    assert_eq!(eth_total(&after).quantity, amount("3"));
    assert_eq!(before.value_usd, after.value_usd);
    // And a second opening adjustment was not stacked on the first.
    assert_eq!(
        vault
            .doc
            .crypto_events
            .iter()
            .filter(|event| event.id.starts_with("copen:"))
            .count(),
        1
    );
}

#[test]
fn a_second_run_changes_no_total_whatever_order_the_batches_came_in() {
    // A small history: arrivals, a departure, a token, in several pages.
    let facts = vec![
        transfer(1, 2, native(), units("2000000000000000000"), true),
        transfer(2, 3, native(), units("500000000000000000"), false),
        transfer(3, 4, token(), units("1500000000"), true),
        transfer(4, 5, token(), units("500000000"), false),
        transfer(5, 6, native(), units("250000000000000000"), true),
        transfer(6, 7, token(), units("100000000"), true),
    ];
    let balances = vec![
        balance(native(), units("1750000000000000000")),
        balance(token(), units("1100000000")),
    ];

    let straight = {
        let (_dir, mut vault) = followed();
        crate::usecases::cryptotrack::recompute::crypto_apply_batch(
            &mut vault,
            &request(),
            &batch(facts.clone(), balances.clone()),
        )
        .expect("applied");
        crate::cryptotrack::service::recompute::crypto_totals(&vault.books(), READ_AT)
    };
    // No adjustment was needed: the operations already add up.
    assert!(!straight.imprecise, "{:?}", straight.assets);

    // Every page order, in two batches, then the whole thing again.
    let mut seed = 0x5eedu64;
    for _ in 0..20 {
        let mut shuffled = facts.clone();
        for index in (1..shuffled.len()).rev() {
            seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            let pick = (seed >> 33) as usize % (index + 1);
            shuffled.swap(index, pick);
        }
        let (_dir, mut vault) = followed();
        let (first, second) = shuffled.split_at(shuffled.len() / 2);
        let mut page = request();
        page.history_done = false;
        crate::usecases::cryptotrack::recompute::crypto_apply_batch(
            &mut vault,
            &page,
            &batch(first.to_vec(), Vec::new()),
        )
        .expect("first page");
        crate::usecases::cryptotrack::recompute::crypto_apply_batch(
            &mut vault,
            &request(),
            &batch(second.to_vec(), balances.clone()),
        )
        .expect("second page");
        assert_eq!(
            crate::cryptotrack::service::recompute::crypto_totals(&vault.books(), READ_AT),
            straight,
            "order {shuffled:?}"
        );

        // A second run over the same facts changes nothing.
        crate::usecases::cryptotrack::recompute::crypto_apply_batch(
            &mut vault,
            &request(),
            &batch(shuffled.clone(), balances.clone()),
        )
        .expect("again");
        assert_eq!(
            crate::cryptotrack::service::recompute::crypto_totals(&vault.books(), READ_AT),
            straight,
            "second run"
        );
    }
}

#[test]
fn two_devices_hold_the_same_bytes_for_the_same_facts() {
    let facts = vec![
        transfer(1, 2, native(), units("2000000000000000000"), true),
        transfer(2, 3, token(), units("1500000000"), true),
        transfer(3, 4, native(), units("500000000000000000"), false),
    ];
    let balances = vec![
        balance(native(), units("1500000000000000000")),
        balance(token(), units("1500000000")),
    ];
    let bytes_of = |pages: Vec<Vec<RawEvent>>| {
        let (_dir, mut vault) = followed();
        let last = pages.len() - 1;
        for (index, page) in pages.into_iter().enumerate() {
            let mut step = request();
            step.history_done = index == last;
            let with_balances = if index == last {
                balances.clone()
            } else {
                Vec::new()
            };
            crate::usecases::cryptotrack::recompute::crypto_apply_batch(
                &mut vault,
                &step,
                &batch(page, with_balances),
            )
            .expect("applied");
        }
        serde_json::to_vec(&vault.doc.crypto_events).expect("events serialize")
    };
    let one_device = bytes_of(vec![facts.clone()]);
    let other_device = bytes_of(vec![
        vec![facts[2].clone()],
        vec![facts[1].clone(), facts[0].clone()],
    ]);
    assert_eq!(
        String::from_utf8_lossy(&one_device),
        String::from_utf8_lossy(&other_device)
    );
}

/// Counts the saves a vault asks for.
struct CountSaves(std::sync::atomic::AtomicUsize);

impl crate::service::SaveHook for CountSaves {
    fn dirty(&self) {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

/// A batch of `tokens` junk airdrops, one fact each.
fn junk_batch(tokens: u64) -> SourceBatch {
    let facts = (1..=tokens)
        .map(|index| {
            let junk = RawAsset::Token {
                contract: format!("0x{index:040x}"),
                symbol: format!("JUNK{index}"),
                name: "Junk".into(),
                decimals: 18,
                suspect: true,
            };
            transfer(index, 5, junk, units("1000"), true)
        })
        .collect();
    batch(facts, Vec::new())
}

/// A followed vault whose saves are counted.
fn counted_vault() -> (
    tempfile::TempDir,
    crate::service::UnlockedVault,
    std::sync::Arc<CountSaves>,
) {
    let (dir, mut vault) = followed();
    let saves = std::sync::Arc::new(CountSaves(std::sync::atomic::AtomicUsize::new(0)));
    vault.set_write_behind(Some(saves.clone()));
    (dir, vault, saves)
}

/// Saves one batch of `tokens` junk airdrops costs.
fn saves_for_a_batch_of(tokens: u64) -> usize {
    let (_dir, mut vault, saves) = counted_vault();
    crate::usecases::cryptotrack::recompute::crypto_apply_batch(
        &mut vault,
        &request(),
        &junk_batch(tokens),
    )
    .expect("applied");
    assert_eq!(vault.doc.crypto_tokens.len() as u64, tokens);
    saves.0.load(std::sync::atomic::Ordering::SeqCst)
}

#[test]
fn a_pass_over_many_new_tokens_saves_once() {
    assert_eq!(saves_for_a_batch_of(50), 1, "50 new tokens: one wake");
}

#[test]
fn an_unchanged_pass_does_not_save() {
    let (_dir, mut vault, saves) = counted_vault();
    let pass = junk_batch(50);
    crate::usecases::cryptotrack::recompute::crypto_apply_batch(&mut vault, &request(), &pass)
        .expect("first");
    let after_first = saves.0.load(std::sync::atomic::Ordering::SeqCst);
    let revision = vault.session.revision();
    crate::usecases::cryptotrack::recompute::crypto_apply_batch(&mut vault, &request(), &pass)
        .expect("second");
    assert_eq!(after_first, 1);
    assert_eq!(
        saves.0.load(std::sync::atomic::Ordering::SeqCst),
        after_first,
        "the identical pass wakes the writer 0 more times"
    );
    assert_eq!(vault.session.revision(), revision, "and bumps nothing");
}

#[test]
fn a_batch_saves_once_however_many_new_tokens_it_met() {
    let one = saves_for_a_batch_of(1);
    assert_eq!(
        saves_for_a_batch_of(40),
        one,
        "the saves must not grow with the number of tokens"
    );
}

#[test]
fn a_quarantined_token_is_outside_the_totals_until_it_is_confirmed() {
    let (_dir, mut vault) = followed();
    let scam = RawAsset::Token {
        contract: "0x00000000000000000000000000000000deadbeef".into(),
        symbol: "USDC".into(),
        name: "USD Coin".into(),
        decimals: 6,
        suspect: true,
    };
    let scam_id = TokenId::new(
        Chain::Ethereum,
        "0x00000000000000000000000000000000deadbeef",
    );
    vault.doc.crypto_prices.push(CryptoPricePoint {
        token_key: CryptoAsset::Token(scam_id.clone()).key(),
        value_usd: amount("1000000"),
        ts: "2026-03-05T00:00:00Z".into(),
        source: "test".into(),
    });
    let seen = batch(
        vec![transfer(1, 5, scam, units("1000000000"), true)],
        Vec::new(),
    );
    let mut page = request();
    page.history_done = false;
    crate::usecases::cryptotrack::recompute::crypto_apply_batch(&mut vault, &page, &seen)
        .expect("applied");
    let totals = crate::cryptotrack::service::recompute::crypto_totals(&vault.books(), READ_AT);
    assert_eq!(
        totals.value_usd,
        Amount::ZERO,
        "a scam airdrop is worth nothing here"
    );
    assert!(totals.assets.is_empty());

    // The user takes it in: now it counts, with the same events.
    vault
        .doc
        .crypto_tokens
        .iter_mut()
        .find(|record| record.id() == scam_id)
        .expect("the token was registered")
        .verdict = Some(TokenVerdict::Confirmed);
    let totals = crate::cryptotrack::service::recompute::crypto_totals(&vault.books(), READ_AT);
    assert_eq!(totals.assets.len(), 1);
    assert_eq!(totals.value_usd, amount("1000000000"));
}

#[test]
fn a_linked_fiat_operation_sets_the_cost_basis_above_the_market() {
    let (_dir, mut vault) = followed();
    let seen = batch(
        vec![transfer(1, 5, native(), units("1000000000000000000"), true)],
        Vec::new(),
    );
    let mut page = request();
    page.history_done = false;
    crate::usecases::cryptotrack::recompute::crypto_apply_batch(&mut vault, &page, &seen)
        .expect("applied");
    let event_id =
        "cev:cacc-1:ethereum:0x0000000000000000000000000000000000000000000000000000000000000001";

    // Without a chain, the market price of the day is the basis.
    assert_eq!(
        eth_total(&crate::cryptotrack::service::recompute::crypto_totals(
            &vault.books(),
            READ_AT
        ))
        .average_usd,
        Some(amount("2000"))
    );

    // The user says this arrival is the other half of a payment: 2 400
    // dollars left the bank for it.
    let mut meta = TxMeta::new("bought ETH");
    meta.id = "tx-1".into();
    meta.timestamp = "2026-03-05T09:00:00Z".into();
    meta.flow_id = Some("flow-1".into());
    vault.doc.transactions.push(Transaction::Payment(PaymentTx {
        meta,
        store_id: "store-1".into(),
        asset: crate::model::store::AssetRef::fiat("usd", "US Dollar"),
        amount: amount("2400"),
        fee: None,
        to: None,
        details: crate::model::transaction::KindDetails::default(),
        price: PriceStamp {
            price_usd: Some(amount("1")),
            ..PriceStamp::default()
        },
    }));
    vault.doc.crypto_overrides.push(CryptoOverride {
        id: "cov-1".into(),
        event_id: event_id.into(),
        kind: None,
        comment: None,
        categories: Vec::new(),
        flow_id: Some("flow-1".into()),
        contact_id: None,
        not_exchange: false,
        hidden: false,
        fiat_tx_id: None,
        updated_at: "2026-03-10T00:00:00Z".into(),
    });
    let coin = eth_total(&crate::cryptotrack::service::recompute::crypto_totals(
        &vault.books(),
        READ_AT,
    ));
    assert_eq!(
        coin.average_usd,
        Some(amount("2400")),
        "the user's own number beats the market quote"
    );
    assert!(coin.linked_basis);
    assert_eq!(coin.cost_usd, amount("2400"));
}

#[test]
fn a_chain_paid_in_another_currency_is_converted_at_the_rate_of_the_day() {
    let (_dir, mut vault) = followed();
    let seen = batch(
        vec![transfer(1, 5, native(), units("1000000000000000000"), true)],
        Vec::new(),
    );
    let mut page = request();
    page.history_done = false;
    crate::usecases::cryptotrack::recompute::crypto_apply_batch(&mut vault, &page, &seen)
        .expect("applied");
    let event_id =
        "cev:cacc-1:ethereum:0x0000000000000000000000000000000000000000000000000000000000000001";

    // 200 000 roubles at 100 to the dollar: the chain says this coin cost
    // 2 000 dollars, and no price of the day is consulted.
    crate::usecases::money::rates::apply_fx(
        &mut vault,
        &[crate::model::vault::FxPoint {
            currency: "rub".into(),
            per_usd: amount("100"),
            ts: "2026-03-05T00:00:00Z".into(),
            source: "test".into(),
        }],
    );
    let mut meta = TxMeta::new("bought ETH with roubles");
    meta.id = "tx-1".into();
    meta.timestamp = "2026-03-05T09:00:00Z".into();
    meta.flow_id = Some("flow-1".into());
    vault.doc.transactions.push(Transaction::Payment(PaymentTx {
        meta,
        store_id: "store-1".into(),
        asset: crate::model::store::AssetRef::fiat("rub", "Rouble"),
        amount: amount("200000"),
        fee: None,
        to: None,
        details: crate::model::transaction::KindDetails::default(),
        price: PriceStamp::default(),
    }));
    vault.doc.crypto_overrides.push(CryptoOverride {
        id: "cov-1".into(),
        event_id: event_id.into(),
        kind: None,
        comment: None,
        categories: Vec::new(),
        flow_id: Some("flow-1".into()),
        contact_id: None,
        not_exchange: false,
        hidden: false,
        fiat_tx_id: None,
        updated_at: "2026-03-10T00:00:00Z".into(),
    });
    let coin = eth_total(&crate::cryptotrack::service::recompute::crypto_totals(
        &vault.books(),
        READ_AT,
    ));
    assert_eq!(coin.average_usd, Some(amount("2000")));
    assert!(coin.linked_basis);
}

#[test]
fn a_declared_basis_is_used_for_what_was_held_before_the_start_date() {
    let (_dir, mut vault) = followed();
    let seen = batch(
        vec![transfer(1, 5, token(), units("100000000"), true)],
        vec![balance(token(), units("1100000000"))],
    );
    crate::usecases::cryptotrack::recompute::crypto_apply_batch(&mut vault, &request(), &seen)
        .expect("applied");
    let token_id = TokenId::new(Chain::Ethereum, USDC);
    let before = crate::cryptotrack::service::recompute::crypto_totals(&vault.books(), READ_AT);
    let held = before
        .assets
        .iter()
        .find(|asset| asset.key == usdc().key())
        .expect("the token");
    assert_eq!(held.quantity, amount("1100"));
    assert!(held.doubts.contains(&Doubt::OpeningAtMarket));

    // The user says the old thousand cost 0.95 each.
    crate::usecases::cryptotrack::tokens::crypto_set_token_basis(
        &mut vault,
        &token_id,
        Some(amount("0.95")),
    )
    .expect("a basis");
    let after = crate::cryptotrack::service::recompute::crypto_totals(&vault.books(), READ_AT);
    let held = after
        .assets
        .iter()
        .find(|asset| asset.key == usdc().key())
        .expect("the token");
    // 1 000 before the start at 0.95 plus 100 seen arriving at 1.00.
    assert_eq!(held.cost_usd, amount("1050"));
    assert!(held.linked_basis);
    assert!(!held.doubts.contains(&Doubt::OpeningAtMarket));
}

#[test]
fn the_snapshots_of_a_day_are_written_once_and_rewritten_in_place() {
    let (_dir, mut vault) = followed();
    let seen = batch(
        vec![transfer(1, 5, native(), units("1000000000000000000"), true)],
        Vec::new(),
    );
    let mut page = request();
    page.history_done = false;
    crate::usecases::cryptotrack::recompute::crypto_apply_batch(&mut vault, &page, &seen)
        .expect("applied");
    // One for the account, one for the module.
    assert_eq!(vault.doc.crypto_snapshots.len(), 2);
    let module = vault
        .doc
        .crypto_snapshots
        .iter()
        .find(|record| record.account_id.is_none())
        .expect("the module's own");
    assert_eq!(module.value_usd, amount("2000.00"));

    let same_day = crate::usecases::cryptotrack::recompute::crypto_snapshot(
        &mut vault,
        "2026-03-09T23:00:00Z",
    )
    .expect("again");
    assert_eq!(same_day.len(), 2);
    assert_eq!(vault.doc.crypto_snapshots.len(), 2, "one record per day");
    let next_day = crate::usecases::cryptotrack::recompute::crypto_snapshot(
        &mut vault,
        "2026-03-10T10:00:00Z",
    )
    .expect("later");
    assert_eq!(next_day.len(), 2);
    assert_eq!(vault.doc.crypto_snapshots.len(), 4);
}

#[test]
fn a_second_reading_on_the_same_day_saves_nothing() {
    let (_dir, mut vault) = followed();
    let seen = batch(
        vec![transfer(1, 5, native(), units("1000000000000000000"), true)],
        Vec::new(),
    );
    crate::usecases::cryptotrack::recompute::crypto_apply_batch(&mut vault, &request(), &seen)
        .expect("applied");
    crate::usecases::cryptotrack::recompute::crypto_snapshot(&mut vault, "2026-03-09T08:00:00Z")
        .expect("first reading");
    let revision = vault.session.revision();
    crate::usecases::cryptotrack::recompute::crypto_snapshot(&mut vault, "2026-03-09T21:00:00Z")
        .expect("later reading");
    assert_eq!(vault.session.revision(), revision, "only `at` differs");
}

#[test]
fn the_price_index_answers_what_the_cache_lookup_answers() {
    let (_dir, mut vault) = followed();
    // A zero (which E4 ignores), two days of one asset, a gap, and a
    // second asset that has nothing at all.
    vault.doc.crypto_prices.push(CryptoPricePoint {
        token_key: eth().key(),
        value_usd: Amount::ZERO,
        ts: "2026-04-01T00:00:00Z".into(),
        source: "test".into(),
    });
    vault.doc.crypto_prices.push(CryptoPricePoint {
        token_key: eth().key(),
        value_usd: amount("2500"),
        ts: "2026-04-05T00:00:00Z".into(),
        source: "test".into(),
    });
    let nothing = CryptoAsset::Token(TokenId::new(
        Chain::Ethereum,
        "0x00000000000000000000000000000000deadbeef",
    ));
    let index = PriceIndex::of(&vault.doc);
    for asset in [eth(), usdc(), nothing] {
        for day in [
            "2026-02-28",
            "2026-03-01",
            "2026-03-15",
            "2026-04-01",
            "2026-04-05",
            "2026-04-06",
        ] {
            let date = NaiveDate::parse_from_str(day, "%Y-%m-%d").expect("a date");
            let want = prices::lookup(&vault.doc, &asset, date);
            assert_eq!(
                index.at(&asset, date),
                (want.value, want.stale),
                "{} on {day}",
                asset.key()
            );
        }
    }
}

#[test]
fn an_unknown_account_is_refused() {
    let (_dir, mut vault) = followed();
    let mut missing = request();
    missing.account_id = "nope";
    assert!(matches!(
        crate::usecases::cryptotrack::recompute::crypto_apply_batch(
            &mut vault,
            &missing,
            &batch(Vec::new(), Vec::new())
        ),
        Err(Error::NotFound(_))
    ));
}

/// A `Prices` that asks E4's cache the way a fold would without an index:
/// one scan of `crypto_prices` per leg.
struct Scanning<'doc>(&'doc VaultDoc);

impl Prices for Scanning<'_> {
    fn unit_usd(&self, asset: &CryptoAsset, time: &str, _event_id: &str) -> Quote {
        let Some(date) = day_of(time) else {
            return Quote::default();
        };
        let found = prices::lookup(self.0, asset, date);
        Quote {
            value: found.value,
            stale: found.stale,
            linked: false,
        }
    }
}

/// What 1 000, 10 000 and 50 000 events cost: the fold, the sync stamp
/// and the save. Not part of the gate; run it on purpose, and in release
/// (a debug build is some ten times slower here):
///
/// ```text
/// cargo test --release -p trade-core crypto_engine_volume -- --ignored --nocapture
/// ```
#[test]
#[ignore = "a measurement, not a check: run it with --ignored --nocapture"]
fn crypto_engine_volume() {
    println!(
        "{:>7} {:>10} {:>10} {:>10} {:>10} {:>12} {:>11} {:>11}",
        "events", "fold", "stamp", "persist", "file KiB", "pack KiB", "fold/5k px", "scan/5k px"
    );
    for count in [1_000usize, 10_000, 50_000] {
        let (_dir, mut vault) = followed();
        let facts: Vec<RawEvent> = (0..count)
            .map(|index| {
                transfer(
                    index as u64 + 1,
                    (index % 28 + 1) as u32,
                    if index % 2 == 0 { native() } else { token() },
                    units("1000000000000000"),
                    index % 3 != 0,
                )
            })
            .collect();
        let made = engine::normalize(
            &facts,
            engine::Followed {
                account_id: "cacc-1",
                chain: Chain::Ethereum,
                address: ME,
            },
        );
        vault.doc.crypto_events = made.events;
        assert_eq!(vault.doc.crypto_events.len(), count);

        let started = Instant::now();
        let totals = crate::cryptotrack::service::recompute::crypto_totals(&vault.books(), READ_AT);
        let fold = started.elapsed();
        assert_eq!(totals.counted, count);

        // The same fold against a price cache of the size a few years of
        // daily history really has, once through the index and once
        // through E4's scan, which is what the engine would do without
        // one.
        for day in 0..5_000u32 {
            let date = NaiveDate::from_ymd_opt(2020, 1, 1)
                .expect("a date")
                .checked_add_days(chrono::Days::new(u64::from(day)))
                .expect("a later date");
            vault.doc.crypto_prices.push(CryptoPricePoint {
                token_key: eth().key(),
                value_usd: amount("2000"),
                ts: prices::date_ts(date),
                source: "test".into(),
            });
        }
        let started = Instant::now();
        let fat = crate::cryptotrack::service::recompute::crypto_totals(&vault.books(), READ_AT);
        let indexed = started.elapsed();
        assert_eq!(fat.counted, count);
        let own: BTreeSet<String> = [ME.to_owned()].into_iter().collect();
        let counts = |_asset: &CryptoAsset| true;
        let scanning = Scanning(&vault.doc);
        let views = engine::overrides::apply(&vault.doc.crypto_events, &[]);
        let started = Instant::now();
        let scanned = engine::fold(
            &views,
            &Inputs {
                own: &own,
                prices: &scanning,
                counts: &counts,
            },
        );
        let scan = started.elapsed();
        assert_eq!(scanned.counted, count);
        vault.doc.crypto_prices.truncate(62);

        vault.track_changes().expect("the first pass remembers");
        let started = Instant::now();
        vault.track_changes().expect("the diff every save pays");
        let stamp = started.elapsed();

        let started = Instant::now();
        vault.persist_now().expect("saved");
        let persist = started.elapsed();

        let file = std::fs::metadata(&vault.path).expect("a file").len();
        let pack = encode(&vault.doc, SnapshotFormat::Pack)
            .expect("packed")
            .len();
        println!(
            "{count:>7} {:>7.0?} {:>7.0?} {:>7.0?} {:>10} {:>12} {:>9.0?} {:>9.0?}",
            fold,
            stamp,
            persist,
            file / 1024,
            pack / 1024,
            indexed,
            scan
        );
    }
}

// --- one transaction, facts spread over several batches ---------------

const ROUTER: &str = "0x3333333333333333333333333333333333333333";
const SWAP_HASH: &str = "0xaaaa000000000000000000000000000000000000000000000000000000000001";

fn piece(
    kind: RawKind,
    tail: &str,
    from: &str,
    to: &str,
    asset: RawAsset,
    value: Uint256,
) -> RawEvent {
    RawEvent {
        id: format!("evm:1:{SWAP_HASH}:{tail}"),
        chain_id: 1,
        hash: SWAP_HASH.into(),
        kind,
        block: 21_000_000,
        time: "2026-03-05T10:00:00Z".into(),
        from: from.into(),
        to: to.into(),
        asset,
        value,
        method: None,
        succeeded: true,
        fee: None,
        payload: "{}".into(),
    }
}

/// The `transactions` list: the call itself, with its fee and its value.
fn call_row(value: &str) -> RawEvent {
    let mut row = piece(
        RawKind::Transaction,
        "tx",
        ME,
        ROUTER,
        native(),
        units(value),
    );
    row.method = Some("0x04e45aaf".into());
    row.fee = Some(units("2100000000000000"));
    row
}

fn token_out() -> RawEvent {
    piece(
        RawKind::TokenTransfer,
        "7",
        ME,
        ROUTER,
        token(),
        units("1500000000"),
    )
}

fn token_in() -> RawEvent {
    piece(
        RawKind::TokenTransfer,
        "8",
        ROUTER,
        ME,
        token(),
        units("1800000000"),
    )
}

fn apply_pages(vault: &mut UnlockedVault, pages: &[Vec<RawEvent>]) {
    for page in pages {
        let mut step = request();
        step.history_done = false;
        crate::usecases::cryptotrack::recompute::crypto_apply_batch(
            vault,
            &step,
            &batch(page.clone(), Vec::new()),
        )
        .expect("a page applied");
    }
}

fn only_event(vault: &UnlockedVault) -> CryptoTx {
    assert_eq!(vault.doc.crypto_events.len(), 1, "one operation");
    vault.doc.crypto_events[0].clone()
}

fn leg_of(event: &CryptoTx, direction: CryptoDirection, asset: &CryptoAsset) -> Option<Amount> {
    event
        .legs
        .iter()
        .find(|leg| leg.direction == direction && leg.asset == *asset)
        .map(|leg| leg.amount)
}

#[test]
fn a_token_send_keeps_its_gas_when_the_lists_come_in_two_batches() {
    let (_dir, mut vault) = followed();
    apply_pages(&mut vault, &[vec![call_row("0")], vec![token_out()]]);
    let event = only_event(&vault);
    assert_eq!(
        leg_of(&event, CryptoDirection::Out, &usdc()),
        Some(amount("1500"))
    );
    assert_eq!(
        event.gas.as_ref().map(|fee| fee.amount),
        Some(amount("0.0021"))
    );
    assert_eq!(event.kind, CryptoTxKind::Transfer);
    assert_eq!(event.method.as_deref(), Some("0x04e45aaf"));
}

#[test]
fn a_swap_for_eth_value_keeps_both_legs_over_three_batches() {
    let (_dir, mut vault) = followed();
    let internal = piece(
        RawKind::InternalTransfer,
        "i0",
        ROUTER,
        ME,
        native(),
        units("1000000000000000"),
    );
    apply_pages(
        &mut vault,
        &[
            vec![call_row("500000000000000000")],
            vec![token_in()],
            vec![internal],
        ],
    );
    let event = only_event(&vault);
    assert_eq!(event.kind, CryptoTxKind::Swap);
    assert_eq!(
        leg_of(&event, CryptoDirection::Out, &eth()),
        Some(amount("0.5"))
    );
    assert_eq!(
        leg_of(&event, CryptoDirection::In, &usdc()),
        Some(amount("1800"))
    );
    assert_eq!(
        leg_of(&event, CryptoDirection::In, &eth()),
        Some(amount("0.001"))
    );
    assert!(event.gas.is_some());
}

#[test]
fn a_token_to_eth_swap_keeps_the_token_leg_when_the_internal_list_follows() {
    let (_dir, mut vault) = followed();
    let internal = piece(
        RawKind::InternalTransfer,
        "i0",
        ROUTER,
        ME,
        native(),
        units("700000000000000000"),
    );
    apply_pages(
        &mut vault,
        &[vec![call_row("0")], vec![token_out()], vec![internal]],
    );
    let event = only_event(&vault);
    assert_eq!(event.kind, CryptoTxKind::Swap);
    assert_eq!(
        leg_of(&event, CryptoDirection::Out, &usdc()),
        Some(amount("1500"))
    );
    assert_eq!(
        leg_of(&event, CryptoDirection::In, &eth()),
        Some(amount("0.7"))
    );
    assert!(event.gas.is_some());
}

#[test]
fn the_batch_order_does_not_change_the_operation() {
    let pieces = [
        vec![call_row("500000000000000000")],
        vec![token_in()],
        vec![piece(
            RawKind::InternalTransfer,
            "i0",
            ROUTER,
            ME,
            native(),
            units("1000000000000000"),
        )],
    ];
    let reference = {
        let (_dir, mut vault) = followed();
        apply_pages(&mut vault, &[pieces.concat()]);
        only_event(&vault)
    };
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let (_dir, mut vault) = followed();
        let pages: Vec<Vec<RawEvent>> = order.iter().map(|index| pieces[*index].clone()).collect();
        apply_pages(&mut vault, &pages);
        let event = only_event(&vault);
        assert_eq!(event.legs.len(), reference.legs.len(), "order {order:?}");
        for leg in &reference.legs {
            assert_eq!(
                leg_of(&event, leg.direction, &leg.asset),
                Some(leg.amount),
                "order {order:?}"
            );
        }
        assert_eq!(event.gas, reference.gas, "order {order:?}");
        assert_eq!(event.kind, reference.kind, "order {order:?}");
        assert_eq!(event.method, reference.method, "order {order:?}");
        assert_eq!(event.from, reference.from, "order {order:?}");
    }
}

#[test]
fn reading_the_same_batches_again_does_not_grow_the_operation() {
    let (_dir, mut vault) = followed();
    let pages = [vec![call_row("500000000000000000")], vec![token_in()]];
    apply_pages(&mut vault, &pages);
    let once = only_event(&vault);
    apply_pages(&mut vault, &pages);
    apply_pages(&mut vault, &[pages.concat()]);
    // A refresh that cuts the same facts at another place.
    apply_pages(
        &mut vault,
        &[vec![token_in(), call_row("500000000000000000")]],
    );
    assert_eq!(only_event(&vault), once);
    assert_eq!(once.legs.len(), 2);
}

#[test]
fn a_transaction_straddling_the_batch_boundary_keeps_every_leg() {
    // `BATCH_EVENTS` cuts the facts of one transaction: the first batch
    // ends after the sent token, the second starts with the received one.
    let (_dir, mut vault) = followed();
    apply_pages(
        &mut vault,
        &[vec![call_row("0"), token_out()], vec![token_in()]],
    );
    let event = only_event(&vault);
    assert_eq!(event.kind, CryptoTxKind::Swap);
    assert_eq!(
        leg_of(&event, CryptoDirection::Out, &usdc()),
        Some(amount("1500"))
    );
    assert_eq!(
        leg_of(&event, CryptoDirection::In, &usdc()),
        Some(amount("1800"))
    );
    assert!(event.gas.is_some());
}
