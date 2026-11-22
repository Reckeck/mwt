//! Tests of the worth area: the old inline tests (names kept), which drive
//! the same rules through the `UnlockedVault` delegates.

use crate::model::store::AssetRef;
use crate::model::store::{Holding, Store, StoreKind};
use crate::model::vault::VaultDoc;
use crate::money::worth::*;
use crate::service::test_support::*;

/// A bare document: a bank (in the budget), a property wallet holding `FLAT`.
fn bare_doc() -> (VaultDoc, AssetRef) {
    let thing = AssetRef {
        symbol: "FLAT".into(),
        name: "Flat".into(),
        coingecko_id: None,
        chain: None,
        contract: None,
        quote: "FLAT".into(),
    };
    let mut doc = VaultDoc::default();
    let mut bank = Store::new("Bank", StoreKind::Bank, "");
    bank.id = "bank".into();
    let mut flat = Store::new("Flat", StoreKind::Property, "");
    flat.id = "flat".into();
    doc.stores.extend([bank, flat]);
    doc.holdings.push(Holding {
        id: "h1".into(),
        store_id: "flat".into(),
        asset: thing.clone(),
        quantity: amount("1"),
        avg_cost: None,
    });
    (doc, thing)
}

#[test]
fn plan_manual_price_refuses_bad_input_and_accepts_held_property() {
    let (doc, thing) = bare_doc();
    let key = thing.key();
    let plan = |price: &str, currency: &str, key: &str| {
        plan_manual_price(&doc, key, amount(price), currency, "now".into())
    };
    assert!(plan("0", "usd", &key).is_err());
    assert!(plan("-5", "usd", &key).is_err());
    assert!(plan("10", "  ", &key).is_err());
    assert!(plan("10", "usd", "asset:OTHER").is_err());
    let price = plan("120000", " USD ", &key).unwrap();
    assert_eq!(price.asset_key, key);
    assert_eq!(price.unit_price, amount("120000"));
    assert_eq!(price.currency, "usd");
    assert_eq!(price.updated_at, "now");
}

#[test]
fn classify_flow_reads_a_bare_document() {
    let (doc, _) = bare_doc();
    assert_eq!(
        classify_flow(&doc, &payment("bank", "5", "lunch")),
        FlowClass::Daily
    );
    assert_eq!(
        classify_flow(&doc, &payment("flat", "5", "x")),
        FlowClass::Ignored
    );
    assert!(goal_store_ids(&doc).is_empty());
}
