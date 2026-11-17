//! Tests of the loyalty area: the old inline tests (names and bodies unchanged),
//! then the pure functions on a bare document and the delegate pins.

use super::loyalty::*;
use crate::model::ids::StoreId;
use crate::model::store::{AssetRef, LoyaltyInfo, StoreKind};
use crate::model::vault::FxPoint;
use crate::service::UnlockedVault;
use crate::service::test_support::*;

fn info(symbol: &str) -> LoyaltyInfo {
    LoyaltyInfo {
        points_symbol: symbol.into(),
        point_value: amount("1"),
        currency: "rub".into(),
        expires_at: None,
        expiring_points: None,
        login: None,
        source_store_ids: Vec::new(),
        unit: Default::default(),
    }
}

#[test]
fn points_are_valued_by_the_programme_and_reported_by_card() {
    let (_dir, mut vault) = empty_vault();
    vault.set_base_currency("rub");
    vault.apply_fx(&[FxPoint {
        currency: "rub".into(),
        per_usd: amount("100"),
        ts: "2026-01-01T00:00:00Z".into(),
        source: "test".into(),
    }]);
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    let programme = store(&mut vault, "Спасибо", StoreKind::Loyalty, None);
    vault.set_loyalty_info(&programme, info("спасибо")).unwrap();
    vault
        .adjust_points(
            &programme,
            amount("500"),
            "groceries",
            Some(card.clone()),
            Some("2026-03-01T10:00:00Z".into()),
        )
        .unwrap();
    let mut cashback = income_of(
        &card,
        AssetRef::fiat("RUB", "RUB"),
        "300",
        "0.01",
        "cashback",
    );
    cashback.meta_mut().categories = vec![CASHBACK_CATEGORY.into()];
    cashback.meta_mut().timestamp = "2026-03-02T10:00:00Z".into();
    vault.add_transaction(cashback).unwrap();

    let report = vault.cashback_report("2026-03-01T00:00:00Z", "2026-03-31T23:59:59Z");
    assert_eq!(report.cashback_base, amount("300"));
    assert_eq!(report.points_base, amount("500"));
    assert_eq!(report.by_source.len(), 1, "both earned through the card");
    assert_eq!(report.by_source[0].store_id.as_deref(), Some(card.as_str()));
    assert_eq!(report.programmes[0].points, amount("500"));
    assert_eq!(report.programmes[0].value_base, amount("500"));
}

#[test]
fn programmes_keep_their_points_apart() {
    let (_dir, mut vault) = empty_vault();
    let first = store(&mut vault, "A", StoreKind::Loyalty, None);
    let second = store(&mut vault, "B", StoreKind::Loyalty, None);
    vault.set_loyalty_info(&first, info("PTS")).unwrap();
    assert!(vault.set_loyalty_info(&second, info("pts")).is_err());
    let bank = store(&mut vault, "Bank", StoreKind::Bank, None);
    assert!(vault.set_loyalty_info(&bank, info("X")).is_err());
}

