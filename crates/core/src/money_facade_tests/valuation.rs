//! Tests of the valuation area that need the vault (`UnlockedVault`): moved from
//! `money/valuation_tests.rs` with their assertions unchanged (crate-split T13).

use crate::model::ids::StoreId;
use crate::model::store::{AssetRef, PropertyInfo};
use crate::model::store::{PropertyCategory, RevalueFloor, RevalueRule, StoreKind};
use crate::model::transaction::Transaction;
use crate::model::transaction::{AssetSellTx, KindDetails, PriceStamp, TopupTx};
use crate::model::vault::ManualPrice;
use crate::money::valuation::*;
use crate::service::StoreExtras;
use crate::service::UnlockedVault;
use crate::service::test_support::*;
use crate::time::secs_or_now;

fn thing() -> AssetRef {
    AssetRef {
        symbol: "FLAT".into(),
        name: "Flat".into(),
        coingecko_id: None,
        chain: None,
        contract: None,
        quote: "FLAT".into(),
    }
}

fn property(vault: &mut UnlockedVault, category: PropertyCategory) -> String {
    crate::usecases::money::stores::create_store_with(
        vault,
        &format!("{category:?}"),
        StoreKind::Property,
        "#555",
        "",
        None,
        StoreExtras {
            property: Some(PropertyInfo {
                category,
                ..PropertyInfo::default()
            }),
            ..StoreExtras::default()
        },
    )
    .unwrap()
    .id
}

fn buy(vault: &mut UnlockedVault, store_id: &str, price: &str, when: &str) {
    let mut tx = Transaction::Topup(TopupTx {
        meta: meta("bought", &["income_other"]),
        store_id: store_id.to_string(),
        asset: thing(),
        amount: amount("1"),
        fee: None,
        from: None,
        details: KindDetails {
            unit_price: Some(amount(price)),
            ..KindDetails::default()
        },
        price: PriceStamp::default(),
    });
    at(&mut tx, when);
    crate::usecases::money::transactions::add_transaction(vault, tx).unwrap();
}

/// An object from before this change: no rule, a hand-set price.
fn old_flat(vault: &mut UnlockedVault) -> String {
    vault.set_base_currency("usd");
    let flat = property(vault, PropertyCategory::Flat);
    buy(vault, &flat, "100000", "2020-01-10T00:00:00Z");
    let store = vault
        .doc
        .stores
        .iter_mut()
        .find(|row| row.id == flat)
        .unwrap();
    store.property.as_mut().unwrap().revalue = None;
    vault.doc.manual_prices.push(ManualPrice {
        asset_key: thing().key(),
        unit_price: amount("150000"),
        currency: "usd".into(),
        updated_at: String::new(),
    });
    flat
}

#[test]
fn new_objects_get_the_rule_of_their_type() {
    let (_dir, mut vault) = empty_vault();
    let car = property(&mut vault, PropertyCategory::Car);
    let rule = crate::money::tree::find_store(&vault.doc, &car)
        .unwrap()
        .property
        .clone()
        .unwrap()
        .revalue;
    assert_eq!(rule, RevalueRule::default_for(PropertyCategory::Car));
    assert!(rule.unwrap().floor.is_some());
    let other = property(&mut vault, PropertyCategory::Other);
    let info = crate::money::tree::find_store(&vault.doc, &other)
        .unwrap()
        .property
        .clone()
        .unwrap();
    assert_eq!(info.revalue, None);
    // Saving the form keeps the rule.
    crate::usecases::money::stores::update_store(
        &mut vault,
        &car,
        "Car",
        "#555",
        "",
        StoreExtras {
            property: Some(PropertyInfo {
                category: PropertyCategory::Car,
                ..PropertyInfo::default()
            }),
            ..StoreExtras::default()
        },
    )
    .unwrap();
    let kept = crate::money::tree::find_store(&vault.doc, &car)
        .unwrap()
        .property
        .clone()
        .unwrap();
    assert!(kept.revalue.is_some());
}

#[test]
fn old_objects_keep_their_value_and_history() {
    let (_dir, mut vault) = empty_vault();
    old_flat(&mut vault);
    let key = thing().key();
    let early = secs_or_now("2021-01-01T00:00:00Z");
    let before = crate::money::rates::rate_book(&vault.books()).history_price(&key, early);
    assert_eq!(before, Some(amount("150000")));
    assert_eq!(
        crate::usecases::money::capital::net_worth(&vault.books()).non_financial,
        amount("150000")
    );
    assert!(crate::money::valuation::valuation_curves(&vault.books()).is_empty());
}

