//! Tests of the asset-classes area: the old inline tests (names and bodies unchanged).

use crate::model::planning::AssetClass;
use crate::model::store::StoreKind;
use crate::model::store::{AssetRef, Purpose, StoreScope};
use crate::model::vault::PricePoint;
use crate::planning::allocation::*;
use crate::service::UnlockedVault;
use crate::service::test_support::{amount, empty_vault, income_of, store};
use crate::{Amount, Error};

fn vault() -> (tempfile::TempDir, UnlockedVault) {
    empty_vault()
}

/// A store holding fiat at one USD a unit.
fn store_with(
    vault: &mut UnlockedVault,
    name: &str,
    kind: StoreKind,
    assets: &[(&str, &str)],
) -> String {
    let id = store(vault, name, kind, None);
    for (symbol, quantity) in assets {
        let asset = AssetRef::fiat(symbol, symbol);
        vault.doc.price_cache.push(PricePoint {
            asset_key: asset.key(),
            value_usd: amount("1"),
            ts: "2000-01-01T00:00:00Z".into(),
            source: "test".into(),
            change_24h_pct: None,
        });
        vault
            .add_transaction(income_of(&id, asset, quantity, "1", "seed"))
            .unwrap();
    }
    id
}

fn class(id: &str, pct: u32, cap: bool) -> AssetClass {
    AssetClass {
        id: id.into(),
        name: id.into(),
        target_pct: Amount::from(pct),
        cap,
    }
}

#[test]
fn preset_until_saved() {
    let (_dir, vault) = vault();
    let view = vault.asset_classes();
    assert!(view.preset);
    let ids: Vec<&str> = view.classes.iter().map(|class| class.id.as_str()).collect();
    assert_eq!(ids, [PRESET_LOCAL, PRESET_GLOBAL, PRESET_RISKY]);
    assert!(view.classes[2].cap);
    assert!(vault.doc.asset_classes.is_empty(), "reading saves nothing");
}

#[test]
fn shares_must_add_up_to_hundred() {
    let (_dir, mut vault) = vault();
    let err = vault
        .set_asset_classes(vec![class("one", 60, false), class("two", 30, false)])
        .unwrap_err();
    assert!(matches!(err, Error::InvalidInput(_)), "{err:?}");
    let err = vault
        .set_asset_classes(vec![class("one", 50, false), class("one", 50, false)])
        .unwrap_err();
    assert!(matches!(err, Error::AlreadyExists(_)), "{err:?}");
    let saved = vault
        .set_asset_classes(vec![class("", 70, false), class("two", 30, true)])
        .unwrap();
    assert!(!saved.preset);
    assert!(!saved.classes[0].id.is_empty(), "a new basket gets an id");
}

#[test]
fn removing_a_basket_drops_its_assignments() {
    let (_dir, mut vault) = vault();
    let store = store_with(&mut vault, "Broker", StoreKind::Exchange, &[("USD", "100")]);
    vault
        .assign_asset_class(
            AssetClassTarget::Store {
                store_id: store.clone(),
            },
            Some(PRESET_GLOBAL.into()),
        )
        .unwrap();
    assert_eq!(vault.doc.asset_classes.len(), 3, "the preset got saved");
    vault
        .assign_asset_class(
            AssetClassTarget::Asset {
                asset_key: "fiat:usd".into(),
            },
            Some(PRESET_RISKY.into()),
        )
        .unwrap();
    vault
        .set_asset_classes(vec![class(PRESET_LOCAL, 100, false)])
        .unwrap();
    assert!(vault.doc.asset_class_of.is_empty());
    assert_eq!(vault.find_store(&store).unwrap().asset_class, None);
}

#[test]
fn asset_basket_beats_store_basket() {
    let (_dir, mut vault) = vault();
    let store = store_with(
        &mut vault,
        "Broker",
        StoreKind::Exchange,
        &[("USD", "100"), ("EUR", "50")],
    );
    vault
        .assign_asset_class(
            AssetClassTarget::Store { store_id: store },
            Some(PRESET_GLOBAL.into()),
        )
        .unwrap();
    vault
        .assign_asset_class(
            AssetClassTarget::Asset {
                asset_key: "fiat:usd".into(),
            },
            Some(PRESET_LOCAL.into()),
        )
        .unwrap();
    assert_eq!(vault.doc.holdings.len(), 2);
    let resolve = vault.slot_resolver();
    for holding in &vault.doc.holdings {
        let expected = if holding.asset.key() == "fiat:usd" {
            PRESET_LOCAL
        } else {
            PRESET_GLOBAL
        };
        assert_eq!(resolve(holding), Slot::Class(Some(expected.into())));
    }
}

