//! Tests of the losses area: the old inline tests (names and bodies unchanged),
//! then the pure functions on a bare document, the L5 pin and the delegate pins.

use super::losses::*;
use crate::Amount;
use crate::calendar::month_index;
use crate::error::Error;
use crate::model::chain::Chain;
use crate::model::store::{AssetRef, StoreKind};
use crate::model::transaction::AccountRef;
use crate::model::transaction::{ConvertSide, ConvertTx, Leg, PriceStamp, Transaction, TxMeta};
use crate::model::vault::{FxPoint, PricePoint, VaultDoc};
use crate::service::test_support::*;
use crate::service::{Report, ReportFilter, ReportKind, SaveHook, UnlockedVault};

fn rub() -> AssetRef {
    AssetRef::fiat("RUB", "Ruble")
}

fn thb() -> AssetRef {
    AssetRef::fiat("THB", "Baht")
}

fn usdt() -> AssetRef {
    AssetRef::crypto("USDT", "Tether", Some("tether"), Chain::Tron)
}

fn convert(
    from: (&str, AssetRef, &str),
    to: (&str, AssetRef, &str),
    rate: Option<&str>,
    ts: &str,
) -> Transaction {
    let side = |(store, asset, sum): (&str, AssetRef, &str)| ConvertSide {
        account: AccountRef::Store(store.into()),
        asset,
        amount: amount(sum),
        price: PriceStamp::default(),
    };
    let mut meta = TxMeta::new("exchange");
    meta.timestamp = ts.into();
    Transaction::Convert(ConvertTx {
        meta,
        from: side(from),
        to: side(to),
        fee: None,
        rate_override: rate.map(amount),
    })
}

/// USD base; 100 RUB and 35 THB per USD; USDT at 1.
fn setup() -> (tempfile::TempDir, UnlockedVault, String, String, String) {
    let (dir, mut vault) = empty_vault();
    vault.set_base_currency("usd");
    let card = store(&mut vault, "RUB card", StoreKind::Bank, None);
    let wallet = store(&mut vault, "Wallet", StoreKind::Crypto, None);
    let baht = store(&mut vault, "THB card", StoreKind::Bank, None);
    let mut seed = income_of(&card, rub(), "20000", "0.01", "salary");
    seed.meta_mut().timestamp = "2026-01-01T00:00:00Z".into();
    vault.add_transaction(seed).unwrap();
    vault.apply_fx(&[
        FxPoint {
            currency: "rub".into(),
            per_usd: amount("100"),
            ts: "2026-01-01T00:00:00Z".into(),
            source: "test".into(),
        },
        FxPoint {
            currency: "thb".into(),
            per_usd: amount("35"),
            ts: "2026-01-01T00:00:00Z".into(),
            source: "test".into(),
        },
    ]);
    vault.apply_prices(&[PricePoint {
        asset_key: usdt().key(),
        value_usd: amount("1"),
        ts: "2026-01-01T00:00:00Z".into(),
        source: "test".into(),
        change_24h_pct: None,
    }]);
    (dir, vault, card, wallet, baht)
}

#[test]
fn a_convert_at_a_worse_rate_than_the_market_loses_the_spread() {
    let (_dir, mut vault, card, wallet, _) = setup();
    let saved = vault
        .add_transaction(convert(
            (&card, rub(), "10000"),
            (&wallet, usdt(), "95"),
            Some("105.26"),
            "2026-02-01T10:00:00Z",
        ))
        .unwrap();
    let loss = vault.tx_loss(&saved.meta().id).unwrap();
    assert_eq!(loss.slippage, amount("5"));
    assert_eq!(loss.total, amount("5"));
    assert!(loss.complete);
    assert!(!loss.internal);
}

#[test]
fn the_preview_of_a_convert_shows_the_overpay_over_the_market() {
    let (_dir, vault, card, wallet, _) = setup();
    let draft = convert(
        (&card, rub(), "10000"),
        (&wallet, usdt(), "95"),
        None,
        "2026-02-01T10:00:00Z",
    );
    let preview = vault.convert_preview(&draft).unwrap();
    assert_eq!(preview.market_price, amount("100"));
    assert_eq!(preview.overpay, amount("500"));
    assert_eq!(preview.overpay_base, amount("5"));
    assert_eq!(preview.overpay_pct, amount("5"));
}