#[test]
fn a_programme_is_created_with_its_points_or_not_at_all() {
    let (_dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    let mut settings = info("миля");
    settings.login = Some("  +7 900 000-00-00 ".into());
    settings.source_store_ids = vec![card.clone(), card.clone()];
    let created = vault
        .create_loyalty_program("Аэрофлот", "#db2777", None, settings)
        .unwrap();
    assert_eq!(created.kind, StoreKind::Loyalty);
    let saved = created.loyalty.unwrap();
    assert_eq!(saved.points_symbol, "МИЛЯ");
    assert_eq!(saved.login.as_deref(), Some("+7 900 000-00-00"));
    assert_eq!(saved.source_store_ids, vec![card]);

    let before = vault.doc.stores.len();
    let taken = vault.create_loyalty_program("Другая", "#db2777", None, info("МИЛЯ"));
    assert!(taken.is_err());
    let mut bad_card = info("БАЛЛ");
    bad_card.source_store_ids = vec![created.id.clone()];
    assert!(
        vault
            .create_loyalty_program("Третья", "#db2777", None, bad_card)
            .is_err()
    );
    assert_eq!(
        vault.doc.stores.len(),
        before,
        "a refusal leaves no wallet behind"
    );
}

#[test]
fn programmes_saved_before_the_login_field_still_open() {
    let raw = r#"{"points_symbol":"СПАСИБО","point_value":"1","currency":"rub"}"#;
    let old: LoyaltyInfo = serde_json::from_str(raw).unwrap();
    assert_eq!(old.login, None);
    assert!(old.source_store_ids.is_empty());
}

/// Two devices chose one symbol before a sync: both programmes exist.
fn clashing_pair(vault: &mut UnlockedVault) -> (String, String) {
    let first = store(vault, "A", StoreKind::Loyalty, None);
    let second = store(vault, "B", StoreKind::Loyalty, None);
    vault.set_loyalty_info(&first, info("PTS")).unwrap();
    vault.set_loyalty_info(&second, info("OTHER")).unwrap();
    vault
        .doc
        .stores
        .iter_mut()
        .find(|candidate| candidate.id == second)
        .and_then(|found| found.loyalty.as_mut())
        .unwrap()
        .points_symbol = "PTS".into();
    (first, second)
}

#[test]
fn a_programme_that_already_clashes_can_still_be_saved() {
    let (_dir, mut vault) = empty_vault();
    let (_first, second) = clashing_pair(&mut vault);
    let mut edit = info("pts");
    edit.point_value = amount("2");
    let saved = vault.set_loyalty_info(&second, edit).unwrap();
    assert_eq!(saved.loyalty.unwrap().point_value, amount("2"));
}

#[test]
fn a_new_clash_is_still_refused() {
    let (_dir, mut vault) = empty_vault();
    let (first, second) = clashing_pair(&mut vault);
    let third = store(&mut vault, "C", StoreKind::Loyalty, None);
    vault.set_loyalty_info(&third, info("FREE")).unwrap();
    assert!(vault.set_loyalty_info(&third, info("PTS")).is_err());
    assert!(
        vault.set_loyalty_info(&first, info("FREE")).is_err(),
        "moving onto another programme's symbol is new"
    );
    assert!(vault.set_loyalty_info(&second, info("PTS")).is_ok());
}

#[test]
fn the_clash_is_computed_for_the_view() {
    let (_dir, mut vault) = empty_vault();
    let (first, second) = clashing_pair(&mut vault);
    assert!(vault.loyalty_symbol_clash(&first));
    assert!(vault.loyalty_symbol_clash(&second));
    assert!(vault.store_overview(&second, false).unwrap().symbol_clash);
    vault.set_loyalty_info(&second, info("MINE")).unwrap();
    assert!(!vault.loyalty_symbol_clash(&first));
    assert!(!vault.store_overview(&second, false).unwrap().symbol_clash);
}

fn doc_with_programmes(symbols: &[&str]) -> (crate::model::vault::VaultDoc, Vec<String>) {
    let mut doc = crate::model::vault::VaultDoc::default();
    let mut ids = Vec::new();
    for (index, symbol) in symbols.iter().enumerate() {
        let mut programme =
            crate::testkit::builders::store(&format!("P{index}"), StoreKind::Loyalty);
        programme.id = format!("p{index}");
        programme.loyalty = Some(info(symbol));
        ids.push(programme.id.clone());
        doc.stores.push(programme);
    }
    (doc, ids)
}

#[test]
fn the_clash_is_decided_on_a_bare_document() {
    let (doc, ids) = doc_with_programmes(&["pts", " PTS ", "FREE"]);
    assert!(loyalty_symbol_clash(&doc, &StoreId::from(&ids[0])));
    assert!(loyalty_symbol_clash(&doc, &StoreId::from(&ids[1])));
    assert!(!loyalty_symbol_clash(&doc, &StoreId::from(&ids[2])));
    assert!(!loyalty_symbol_clash(&doc, &StoreId::from("unknown")));
}

#[test]
fn the_settings_are_checked_on_a_bare_document() {
    let (doc, ids) = doc_with_programmes(&["PTS", "OTHER"]);
    assert!(checked_loyalty_info(&doc, Some(&ids[1]), info("pts")).is_err());
    assert_eq!(
        checked_loyalty_info(&doc, Some(&ids[1]), info("free"))
            .unwrap()
            .points_symbol,
        "FREE"
    );
    let mut nameless = info(" ");
    nameless.currency = "rub".into();
    assert!(checked_loyalty_info(&doc, None, nameless).is_err());
}

#[test]
fn the_points_adjustment_is_built_without_a_vault() {
    let (doc, ids) = doc_with_programmes(&["PTS"]);
    let credit =
        plan_adjust_points(&doc, &StoreId::from(&ids[0]), amount("5"), "c", None, None).unwrap();
    assert!(matches!(
        credit,
        crate::model::transaction::Transaction::Topup(_)
    ));
    let spend =
        plan_adjust_points(&doc, &StoreId::from(&ids[0]), amount("-5"), "c", None, None).unwrap();
    assert!(matches!(
        spend,
        crate::model::transaction::Transaction::Payment(_)
    ));
    assert!(
        plan_adjust_points(&doc, &StoreId::from(&ids[0]), amount("0"), "c", None, None).is_err()
    );
}

#[test]
fn the_cashback_is_reported_on_a_bare_document() {
    let (mut doc, ids) = doc_with_programmes(&["PTS"]);
    let card = crate::testkit::builders::store("Card", StoreKind::Bank);
    let mut topup = income_of(
        &card.id,
        AssetRef::fiat("RUB", "RUB"),
        "300",
        "0.01",
        "cashback",
    );
    topup.meta_mut().categories = vec![CASHBACK_CATEGORY.into()];
    topup.meta_mut().timestamp = "2026-03-02T10:00:00Z".into();
    doc.transactions.push(topup);
    let book = crate::service::RateBook::build(&doc, Default::default());
    let rates = CashbackRates {
        base: "rub",
        fx: amount("1"),
        book: &book,
        price_usd: &|_| amount("1"),
    };
    let report = cashback_over(&doc, &rates, "2026-03-01T00:00:00Z", "2026-03-31T23:59:59Z");
    assert_eq!(report.base_currency, "rub");
    assert_eq!(report.by_source.len(), 1);
    assert_eq!(
        report.by_source[0].store_id.as_deref(),
        Some(card.id.as_str())
    );
    assert_eq!(report.programmes.len(), 1);
    assert_eq!(report.programmes[0].store_id, ids[0]);
    let empty = cashback_over(&doc, &rates, "2027-01-01T00:00:00Z", "2027-12-31T00:00:00Z");
    assert!(empty.by_source.is_empty());
}

#[test]
fn the_delegates_equal_the_scenarios() {
    let (_dir, mut vault) = empty_vault();
    let programme = store(&mut vault, "A", StoreKind::Loyalty, None);
    let saved = vault.set_loyalty_info(&programme, info("PTS")).unwrap();
    assert_eq!(
        saved,
        crate::usecases::household::loyalty::set_loyalty_info(
            &mut vault,
            &StoreId::from(&programme),
            info("PTS")
        )
        .unwrap()
    );
    assert_eq!(
        vault.loyalty_symbol_clash(&programme),
        loyalty_symbol_clash(&vault.doc, &StoreId::from(&programme))
    );
    let (from, to) = ("2026-01-01T00:00:00Z", "2026-12-31T00:00:00Z");
    assert_eq!(
        vault.cashback_report(from, to),
        cashback_report(&vault.books(), from, to)
    );
}