#[test]
fn unassigned_holdings_have_no_basket() {
    let (_dir, mut vault) = vault();
    store_with(&mut vault, "Bank", StoreKind::Bank, &[("USD", "100")]);
    let advice = vault.topup_advice(Some(amount("10")), 0, "UTC");
    assert_eq!(advice.unclassified, amount("100"));
    assert!(advice.rows.iter().all(|row| row.value.is_zero()));
}

#[test]
fn fund_accounts_stay_outside() {
    let (_dir, mut vault) = vault();
    let reserve = store_with(&mut vault, "Reserve", StoreKind::Bank, &[("USD", "300")]);
    if let Some(store) = vault
        .doc
        .stores
        .iter_mut()
        .find(|store| store.id == reserve)
    {
        store.purpose = Purpose::Savings;
    }
    vault
        .set_emergency_fund(crate::model::planning::EmergencyFund {
            scope: StoreScope {
                store_ids: vec![reserve.clone()],
                include_children: false,
            },
            ..Default::default()
        })
        .unwrap();
    let err = vault
        .assign_asset_class(
            AssetClassTarget::Store { store_id: reserve },
            Some(PRESET_LOCAL.into()),
        )
        .unwrap_err();
    assert!(matches!(err, Error::InvalidInput(_)), "{err:?}");
    let advice = vault.topup_advice(Some(amount("10")), 0, "UTC");
    assert_eq!(advice.emergency, amount("300"));
    assert!(advice.unclassified.is_zero());
}

#[test]
fn net_worth_splits_financial_by_basket() {
    let (_dir, mut vault) = vault();
    let broker = store_with(&mut vault, "Broker", StoreKind::Exchange, &[("USD", "70")]);
    store_with(&mut vault, "Bank", StoreKind::Bank, &[("USD", "30")]);
    let reserve = store_with(&mut vault, "Reserve", StoreKind::Bank, &[("USD", "50")]);
    if let Some(store) = vault
        .doc
        .stores
        .iter_mut()
        .find(|store| store.id == reserve)
    {
        store.purpose = Purpose::Savings;
    }
    vault.emergency_fund_toggle(&reserve, true).unwrap();
    vault
        .assign_asset_class(
            AssetClassTarget::Store { store_id: broker },
            Some(PRESET_GLOBAL.into()),
        )
        .unwrap();
    let worth = vault.net_worth();
    assert_eq!(worth.financial, amount("150"));
    assert_eq!(worth.financial_fund, amount("50"));
    assert_eq!(
        worth.financial_by_class,
        vec![
            crate::service::ClassShare {
                class_id: Some(PRESET_GLOBAL.into()),
                amount: amount("70"),
            },
            crate::service::ClassShare {
                class_id: None,
                amount: amount("30"),
            },
        ]
    );
}

#[test]
fn deficits_share_the_amount() {
    let classes = [
        class("local", 40, false),
        class("global", 40, false),
        class("risky", 20, true),
    ];
    // 600 / 300 / 100 of 1000; +200 → 1200: targets 480 / 480 / 240.
    let values = [amount("600"), amount("300"), amount("100")];
    let adds = split_topup(&classes, &values, amount("200"));
    // Deficits: 0, 180, 140 → 200 × 180/320 and 200 × 140/320.
    assert_eq!(adds[0], (Amount::ZERO, false));
    assert_eq!(adds[1], (amount("112.50"), false));
    assert_eq!(adds[2], (amount("87.50"), false));
}

#[test]
fn capped_basket_above_target_gets_nothing() {
    let classes = [class("safe", 70, false), class("risky", 30, true)];
    let values = [amount("500"), amount("500")];
    let adds = split_topup(&classes, &values, amount("100"));
    assert_eq!(adds[0], (amount("100"), false));
    assert_eq!(adds[1], (Amount::ZERO, true));
}

#[test]
fn empty_portfolio_follows_the_targets() {
    let classes = [class("one", 50, false), class("two", 50, false)];
    let adds = split_topup(&classes, &[Amount::ZERO, Amount::ZERO], amount("0.03"));
    let sum = adds.iter().fold(Amount::ZERO, |sum, (add, _)| sum + *add);
    assert_eq!(sum, amount("0.03"), "rounding leftovers are not lost");
    let none = split_topup(&classes, &[Amount::ZERO, Amount::ZERO], Amount::ZERO);
    assert!(none.iter().all(|(add, _)| add.is_zero()));
}

