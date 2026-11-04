//! The threshold a price alert fired on (`triggered_for`) and the list answer.

use super::*;
use crate::model::vault::PricePoint;
use crate::service::test_support::*;

fn alert(price: &str) -> PriceAlert {
    PriceAlert {
        id: "alert".into(),
        asset: btc(),
        condition: AlertCondition::Above,
        price: amount(price),
        currency: "usd".into(),
        active: true,
        triggered_at: None,
        triggered_for: None,
        store_id: None,
        repeats: false,
    }
}

#[test]
fn the_fingerprint_follows_asset_condition_price_and_currency() {
    let base = alert("95000");
    let mut other_asset = alert("95000");
    other_asset.asset = usd();
    let mut other_condition = alert("95000");
    other_condition.condition = AlertCondition::Below;
    let mut other_currency = alert("95000");
    other_currency.currency = "eur".into();
    for other in [other_asset, other_condition, other_currency, alert("96000")] {
        assert_ne!(base.fingerprint(), other.fingerprint());
    }
}

#[test]
fn the_same_price_written_with_another_scale_has_one_fingerprint() {
    assert_eq!(alert("100").fingerprint(), alert("100.0").fingerprint());
    assert_eq!(alert("100").fingerprint(), alert("100.00").fingerprint());
}

#[test]
fn a_firing_without_a_threshold_counts_as_fired() {
    let mut old = alert("95000");
    old.triggered_at = Some("2026-03-01T00:00:00Z".into());
    assert!(old.is_triggered());
    old.triggered_for = Some(old.fingerprint());
    assert!(old.is_triggered());
    old.triggered_for = Some("another threshold".into());
    assert!(!old.is_triggered());
    old.triggered_at = None;
    old.triggered_for = None;
    assert!(!old.is_triggered());
}

#[test]
fn a_fired_alert_remembers_its_threshold_and_an_edit_clears_both() {
    let (_dir, mut vault) = empty_vault();
    vault.doc.price_cache.push(PricePoint {
        asset_key: btc().key(),
        value_usd: amount("100000"),
        ts: "2026-03-01T00:00:00Z".into(),
        source: "test".into(),
        change_24h_pct: None,
    });
    let draft = |price: &str| PriceAlertDraft {
        asset: btc(),
        condition: AlertCondition::Above,
        price: amount(price),
        currency: "usd".into(),
        active: true,
        store_id: None,
        repeats: false,
    };
    let created = vault.create_price_alert(draft("95000")).unwrap();
    let now = unix_secs("2026-03-01T01:00:00Z").unwrap();
    assert_eq!(vault.check_alerts(now).unwrap().len(), 1);
    let stored = &vault.doc.price_alerts[0];
    assert_eq!(
        stored.triggered_for.as_deref(),
        Some(stored.fingerprint().as_str())
    );
    assert!(stored.is_triggered());
    let edited = vault
        .update_price_alert(&created.id, draft("99000"))
        .unwrap();
    assert!(edited.triggered_at.is_none() && edited.triggered_for.is_none());
}

#[test]
fn the_list_hides_a_stale_firing_but_the_record_keeps_it() {
    let (_dir, mut vault) = empty_vault();
    let mut stale = alert("120000");
    stale.triggered_at = Some("2026-03-01T00:00:00Z".into());
    stale.triggered_for = Some(alert("95000").fingerprint());
    let mut fired = alert("95000");
    fired.id = "fired".into();
    fired.triggered_at = Some("2026-03-01T00:00:00Z".into());
    fired.triggered_for = Some(fired.fingerprint());
    vault.doc.price_alerts = vec![stale, fired];
    let listed = vault.price_alerts();
    assert!(
        listed[0].triggered_at.is_none(),
        "a stale firing shows as armed"
    );
    assert!(
        listed[1].triggered_at.is_some(),
        "a real firing still shows"
    );
    assert!(vault.doc.price_alerts[0].triggered_at.is_some());
    assert!(vault.doc.price_alerts[0].triggered_for.is_some());
}