#[test]
fn an_internal_convert_still_counts_its_spread() {
    let (_dir, mut vault, card, _, _) = setup();
    let saved = vault
        .add_transaction(convert(
            (&card, rub(), "10000"),
            (&card, AssetRef::fiat("USD", "Dollar"), "98"),
            None,
            "2026-02-01T10:00:00Z",
        ))
        .unwrap();
    let loss = vault.tx_loss(&saved.meta().id).unwrap();
    assert!(loss.internal);
    assert_eq!((loss.fee, loss.slippage), (Amount::ZERO, amount("2")));
}

#[test]
fn a_rub_usdt_thb_chain_adds_up_its_links() {
    let (_dir, mut vault, card, wallet, baht) = setup();
    let first = vault
        .add_transaction(convert(
            (&card, rub(), "10000"),
            (&wallet, usdt(), "98"),
            None,
            "2026-02-01T10:00:00Z",
        ))
        .unwrap();
    let second = vault
        .add_transaction(convert(
            (&wallet, usdt(), "98"),
            (&baht, thb(), "3400"),
            None,
            "2026-02-01T12:00:00Z",
        ))
        .unwrap();
    let flow = vault
        .link_transactions(&[first.meta().id.clone(), second.meta().id.clone()], None)
        .unwrap();
    let flows = vault.list_flows(None, None, &[]);
    assert_eq!(flows.len(), 1);
    let chain = &flows[0];
    assert_eq!(chain.flow_id, flow);
    assert_eq!(chain.links.len(), 2);
    assert_eq!(chain.links[0].loss.total, amount("2"));
    assert_eq!(chain.links[1].loss.total, amount("0.86"));
    assert_eq!(chain.gave_value, amount("100"));
    assert_eq!(chain.got_value, amount("97.14"));
    assert_eq!(chain.loss, amount("2.86"));
    assert_eq!(chain.difference, amount("2.86"));

    vault.unlink_transaction(&second.meta().id).unwrap();
    assert_eq!(vault.list_flows(None, None, &[])[0].links.len(), 1);
    // Another wallet's chains stay out; one of the chain's own wallets keeps it.
    assert!(
        vault
            .list_flows(None, None, std::slice::from_ref(&baht))
            .is_empty()
    );
    assert_eq!(
        vault
            .list_flows(None, None, std::slice::from_ref(&card))
            .len(),
        1
    );
}

#[test]
fn the_rates_table_is_against_the_base() {
    let (_dir, mut vault, ..) = setup();
    vault.set_base_currency("rub");
    let table = vault.fx_table(&[]);
    assert_eq!(table.base, "rub");
    let usd = table.rows.iter().find(|row| row.currency == "usd").unwrap();
    assert_eq!(usd.in_base, amount("100"));
    let thb = table.rows.iter().find(|row| row.currency == "thb").unwrap();
    assert_eq!(thb.per_base, amount("0.35"));
    assert!(!thb.crypto && !thb.favorite);
}

#[test]
fn crypto_favorites_join_the_rates_table() {
    let (_dir, mut vault, ..) = setup();
    vault.set_base_currency("rub");
    vault.apply_prices(&[PricePoint {
        asset_key: "coin:bitcoin".into(),
        value_usd: amount("50000"),
        ts: "2026-01-02T00:00:00Z".into(),
        source: "test".into(),
        change_24h_pct: None,
    }]);
    let favorites = ["BTC".to_string(), "THB".to_string()];
    let table = vault.fx_table(&favorites);
    let btc = table.rows.iter().find(|row| row.currency == "btc").unwrap();
    assert!(btc.crypto && btc.favorite);
    assert_eq!(btc.in_base, amount("5000000"));
    assert_eq!(btc.per_base, amount("0.0000002"));
    assert!(btc.updated_at.starts_with("2026-01-02"));
    let thb = table.rows.iter().find(|row| row.currency == "thb").unwrap();
    assert!(thb.favorite && !thb.crypto);
}

#[test]
fn an_unpriced_favorite_has_no_row() {
    let (_dir, vault, ..) = setup();
    let table = vault.fx_table(&["DOGE".to_string()]);
    assert!(table.rows.iter().all(|row| row.currency != "doge"));
}

