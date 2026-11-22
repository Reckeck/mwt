//! Tests of the rates area that need the vault (`UnlockedVault`): moved from
//! `money/rates_tests.rs` with their assertions unchanged (crate-split T13).

use crate::Amount;
use crate::ipc_contract::{CachedPrice, PricePoint};
use crate::model::store::{AssetRef, StoreKind};
use crate::model::transaction::Leg;
use crate::model::transaction::{PriceStamp, Transaction};
use crate::model::vault::FxPoint;
use crate::money::rates::*;
use crate::service::UnlockedVault;
use crate::service::test_support::*;
use crate::time::secs_or_now;

fn fx(currency: &str, per_usd: &str, ts: &str) -> FxPoint {
    FxPoint {
        currency: currency.into(),
        per_usd: amount(per_usd),
        ts: ts.into(),
        source: "test".into(),
    }
}

fn rub() -> AssetRef {
    AssetRef::fiat("RUB", "Russian ruble")
}

/// RUB income on `ts` with no price typed.
fn rub_income(store_id: &str, sum: &str, ts: &str) -> Transaction {
    let mut tx = income_of(store_id, rub(), sum, "1", "salary");
    if let Transaction::Topup(inner) = &mut tx {
        inner.price = PriceStamp::default();
    }
    at(&mut tx, ts);
    tx
}

/// A stamp whose base price was written by a device with base `label`.
fn labelled_stamp(usd: &str, in_base: &str, label: Option<&str>) -> PriceStamp {
    PriceStamp {
        price_usd: Some(amount(usd)),
        price_base: Some(amount(in_base)),
        price_base_currency: label.map(str::to_string),
        fx_override: None,
    }
}

fn priced_payment(price: &str, currency: &str, ts: &str) -> crate::ipc_contract::TransactionInput {
    serde_json::from_value(serde_json::json!({
        "type": "payment",
        "common": {
            "timestamp": ts, "comment": "", "categories": [], "flow_id": null, "contact_id": null,
            "price_usd": null, "fx_override": null, "price": price, "price_currency": currency,
        },
        "store_id": "s", "asset": {"symbol": "BTC", "name": "", "coingecko_id": null, "chain": null, "contract": null},
        "amount": "1", "fee": null, "to": null,
        "details": {"cash_breakdown": null, "unit_price": null, "network": null, "tx_hash": null},
    }))
    .unwrap()
}

fn first_price(vault: &mut UnlockedVault) -> Option<Amount> {
    vault.doc.transactions[0].stamps_mut()[0].1.price_usd
}

/// RUB is 80 per USD in January and 100 from June. RUB is the base and
/// THB the budget currency, so both keep their history.
fn with_rub_history(vault: &mut UnlockedVault) {
    vault.set_base_currency("rub");
    vault.doc.settings.budget_currency = "thb".into();
    vault.apply_fx(&[
        fx("rub", "80", "2026-01-01T00:00:00Z"),
        fx("thb", "32", "2026-01-01T00:00:00Z"),
    ]);
    vault.apply_fx(&[
        fx("rub", "100", "2026-06-01T00:00:00Z"),
        fx("thb", "40", "2026-06-01T00:00:00Z"),
    ]);
}

#[test]
fn convert_of_a_huge_amount_is_none_not_a_panic() {
    let (_dir, mut vault) = empty_vault();
    vault.set_base_currency("usd");
    vault.apply_fx(&[fx("eur", "0.000000000001", "2026-01-01T00:00:00Z")]);
    assert_eq!(
        vault.convert_at(Amount::MAX_INPUT, "eur", "usd", "2026-01-15T00:00:00Z"),
        None
    );
}

#[test]
fn convert_crosses_through_usd_at_the_date() {
    let (_dir, mut vault) = empty_vault();
    with_rub_history(&mut vault);
    // 8000 RUB in January = 100 USD = 3200 THB; in July 8000 RUB = 80 USD.
    assert_eq!(
        vault.convert_at(amount("8000"), "rub", "thb", "2026-01-15T00:00:00Z"),
        Some(amount("3200"))
    );
    assert_eq!(
        vault.convert_at(amount("8000"), "RUB", "usd", "2026-07-01T00:00:00Z"),
        Some(amount("80"))
    );
    assert_eq!(vault.convert_at(amount("1"), "rub", "xyz", ""), None);
}