#[test]
fn advice_defaults_to_last_month_savings() {
    let (_dir, vault) = vault();
    let advice = vault.topup_advice(None, 0, "UTC");
    assert!(advice.from_savings);
    assert_eq!(advice.savings_month.as_deref(), Some("1969-12"));
    assert!(advice.amount.is_zero());
}

// -- the deciders on a bare document --

#[test]
fn shares_are_checked_without_a_vault() {
    let class = |id: &str, pct: u32| AssetClass {
        id: id.into(),
        name: format!(" {id} "),
        target_pct: Amount::from(pct),
        cap: false,
    };
    let update = plan_set_asset_classes(vec![class("a", 60), class("b", 40)]).unwrap();
    assert_eq!(update.classes[0].name, "a", "names are trimmed");
    assert_eq!(update.known.len(), 2);
    assert!(plan_set_asset_classes(vec![class("a", 60)]).is_err());
    assert!(plan_set_asset_classes(vec![class("a", 50), class("a", 50)]).is_err());
    let preset = plan_set_asset_classes(Vec::new()).unwrap();
    assert!(
        preset.known.contains(PRESET_LOCAL),
        "empty means the preset"
    );
}

#[test]
fn an_assignment_is_refused_before_anything_changes() {
    let doc = crate::model::vault::VaultDoc::default();
    let asset = AssetClassTarget::Asset {
        asset_key: " ".into(),
    };
    assert!(check_assign_asset_class(&doc, &asset, None).is_err());
    let store = AssetClassTarget::Store {
        store_id: "none".into(),
    };
    assert!(matches!(
        check_assign_asset_class(&doc, &store, Some("nope")),
        Err(Error::Missing(_))
    ));
    assert!(matches!(
        check_assign_asset_class(&doc, &store, Some(PRESET_LOCAL)),
        Err(Error::Missing(_))
    ));
}

fn basket(id: &str, pct: u32) -> AssetClass {
    class(id, pct, false)
}

#[test]
fn dropping_a_basket_leaves_a_tombstone() {
    let (_dir, mut vault) = vault();
    vault
        .set_asset_classes(vec![basket("keep", 50), basket("drop", 50)])
        .unwrap();
    super::goals_tests::flush(&mut vault);
    vault.set_asset_classes(vec![basket("keep", 100)]).unwrap();
    super::goals_tests::flush(&mut vault);
    assert!(super::goals_tests::has_key(
        vault.doc.sync.tombstones.keys(),
        "drop"
    ));
    assert!(!super::goals_tests::has_key(
        vault.doc.sync.stamps.keys(),
        "drop"
    ));
    assert_eq!(vault.doc.asset_classes.len(), 1);
}

#[test]
fn an_empty_list_brings_the_preset_back() {
    let (_dir, mut vault) = vault();
    let store = store_with(&mut vault, "Broker", StoreKind::Exchange, &[("USD", "100")]);
    vault
        .assign_asset_class(
            AssetClassTarget::Store { store_id: store },
            Some(PRESET_GLOBAL.into()),
        )
        .unwrap();
    assert_eq!(vault.doc.asset_classes.len(), 3, "the preset got saved");
    vault
        .set_asset_classes(vec![
            class(PRESET_LOCAL, 50, false),
            class(PRESET_GLOBAL, 30, false),
            class(PRESET_RISKY, 20, true),
        ])
        .unwrap();
    let view = vault.set_asset_classes(Vec::new()).unwrap();
    assert!(vault.doc.asset_classes.is_empty());
    assert!(view.preset);
}

#[test]
fn the_allocation_delegates_equal_the_scenarios() {
    use crate::usecases::planning::allocation as scenario;
    let (_a, mut by_method) = vault();
    let (_b, mut by_scenario) = vault();
    let first = by_method
        .set_asset_classes(vec![basket("b", 40), basket("a", 60)])
        .unwrap();
    let second =
        scenario::set_asset_classes(&mut by_scenario, vec![basket("b", 40), basket("a", 60)])
            .unwrap();
    assert_eq!(first, second);
    let target = || AssetClassTarget::Asset {
        asset_key: "fiat:usd".into(),
    };
    let first = by_method
        .assign_asset_class(target(), Some("a".into()))
        .unwrap();
    let second =
        scenario::assign_asset_class(&mut by_scenario, target(), Some("a".into())).unwrap();
    assert_eq!(first, second);
    assert_eq!(by_method.doc.asset_classes, by_scenario.doc.asset_classes);
    assert_eq!(by_method.doc.asset_class_of, by_scenario.doc.asset_class_of);
}