/// A book that knows no rate: every side is unpriced.
struct NoRates;

impl Valuation for NoRates {
    fn leg_value_in(&self, _leg: &Leg, _currency: &str, _secs: i64) -> Option<Amount> {
        None
    }

    fn value_at(&self, _asset: &AssetRef, _qty: Amount, _currency: &str, _secs: i64) -> Amount {
        Amount::ZERO
    }
}

fn bare_doc_with_convert() -> (VaultDoc, String) {
    let mut doc = VaultDoc::default();
    let tx = convert(
        ("a", rub(), "100"),
        ("b", usdt(), "1"),
        None,
        "2026-02-01T10:00:00Z",
    );
    let id = tx.meta().id.clone();
    doc.transactions.push(tx);
    (doc, id)
}

#[test]
fn an_unpriced_document_lists_its_losses_as_incomplete() {
    let (doc, id) = bare_doc_with_convert();
    let losses = losses_by_id(&doc, &NoRates, "usd");
    assert_eq!(losses.len(), 1);
    assert!(!losses[&id].complete);
    assert!(loss_by_id(&doc, &NoRates, "usd", "nope").is_err());
}

#[test]
fn a_chain_needs_known_transactions() {
    let (doc, id) = bare_doc_with_convert();
    assert!(matches!(
        plan_link(&doc, &[], None),
        Err(Error::InvalidInput(_))
    ));
    assert!(matches!(
        plan_link(&doc, &["nope".to_string()], None),
        Err(Error::Missing(_))
    ));
    // A given chain wins, a blank one is ignored, otherwise a new one is made.
    let ids = [id];
    assert_eq!(plan_link(&doc, &ids, Some("flow")).unwrap(), "flow");
    assert_eq!(plan_link(&doc, &ids, Some(" ")).unwrap().len(), 36);
}

#[test]
fn empty_or_unreadable_flow_bounds_stay_open() {
    let (mut doc, _) = bare_doc_with_convert();
    doc.transactions[0].meta_mut().flow_id = Some("flow".into());
    let count = |from: Option<&str>, to: Option<&str>| {
        chain_summaries(&doc, &NoRates, "usd", from, to, &[]).len()
    };
    assert_eq!(count(None, None), 1);
    for open in ["", "garbage"] {
        assert_eq!(count(Some(open), None), 1, "from {open:?}");
        assert_eq!(count(None, Some(open)), 1, "to {open:?}");
        assert_eq!(count(Some(open), Some(open)), 1, "both {open:?}");
    }
}

#[test]
fn a_chain_is_joined_when_one_link_is_in_it() {
    let (mut doc, id) = bare_doc_with_convert();
    doc.transactions[0].meta_mut().flow_id = Some("old".into());
    assert_eq!(plan_link(&doc, &[id], None).unwrap(), "old");
}

#[test]
fn the_buckets_are_ranked_by_loss_then_key() {
    let (doc, _) = bare_doc_with_convert();
    let mut buckets = LossBuckets::default();
    buckets.add(&doc.transactions[0], amount("2"));
    buckets.add(&doc.transactions[0], amount("3"));
    let [by_kind, by_store, by_contact, by_flow] = buckets.ranked();
    assert_eq!(by_kind.len(), 1);
    assert_eq!((by_kind[0].total, by_kind[0].count), (amount("5"), 2));
    // The wallet is the first side that gives; no contact and no chain here.
    assert_eq!(by_store[0].key, "a");
    assert!(by_contact.is_empty() && by_flow.is_empty());
}

/// Counts the wake-ups of the batched writer: one per committed transaction.
#[derive(Default)]
struct Wakes(std::sync::atomic::AtomicUsize);