#[test]
fn saving_stamps_the_rate_of_that_day_and_keeps_a_typed_price() {
    let (_dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    with_rub_history(&mut vault);

    vault
        .add_transaction(rub_income(&card, "8000", "2026-02-01T00:00:00Z"))
        .unwrap();
    assert_eq!(first_price(&mut vault), Some(amount("0.0125")));

    let mut typed = rub_income(&card, "100", "2026-02-02T00:00:00Z");
    if let Transaction::Topup(inner) = &mut typed {
        inner.price.price_usd = Some(amount("0.02"));
    }
    let mut saved = vault.add_transaction(typed).unwrap();
    assert_eq!(saved.stamps_mut()[0].1.price_usd, Some(amount("0.02")));
}

#[test]
fn a_fiat_holding_has_value() {
    let (_dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    vault.apply_fx(&[fx("rub", "100", "2026-01-01T00:00:00Z")]);
    vault
        .add_transaction(rub_income(&card, "10000", "2026-02-01T00:00:00Z"))
        .unwrap();
    let worth = vault.net_worth();
    assert!(worth.financial.is_positive(), "{worth:?}");
}

#[test]
fn restamp_fills_only_what_is_missing() {
    let (_dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    vault
        .add_transaction(rub_income(&card, "8000", "2026-02-01T00:00:00Z"))
        .unwrap();
    assert_eq!(first_price(&mut vault), None);

    vault.apply_fx(&[fx("rub", "80", "2026-01-01T00:00:00Z")]);
    assert_eq!(vault.restamp_missing(), 1);
    assert_eq!(vault.restamp_missing(), 0);
    assert_eq!(first_price(&mut vault), Some(amount("0.0125")));
}

#[test]
fn leg_price_in_base_uses_the_rate_of_the_moment() {
    let (_dir, mut vault) = empty_vault();
    with_rub_history(&mut vault);
    let book = vault.rate_book();
    let leg = Leg::in_coming("s", &usd(), amount("1"), &usd_price("1"));
    let feb = secs_or_now("2026-02-01T00:00:00Z");
    let jul = secs_or_now("2026-07-01T00:00:00Z");
    assert_eq!(book.leg_price_in_base(&leg, "rub", feb), amount("80"));
    assert_eq!(book.leg_price_in_base(&leg, "rub", jul), amount("100"));
}

#[test]
fn a_base_price_of_another_currency_is_read_through_usd() {
    let (_dir, mut vault) = empty_vault();
    with_rub_history(&mut vault);
    vault.apply_fx(&[fx("eur", "0.5", "2026-01-01T00:00:00Z")]);
    let book = vault.rate_book();
    let feb = secs_or_now("2026-02-01T00:00:00Z");
    // Written on a device whose base is rub: 100 USD = 8000 RUB in February.
    let stamp = labelled_stamp("100", "8000", Some("rub"));
    let leg = Leg::in_coming("s", &usd(), amount("1"), &stamp);
    // The reader's base is EUR (0.5 per USD): 50, not the 8000 rubles.
    assert_eq!(book.leg_price_in_base(&leg, "eur", feb), amount("50"));
    // The label is compared without regard to case.
    let upper = labelled_stamp("100", "8000", Some("RUB"));
    let same = Leg::in_coming("s", &usd(), amount("1"), &upper);
    assert_eq!(book.leg_price_in_base(&same, "rub", feb), amount("8000"));
}

#[test]
fn a_base_price_with_the_readers_label_is_kept_as_is() {
    let (_dir, mut vault) = empty_vault();
    with_rub_history(&mut vault);
    let book = vault.rate_book();
    let feb = secs_or_now("2026-02-01T00:00:00Z");
    let stamp = labelled_stamp("100", "7777", Some("rub"));
    let leg = Leg::in_coming("s", &usd(), amount("1"), &stamp);
    assert_eq!(book.leg_price_in_base(&leg, "rub", feb), amount("7777"));
}

#[test]
fn a_base_price_without_a_label_reads_as_before() {
    let (_dir, mut vault) = empty_vault();
    with_rub_history(&mut vault);
    let book = vault.rate_book();
    let feb = secs_or_now("2026-02-01T00:00:00Z");
    let stamp = labelled_stamp("100", "7777", None);
    let leg = Leg::in_coming("s", &usd(), amount("1"), &stamp);
    assert_eq!(book.leg_price_in_base(&leg, "thb", feb), amount("7777"));
}

#[test]
fn a_labelled_base_price_with_no_usd_price_or_rate_is_kept() {
    let (_dir, mut vault) = empty_vault();
    with_rub_history(&mut vault);
    let book = vault.rate_book();
    let feb = secs_or_now("2026-02-01T00:00:00Z");
    let mut stamp = labelled_stamp("100", "8000", Some("rub"));
    stamp.price_usd = None;
    let no_usd = Leg::in_coming("s", &usd(), amount("1"), &stamp);
    assert_eq!(book.leg_price_in_base(&no_usd, "thb", feb), amount("8000"));
    // No rate for the reader's base: the number is kept, as for a USD price.
    let stamp = labelled_stamp("100", "8000", Some("rub"));
    let leg = Leg::in_coming("s", &usd(), amount("1"), &stamp);
    assert_eq!(book.leg_price_in_base(&leg, "xyz", feb), amount("8000"));
}

#[test]
fn a_price_typed_in_the_base_currency_is_labelled_with_it() {
    let (_dir, mut vault) = empty_vault();
    vault.set_base_currency("rub");
    vault.apply_fx(&[
        fx("rub", "80", "2026-01-01T00:00:00Z"),
        fx("thb", "32", "2026-01-01T00:00:00Z"),
    ]);
    let mut input = priced_payment("8000", "rub", "2026-02-01T00:00:00Z");
    vault.resolve_input_price(&mut input).unwrap();
    assert_eq!(
        input.common_mut().price_base_currency.as_deref(),
        Some("rub")
    );
    // Not the base: no base price, so no label.
    let mut other = priced_payment("3200", "thb", "2026-02-01T00:00:00Z");
    vault.resolve_input_price(&mut other).unwrap();
    assert_eq!(other.common_mut().price_base, None);
    assert_eq!(other.common_mut().price_base_currency, None);
}

#[test]
fn untracked_currencies_do_not_grow_history() {
    let (_dir, mut vault) = empty_vault();
    vault.apply_fx(&[fx("jpy", "150", "2026-01-01T00:00:00Z")]);
    vault.apply_fx(&[fx("jpy", "151", "2026-02-01T00:00:00Z")]);
    let jpy: Vec<_> = vault
        .doc
        .fx_cache
        .iter()
        .filter(|point| point.currency == "jpy")
        .collect();
    assert_eq!(jpy.len(), 1);
    assert_eq!(jpy[0].per_usd, amount("151"));
}

#[test]
fn a_hand_set_price_holds_for_the_whole_history() {
    let (_dir, mut vault) = empty_vault();
    vault
        .doc
        .manual_prices
        .push(crate::model::vault::ManualPrice {
            asset_key: "asset:FLAT".into(),
            unit_price: amount("100000"),
            currency: "usd".into(),
            updated_at: "2026-09-01T00:00:00Z".into(),
        });
    let book = vault.rate_book();
    let long_ago = secs_or_now("2020-01-01T00:00:00Z");
    assert_eq!(
        book.history_price("asset:FLAT", long_ago),
        Some(amount("100000"))
    );
}

#[test]
fn the_quote_carries_the_moment_of_its_rate() {
    let (_dir, mut vault) = empty_vault();
    with_rub_history(&mut vault);
    let quote = vault.cached_quote(&rub(), "2026-03-01T12:00:00Z");
    assert_eq!(quote.price_usd, amount("0.0125"));
    assert_eq!(quote.ts, "2026-01-01T00:00:00Z");
    assert_eq!(vault.cached_quote(&btc(), ""), CachedPrice::default());
}

#[test]
fn fx_missing_on_checks_the_calendar_day() {
    let (_dir, mut vault) = crate::service::test_support::empty_vault();
    vault.apply_fx(&[fx("rub", "90", "2026-10-09T12:00:00Z")]);
    let day = chrono::NaiveDate::from_ymd_opt(2026, 10, 9).unwrap();
    let wanted = ["rub".to_string(), "EUR".to_string(), "usd".to_string()];
    assert_eq!(vault.fx_missing_on(&wanted, day), vec!["eur".to_string()]);
    let next = day.succ_opt().unwrap();
    assert_eq!(
        vault.fx_missing_on(&wanted, next),
        vec!["eur".to_string(), "rub".to_string()]
    );
}

#[test]
fn fx_missing_on_skips_crypto_codes() {
    let (_dir, vault) = crate::service::test_support::empty_vault();
    let day = chrono::NaiveDate::from_ymd_opt(2026, 10, 9).unwrap();
    let wanted = ["btc".to_string(), "TON".to_string(), "eur".to_string()];
    assert_eq!(vault.fx_missing_on(&wanted, day), vec!["eur".to_string()]);
}

#[test]
fn convert_reaches_a_well_known_coin_not_held() {
    let (_dir, mut vault) = empty_vault();
    vault.apply_prices(&[PricePoint {
        asset_key: "coin:ethereum".into(),
        value_usd: amount("2000"),
        ts: "2026-01-01T00:00:00Z".into(),
        source: "test".into(),
        change_24h_pct: None,
    }]);
    assert_eq!(
        vault.convert_at(amount("1"), "eth", "usd", ""),
        Some(amount("2000"))
    );
}

#[test]
fn favorite_asset_prefers_the_held_identity() {
    let (_dir, mut vault) = empty_vault();
    assert_eq!(
        vault.favorite_asset("btc").map(|asset| asset.key()),
        Some("coin:bitcoin".to_string())
    );
    assert!(vault.favorite_asset("eur").is_none());
    assert!(vault.favorite_asset("usd").is_none());

    let wallet = store(&mut vault, "Cold", StoreKind::Crypto, None);
    let held = AssetRef::crypto("TON", "Toncoin", None, crate::model::chain::Chain::Ton);
    vault
        .add_transaction(income_of(&wallet, held.clone(), "5", "3", "gift"))
        .unwrap();
    assert_eq!(vault.favorite_asset("ton"), Some(held));
}

/// A price typed in rubles is stored as USD at the rate of the moment,
/// and kept as is when rubles are the base currency.
#[test]
fn a_price_in_any_currency_becomes_usd_at_the_moment_rate() {
    let (_dir, mut vault) = empty_vault();
    vault.set_base_currency("rub");
    vault.apply_fx(&[
        fx("rub", "80", "2026-01-01T00:00:00Z"),
        fx("rub", "100", "2026-06-01T00:00:00Z"),
    ]);
    let mut input = priced_payment("8000", "rub", "2026-02-01T00:00:00Z");
    vault.resolve_input_price(&mut input).unwrap();
    let common = input.common_mut();
    assert_eq!(common.price_usd, Some(amount("100")));
    assert_eq!(common.price_base, Some(amount("8000")));
    assert_eq!(common.price, None);

    let mut unknown = priced_payment("10", "thb", "2026-02-01T00:00:00Z");
    assert!(
        matches!(vault.resolve_input_price(&mut unknown), Err(crate::Error::FxMissing(code)) if code == "thb")
    );
}

/// A vault with a rate history, a quote and a hand-set price, as pinned.
fn priced_vault() -> (tempfile::TempDir, UnlockedVault) {
    let (dir, mut vault) = empty_vault();
    with_rub_history(&mut vault);
    vault.apply_fx(&[fx("eur", "0.5", "2026-01-01T00:00:00Z")]);
    vault.apply_prices(&[PricePoint {
        asset_key: "coin:bitcoin".into(),
        value_usd: amount("50000"),
        ts: "2026-01-01T00:00:00Z".into(),
        source: "test".into(),
        change_24h_pct: None,
    }]);
    vault
        .doc
        .manual_prices
        .push(crate::model::vault::ManualPrice {
            asset_key: "asset:FLAT".into(),
            unit_price: amount("100000"),
            currency: "usd".into(),
            updated_at: "2026-09-01T00:00:00Z".into(),
        });
    (dir, vault)
}

#[test]
fn lookups_of_a_fixed_vault_are_pinned() {
    let (_dir, vault) = priced_vault();
    let book = vault.rate_book();
    let feb = secs_or_now("2026-02-01T00:00:00Z");
    let jul = secs_or_now("2026-07-01T00:00:00Z");
    assert_eq!(book.fx_at("usd", feb), Some(amount("1")));
    assert_eq!(book.fx_at("rub", feb), Some(amount("80")));
    assert_eq!(book.fx_at("rub", jul), Some(amount("100")));
    assert_eq!(book.price_at("fiat:usd", feb), Some(amount("1")));
    assert_eq!(book.price_at("fiat:rub", feb), Some(amount("0.0125")));
    assert_eq!(book.price_at("fiat:eur", feb), Some(amount("2")));
    assert_eq!(book.price_at("coin:bitcoin", feb), Some(amount("50000")));
    assert_eq!(book.price_at("asset:FLAT", feb), Some(amount("100000")));
    assert_eq!(
        book.quote_at("fiat:rub", feb).map(|(price, _)| price),
        Some(amount("0.0125"))
    );
    assert_eq!(
        book.quote_at("coin:bitcoin", feb).map(|(price, _)| price),
        Some(amount("50000"))
    );
    assert_eq!(
        book.quote_at("asset:FLAT", feb),
        Some((amount("100000"), None))
    );
}

#[test]
fn build_equals_the_old_rate_book() {
    let (_dir, vault) = priced_vault();
    let curves = vault
        .valuation_curves()
        .into_iter()
        .map(|(key, curve)| {
            (
                key,
                std::sync::Arc::new(curve) as std::sync::Arc<dyn ValueCurve>,
            )
        })
        .collect();
    let built = RateBook::build(&vault.doc, curves);
    let book = vault.rate_book();
    let feb = secs_or_now("2026-02-01T00:00:00Z");
    for code in ["usd", "rub", "thb", "eur", "xyz"] {
        assert_eq!(built.fx_at(code, feb), book.fx_at(code, feb), "{code}");
    }
    for key in [
        "fiat:usd",
        "fiat:rub",
        "fiat:eur",
        "coin:bitcoin",
        "asset:FLAT",
        "coin:none",
    ] {
        assert_eq!(built.price_at(key, feb), book.price_at(key, feb), "{key}");
        assert_eq!(built.quote_at(key, feb), book.quote_at(key, feb), "{key}");
        assert_eq!(
            built.history_price(key, feb),
            book.history_price(key, feb),
            "{key}"
        );
    }
    assert_eq!(
        built.convert(amount("8000"), "rub", "eur", feb),
        book.convert(amount("8000"), "rub", "eur", feb)
    );
}

/// On the base: the stamp 0.0125 (80 per USD) times the July rate 100 gave
/// 1.25 per ruble instead of 1.
#[test]
fn a_base_currency_leg_is_priced_at_exactly_one() {
    let (_dir, mut vault) = empty_vault();
    with_rub_history(&mut vault);
    let book = vault.rate_book();
    let leg = Leg::in_coming("s", &rub(), amount("1"), &usd_price("0.0125"));
    let jul = secs_or_now("2026-07-01T00:00:00Z");
    assert_eq!(book.leg_price_in_base(&leg, "rub", jul), amount("1"));
    assert_eq!(book.price_in_base("fiat:rub", jul), Some(amount("1")));
    // Another currency keeps going through its rate.
    let other = Leg::in_coming("s", &usd(), amount("1"), &usd_price("1"));
    assert_eq!(book.leg_price_in_base(&other, "rub", jul), amount("100"));
}

/// On the base: no FX point for the base meant a USD price of 0, so the
/// cash was worth 0 instead of its quantity.
#[test]
fn base_currency_cash_is_valued_without_a_rate() {
    let (_dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    vault.set_base_currency("rub");
    vault
        .add_transaction(rub_income(&card, "10000", "2026-02-01T00:00:00Z"))
        .unwrap();
    assert_eq!(vault.net_worth().financial, amount("10000"));
}

/// The base figure is the quantity, but the USD side must not claim one
/// dollar per ruble when there is no rate to say otherwise.
#[test]
fn base_currency_cash_without_a_rate_is_not_one_usd_per_unit() {
    let (_dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    vault.set_base_currency("rub");
    vault
        .add_transaction(rub_income(&card, "10000", "2026-02-01T00:00:00Z"))
        .unwrap();
    let (_, fx) = vault.base_and_fx();
    let positions = vault.store_positions(&card, fx);
    assert_eq!(positions.len(), 1);
    assert_eq!(positions[0].value_base, amount("10000"));
    assert_eq!(positions[0].value_usd, Amount::ZERO);
    assert_eq!(vault.price_usd_for("fiat:rub"), Amount::ZERO);
}

#[test]
fn per_usd_is_the_latest_rate_and_usd_is_one() {
    let (_dir, mut vault) = empty_vault();
    vault.apply_fx(&[
        fx("rub", "90", "2026-01-01T00:00:00Z"),
        fx("rub", "100", "2026-02-01T00:00:00Z"),
    ]);
    assert_eq!(per_usd(&vault.doc, "RUB"), Some(amount("100")));
    assert_eq!(per_usd(&vault.doc, "usd"), Some(amount("1")));
    assert_eq!(per_usd(&vault.doc, "jpy"), None);
    assert_eq!(
        latest_fx_point(&vault.doc, "rub").map(|point| point.ts.as_str()),
        Some("2026-02-01T00:00:00Z")
    );
}