#[test]
fn a_valuation_keeps_the_old_price_as_the_start() {
    let (_dir, mut vault) = empty_vault();
    let flat = old_flat(&mut vault);
    let key = thing().key();
    crate::usecases::money::valuation::add_valuation(
        &mut vault,
        ValuationInput {
            store_id: flat.clone(),
            asset_key: key.clone(),
            date: "2024-01-10".into(),
            unit_price: amount("200000"),
            currency: "usd".into(),
        },
    )
    .unwrap();
    let book = crate::money::rates::rate_book(&vault.books());
    let purchase = secs_or_now("2020-01-10T00:00:00Z");
    assert_eq!(book.history_price(&key, purchase), Some(amount("150000")));
    assert_eq!(
        crate::usecases::money::capital::net_worth(&vault.books()).non_financial,
        amount("200000")
    );
    let values = crate::money::valuation::property_values(
        &vault.books(),
        &StoreId::from(flat.as_str()),
        "UTC",
    )
    .unwrap();
    assert_eq!(values[0].base, Some(ValuationBaseKind::Manual));
    assert_eq!(values[0].change, Some(amount("100000")));
    assert_eq!(values[0].change_pct, Some(amount("100")));
    // The same day replaces, delete returns the point.
    let again = crate::usecases::money::valuation::add_valuation(
        &mut vault,
        ValuationInput {
            store_id: flat,
            asset_key: key,
            date: "2024-01-10".into(),
            unit_price: amount("210000"),
            currency: "usd".into(),
        },
    )
    .unwrap();
    assert_eq!(vault.doc.valuations.len(), 1);
    assert_eq!(
        crate::usecases::money::valuation::delete_valuation(&mut vault, &again.id).unwrap(),
        again
    );
    assert!(vault.doc.valuations.is_empty());
}

#[test]
fn depreciation_needs_a_floor() {
    let (_dir, mut vault) = empty_vault();
    let flat = old_flat(&mut vault);
    let rule = RevalueRule {
        pct_per_year: Some(amount("-10")),
        floor: None,
    };
    assert!(
        crate::usecases::money::valuation::set_revalue_rule(
            &mut vault,
            &StoreId::from(flat.as_str()),
            Some(rule),
            "UTC"
        )
        .is_err()
    );
    let rule = RevalueRule {
        pct_per_year: Some(amount("-10")),
        floor: Some(RevalueFloor::PctOfPurchase { pct: amount("20") }),
    };
    crate::usecases::money::valuation::set_revalue_rule(
        &mut vault,
        &StoreId::from(flat.as_str()),
        Some(rule),
        "UTC",
    )
    .unwrap();
    // Switching on starts from today's value, so capital does not jump.
    assert_eq!(vault.doc.valuations.len(), 1);
    assert_eq!(vault.doc.valuations[0].unit_price, amount("150000"));
    // Only the hours since midnight of depreciation, not years.
    let now = crate::usecases::money::capital::net_worth(&vault.books()).non_financial;
    assert!(now <= amount("150000") && now > amount("149000"), "{now}");
}

#[test]
fn loyalty_prices_stay_as_they_were() {
    let (_dir, mut vault) = empty_vault();
    let flat = old_flat(&mut vault);
    crate::usecases::money::valuation::add_valuation(
        &mut vault,
        ValuationInput {
            store_id: flat,
            asset_key: thing().key(),
            date: "2024-01-10".into(),
            unit_price: amount("200000"),
            currency: "usd".into(),
        },
    )
    .unwrap();
    vault.doc.manual_prices.push(ManualPrice {
        asset_key: "asset:POINTS".into(),
        unit_price: amount("0.5"),
        currency: "usd".into(),
        updated_at: String::new(),
    });
    assert_eq!(
        crate::usecases::money::capital::manual_price_usd(&vault.books(), "asset:POINTS"),
        Some(amount("0.5"))
    );
    assert!(crate::money::valuation::valuation_curve(&vault.books(), "asset:POINTS").is_none());
}