impl SaveHook for Wakes {
    fn dirty(&self) {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

#[test]
fn link_and_unlink_are_one_write_each() {
    let (_dir, mut vault, card, wallet, _) = setup();
    let first = vault
        .add_transaction(convert(
            (&card, rub(), "10000"),
            (&wallet, usdt(), "98"),
            None,
            "2026-02-01T10:00:00Z",
        ))
        .unwrap();
    let wakes = std::sync::Arc::new(Wakes::default());
    vault.set_write_behind(Some(wakes.clone()));
    let id = first.meta().id.clone();
    let flow = vault
        .link_transactions(std::slice::from_ref(&id), None)
        .unwrap();
    assert_eq!(wakes.0.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(
        vault.doc.transactions.last().unwrap().meta().flow_id,
        Some(flow)
    );
    vault.unlink_transaction(&id).unwrap();
    assert_eq!(wakes.0.load(std::sync::atomic::Ordering::SeqCst), 2);
    // A refusal writes nothing.
    assert!(vault.unlink_transaction("nope").is_err());
    assert!(vault.link_transactions(&[], None).is_err());
    assert_eq!(wakes.0.load(std::sync::atomic::Ordering::SeqCst), 2);
}

#[test]
fn the_delegates_equal_the_scenarios() {
    let (_dir, mut vault, card, wallet, _) = setup();
    let saved = vault
        .add_transaction(convert(
            (&card, rub(), "10000"),
            (&wallet, usdt(), "95"),
            Some("105.26"),
            "2026-02-01T10:00:00Z",
        ))
        .unwrap();
    let id = saved.meta().id.clone();
    vault
        .link_transactions(std::slice::from_ref(&id), Some("chain"))
        .unwrap();
    assert_eq!(
        vault.tx_loss(&id).unwrap(),
        crate::usecases::household::losses::tx_loss(&vault, &id).unwrap()
    );
    assert_eq!(
        vault.tx_losses(),
        crate::usecases::household::losses::tx_losses(&vault)
    );
    assert_eq!(
        vault.list_flows(None, None, &[]),
        crate::usecases::household::losses::list_flows(&vault, None, None, &[])
    );
    let favorites = ["THB".to_string()];
    assert_eq!(
        vault.fx_table(&favorites),
        crate::usecases::household::losses::fx_table(&vault, &favorites)
    );
}

/// L5: a side without a price is no evidence of a spread. The missing side
/// used to count as zero, so 1000 USDT given for an unlisted token read as a
/// loss of 1000.
#[test]
fn an_unlisted_token_bought_for_usdt_has_no_slippage_but_stays_incomplete() {
    let (_dir, mut vault, _card, wallet, _) = setup();
    let token = AssetRef::crypto("ZZZ", "Unlisted", None, Chain::Tron);
    let bought = convert(
        (&wallet, usdt(), "1000"),
        (&wallet, token, "500"),
        None,
        "2026-02-01T10:00:00Z",
    );
    let id = bought.meta().id.clone();
    vault.doc.transactions.push(bought);

    let loss = vault.tx_loss(&id).unwrap();
    assert_eq!(loss.slippage, Amount::ZERO);
    assert_eq!(loss.total, Amount::ZERO);
    assert!(!loss.complete);
    assert!(vault.tx_losses().contains_key(&id));

    let filter = ReportFilter {
        from: Some("2026-01".into()),
        to: Some("2026-03".into()),
        currency: Some("usd".into()),
        ..ReportFilter::default()
    };
    let Report::Losses(report) = vault
        .report(
            ReportKind::Losses,
            &filter,
            secs("2026-03-15T00:00:00Z"),
            "UTC",
        )
        .unwrap()
    else {
        panic!("a losses report");
    };
    assert_eq!(report.losses.total.slippage, Amount::ZERO);
    assert_eq!(report.losses.incomplete, 1);
}

#[test]
fn month_index_finds_the_month_of_a_date() {
    let day = |year, month, day| chrono::NaiveDate::from_ymd_opt(year, month, day).unwrap();
    let months = [day(2026, 1, 1), day(2026, 2, 1)];
    assert_eq!(month_index(&months, day(2026, 2, 17)), Some(1));
    assert_eq!(month_index(&months, day(2026, 1, 1)), Some(0));
    assert_eq!(month_index(&months, day(2026, 3, 1)), None);
}

#[test]
fn loss_report_serializes_flat_without_a_losses_key() {
    let json = serde_json::to_value(crate::service::LossReport::default()).unwrap();
    let object = json.as_object().unwrap();
    for key in ["meta", "rows", "total", "incomplete"] {
        assert!(object.contains_key(key), "missing top-level key {key}");
    }
    assert!(!object.contains_key("losses"));
}
