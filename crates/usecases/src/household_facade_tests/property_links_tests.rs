//! Tests of the property-links area: the old inline tests (names and bodies
//! unchanged), then the pure functions on a bare document.

use crate::model::planning::{Schedule, TransactionDraft};
use crate::model::store::StoreKind;
use crate::model::store::{AssetRef, PropertyCategory, PropertyInfo};
use crate::model::transaction::Transaction;
use crate::model::transaction::{KindDetails, PriceStamp, TopupTx};
use crate::model::vault::now_iso;
use crate::planning::automations::AutomationDraft;
use crate::service::Session;
use crate::service::StoreExtras;
use crate::service::test_support::*;
use crate::time::unix_secs;

fn property(vault: &mut Session, category: PropertyCategory) -> String {
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

fn thing(symbol: &str) -> AssetRef {
    AssetRef {
        symbol: symbol.into(),
        name: symbol.into(),
        coingecko_id: None,
        chain: None,
        contract: None,
        quote: symbol.into(),
    }
}

fn buy(vault: &mut Session, store_id: &str, symbol: &str, price: &str, when: &str) {
    let mut tx = Transaction::Topup(TopupTx {
        meta: meta("bought", &["income_other"]),
        store_id: store_id.to_string(),
        asset: thing(symbol),
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

fn days_ago(days: i64) -> String {
    crate::calendar::iso(unix_secs(&now_iso()).unwrap() - days * 86_400)
}

#[test]
fn a_link_follows_the_operation() {
    let (_dir, mut vault) = empty_vault();
    vault.set_base_currency("usd");
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    let flat = property(&mut vault, PropertyCategory::Flat);
    let mut rent = income(&card, "500", "rent");
    rent.meta_mut().categories = vec!["rent_income".into()];
    let saved =
        crate::usecases::money::transactions::add_transaction_for(&mut vault, rent, Some(&flat))
            .unwrap();
    let id = saved.meta().id.clone();
    assert_eq!(
        crate::household::property_links::property_of_tx(&vault.doc, &id),
        Some(flat.as_str())
    );
    // A plain edit keeps the link; an edit with `None` drops it.
    crate::usecases::money::transactions::update_transaction(&mut vault, &id, saved.clone())
        .unwrap();
    assert_eq!(
        crate::household::property_links::property_of_tx(&vault.doc, &id),
        Some(flat.as_str())
    );
    crate::usecases::money::transactions::update_transaction_for(
        &mut vault,
        &id,
        saved.clone(),
        None,
    )
    .unwrap();
    assert_eq!(
        crate::household::property_links::property_of_tx(&vault.doc, &id),
        None
    );
    crate::usecases::money::transactions::update_transaction_for(
        &mut vault,
        &id,
        saved,
        Some(&flat),
    )
    .unwrap();
    crate::usecases::money::transactions::delete_transaction(&mut vault, &id).unwrap();
    assert!(crate::household::property_links::tx_property_links(&vault.doc).is_empty());
}

#[test]
fn only_a_live_property_takes_a_link() {
    let (_dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    let before = vault.doc.transactions.len();
    assert!(
        crate::usecases::money::transactions::add_transaction_for(
            &mut vault,
            income(&card, "1", "x"),
            Some(&card)
        )
        .is_err()
    );
    assert!(
        crate::usecases::money::transactions::add_transaction_for(
            &mut vault,
            income(&card, "1", "x"),
            Some("nope")
        )
        .is_err()
    );
    assert_eq!(vault.doc.transactions.len(), before);
}

/// The old rent preset credited money to the property itself; the
/// existing rule already refuses that.
#[test]
fn rent_into_the_property_itself_is_refused() {
    let (_dir, mut vault) = empty_vault();
    let flat = property(&mut vault, PropertyCategory::Flat);
    let mut rent = income(&flat, "500", "rent");
    rent.meta_mut().categories = vec!["rent_income".into()];
    assert!(crate::usecases::money::transactions::add_transaction(&mut vault, rent).is_err());
}

#[test]
fn an_automation_links_what_it_books() {
    let (_dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    let flat = property(&mut vault, PropertyCategory::Flat);
    let Transaction::Topup(mut inner) = income(&card, "500", "rent") else {
        unreachable!()
    };
    inner.meta.categories = vec!["rent_income".into()];
    let draft = |property_id: Option<String>| AutomationDraft {
        time: None,
        asks: false,
        for_month_offset: None,
        name: "Rent".into(),
        active: true,
        preset: None,
        schedule: Schedule::Monthly { day: 5 },
        starts_at: "2026-01-01T00:00:00Z".into(),
        template: TransactionDraft::Topup(inner.clone()),
        amount_mode: Default::default(),
        workday_shift: Default::default(),
        property_id,
    };
    assert!(
        crate::usecases::planning::automations::create_automation(
            &mut vault,
            draft(Some(card.clone()))
        )
        .is_err()
    );
    let rent = crate::usecases::planning::automations::create_automation(
        &mut vault,
        draft(Some(flat.clone())),
    )
    .unwrap();
    assert_eq!(rent.property_id.as_deref(), Some(flat.as_str()));
    let pass = crate::usecases::workflow::runs::run_due_workflows(
        &mut vault,
        unix_secs("2026-02-20T12:00:00Z").unwrap(),
        "UTC",
    )
    .unwrap();
    assert_eq!(pass.created.len(), 2);
    let links = crate::household::property_links::tx_property_links(&vault.doc);
    assert_eq!(links.len(), 2);
    assert!(links.iter().all(|link| link.property_id == flat));
}

#[test]
fn the_summary_counts_rent_and_its_yield() {
    let (_dir, mut vault) = empty_vault();
    vault.set_base_currency("usd");
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    let flat = property(&mut vault, PropertyCategory::Flat);
    buy(&mut vault, &flat, "FLAT", "100000", &days_ago(400));
    for days in [20, 50, 400] {
        let mut rent = income(&card, "500", "rent");
        rent.meta_mut().categories = vec!["rent_income".into()];
        at(&mut rent, &days_ago(days));
        crate::usecases::money::transactions::add_transaction_for(&mut vault, rent, Some(&flat))
            .unwrap();
    }
    let mut repair = payment(&card, "300", "paint");
    repair.meta_mut().categories = vec!["repair".into()];
    at(&mut repair, &days_ago(10));
    crate::usecases::money::transactions::add_transaction_for(&mut vault, repair, Some(&flat))
        .unwrap();

    let summary =
        crate::usecases::household::utilities::property_summary(&vault, &flat, "UTC").unwrap();
    assert_eq!(summary.rent_12m_base, amount("1000"), "a year back only");
    assert_eq!(summary.expenses_12m_base, amount("300"));
    let value = summary.value_base;
    assert!(value.is_positive());
    assert_eq!(
        summary.rent_yield_pct,
        Some((amount("1000") / value * amount("100")).round(2))
    );
    assert_eq!(summary.ownership_month_base, None, "a flat is no car");
}

#[test]
fn a_car_costs_its_expenses_and_its_wear() {
    let (_dir, mut vault) = empty_vault();
    vault.set_base_currency("usd");
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    let car = property(&mut vault, PropertyCategory::Car);
    buy(&mut vault, &car, "CAR", "10000", &days_ago(800));
    let mut fuel = payment(&card, "1200", "fuel");
    fuel.meta_mut().categories = vec!["fuel".into()];
    at(&mut fuel, &days_ago(30));
    crate::usecases::money::transactions::add_transaction_for(&mut vault, fuel, Some(&car))
        .unwrap();

    let summary =
        crate::usecases::household::utilities::property_summary(&vault, &car, "UTC").unwrap();
    assert_eq!(summary.expenses_12m_base, amount("1200"));
    assert!(summary.wear_12m_base.is_positive(), "a car loses value");
    // wear_12m_base is rounded, the month cost divides the exact wear: a cent apart at most.
    let month = summary
        .ownership_month_base
        .expect("a car has a month cost");
    let expected = (amount("1200") + summary.wear_12m_base) / amount("12");
    assert!(
        (month - expected).abs() <= amount("0.01"),
        "{month:?} vs {expected:?}"
    );
    assert_eq!(summary.rent_yield_pct, None);
}

#[test]
fn links_are_set_read_and_dropped_on_a_bare_document() {
    use crate::household::property_links::*;
    let mut doc = crate::model::vault::VaultDoc::default();
    replace_link(&mut doc, "tx-1", Some("flat"));
    replace_link(&mut doc, "tx-2", Some("car"));
    assert_eq!(property_of_tx(&doc, "tx-1"), Some("flat"));
    assert_eq!(tx_property_links(&doc).len(), 2);
    replace_link(&mut doc, "tx-1", Some("car"));
    assert_eq!(property_of_tx(&doc, "tx-1"), Some("car"));
    replace_link(&mut doc, "tx-1", None);
    assert_eq!(property_of_tx(&doc, "tx-1"), None);
    assert_eq!(tx_property_links(&doc).len(), 1);
}

#[test]
fn the_delegates_equal_the_scenarios() {
    use crate::usecases::household::property_links as scenario;
    let (_dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    let flat = property(&mut vault, PropertyCategory::Flat);
    assert!(crate::usecases::household::property_links::check_property_link(&vault, None).is_ok());
    assert!(
        crate::usecases::household::property_links::check_property_link(&vault, Some(&flat))
            .is_ok()
    );
    assert!(
        crate::usecases::household::property_links::check_property_link(&vault, Some(&card))
            .is_err()
    );
    assert!(scenario::check_property_link(&vault, Some(&card)).is_err());
    crate::usecases::household::property_links::set_property_link(&mut vault, "tx-1", Some(&flat));
    scenario::set_property_link(&mut vault, "tx-2", Some(&flat));
    assert_eq!(
        crate::household::property_links::property_of_tx(&vault.doc, "tx-2"),
        Some(flat.as_str())
    );
    assert_eq!(
        crate::household::property_links::tx_property_links(&vault.doc).len(),
        2
    );
}