/// A purchase priced by another device is signed with the currency that
/// device wrote, not with the base of the reader (26-c).
#[test]
fn a_purchase_is_signed_with_the_currency_of_its_label() {
    let (_dir, mut vault) = empty_vault();
    vault.set_base_currency("usd");
    let flat = property(&mut vault, PropertyCategory::Flat);
    let mut tx = Transaction::Topup(TopupTx {
        meta: meta("bought", &["income_other"]),
        store_id: flat.clone(),
        asset: thing(),
        amount: amount("1"),
        fee: None,
        from: None,
        details: KindDetails {
            unit_price: Some(amount("9000000")),
            ..KindDetails::default()
        },
        price: PriceStamp {
            price_usd: Some(amount("100000")),
            price_base: Some(amount("9000000")),
            price_base_currency: Some("rub".into()),
            fx_override: None,
        },
    });
    at(&mut tx, "2020-01-10T00:00:00Z");
    crate::usecases::money::transactions::add_transaction(&mut vault, tx).unwrap();
    let store = crate::money::tree::find_store(&vault.doc, &flat)
        .unwrap()
        .clone();
    let purchase = purchase_of(&vault.books(), &store, &thing().key()).unwrap();
    assert_eq!(purchase.unit_price, amount("9000000"));
    assert_eq!(purchase.currency, "rub");
    assert!(purchase.from_operation);
}

/// An old operation (no label) and the form field keep the reader's base.
#[test]
fn a_purchase_without_a_label_is_signed_with_the_readers_base() {
    let (_dir, mut vault) = empty_vault();
    vault.set_base_currency("eur");
    let flat = property(&mut vault, PropertyCategory::Flat);
    buy(&mut vault, &flat, "100000", "2020-01-10T00:00:00Z");
    let store = crate::money::tree::find_store(&vault.doc, &flat)
        .unwrap()
        .clone();
    let purchase = purchase_of(&vault.books(), &store, &thing().key()).unwrap();
    assert_eq!(purchase.currency, "eur");
    // Completing the operation labels the base price with the author's base.
    let stamp = match &vault.doc.transactions[0] {
        Transaction::Topup(inner) => inner.price.clone(),
        other => panic!("unexpected {other:?}"),
    };
    assert_eq!(stamp.price_base_currency.as_deref(), Some("eur"));
}

/// L9: a flat bought at 5M, revalued to 8M and then sold keeps its curve, so
/// the months it was held are still valued.
#[test]
fn a_sold_flat_keeps_its_valuation_curve_in_past_months() {
    let (_dir, mut vault) = empty_vault();
    vault.set_base_currency("usd");
    let flat = property(&mut vault, PropertyCategory::Flat);
    buy(&mut vault, &flat, "5000000", "2020-01-10T00:00:00Z");
    crate::usecases::money::valuation::add_valuation(
        &mut vault,
        ValuationInput {
            store_id: flat.clone(),
            asset_key: thing().key(),
            date: "2024-01-10".into(),
            unit_price: amount("8000000"),
            currency: "usd".into(),
        },
    )
    .unwrap();
    let mut sale = Transaction::AssetSell(AssetSellTx {
        meta: meta("sold", &["income_other"]),
        store_id: flat,
        asset: thing(),
        unit_price: amount("8000000"),
        quantity: amount("1"),
        currency: AssetRef::fiat("USD", "USD"),
        proceeds_to: None,
        fee: None,
        price: PriceStamp::default(),
    });
    at(&mut sale, "2025-01-10T00:00:00Z");
    crate::usecases::money::transactions::add_transaction(&mut vault, sale).unwrap();
    let key = thing().key();
    assert!(
        vault.doc.holdings.iter().all(|row| row.asset.key() != key),
        "the flat is no longer held"
    );
    let book = crate::money::rates::rate_book(&vault.books());
    let bought = secs_or_now("2020-01-10T00:00:00Z");
    assert_eq!(book.history_price(&key, bought), Some(amount("5000000")));
    let between = book
        .history_price(&key, secs_or_now("2022-01-10T00:00:00Z"))
        .expect("a month of the holding is valued");
    assert!(
        between > amount("5000000") && between < amount("8000000"),
        "{between}"
    );
    let revalued = secs_or_now("2024-01-10T00:00:00Z");
    assert_eq!(book.history_price(&key, revalued), Some(amount("8000000")));
}
