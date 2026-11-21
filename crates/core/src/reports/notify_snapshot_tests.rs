//! Tests of the notify_snapshot area: the old inline tests (names and bodies unchanged).

use std::collections::{HashMap, HashSet};

use super::notify_snapshot::*;
use crate::model::planning::AlertCondition;
use crate::model::planning::{ReminderRepeat, RepeatUnit};
use crate::model::vault::now_iso;
use crate::service::test_support::*;
use crate::time::secs_or_now;

#[test]
fn a_snapshot_holds_every_repeat_and_reminds_once_without_the_vault() {
    let (_dir, mut vault) = empty_vault();
    vault
        .create_reminder(
            "Аренда",
            "2026-03-05T09:00:00Z",
            Some(ReminderRepeat {
                every: 1,
                unit: RepeatUnit::Month,
            }),
        )
        .unwrap();
    let now = secs_or_now("2026-03-01T00:00:00Z");
    let snapshot = vault.notification_snapshot(now, "UTC", true);
    // March 5, April 5 within 60 days.
    assert_eq!(snapshot.notices.len(), 2);

    let mut shown = HashSet::new();
    let april = secs_or_now("2026-04-05T09:01:00Z");
    let due = snapshot.due(april, &shown);
    assert_eq!(due.len(), 2);
    shown.extend(due.iter().map(|record| record.key.clone()));
    assert!(snapshot.due(april + 60, &shown).is_empty());

    // At unlock the vault takes them in: the same notices are not told again.
    vault.apply_locked_deliveries(due, &[]).unwrap();
    vault.doc.reminders[0].due_at = "2026-04-05T09:00:00Z".into();
    assert!(
        vault
            .due_notices(april + 120, "UTC", true)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn a_locked_client_snapshot_fires_nothing() {
    let (_dir, mut vault) = empty_vault();
    vault
        .create_reminder("Аренда", "2026-03-05T09:00:00Z", None)
        .unwrap();
    vault
        .doc
        .price_alerts
        .push(crate::model::planning::PriceAlert {
            id: "alert".into(),
            asset: btc(),
            condition: AlertCondition::Above,
            price: amount("60000"),
            currency: "usd".into(),
            active: true,
            triggered_at: None,
            triggered_for: None,
            store_id: None,
            repeats: false,
        });
    let mut snapshot =
        vault.notification_snapshot(secs_or_now("2026-03-01T00:00:00Z"), "UTC", true);
    let later = secs_or_now("2026-03-05T10:00:00Z");
    let prices = HashMap::from([(btc().key(), amount("61000"))]);
    assert_eq!(snapshot.due(later, &HashSet::new()).len(), 1);
    assert_eq!(snapshot.alert_hits(&prices, &HashSet::new(), true).len(), 1);
    snapshot.role_client = true;
    assert!(snapshot.due(later, &HashSet::new()).is_empty());
    assert!(
        snapshot
            .alert_hits(&prices, &HashSet::new(), true)
            .is_empty()
    );
}

#[test]
fn an_old_snapshot_without_role_still_fires() {
    let (_dir, mut vault) = empty_vault();
    vault
        .create_reminder("Аренда", "2026-03-05T09:00:00Z", None)
        .unwrap();
    let snapshot = vault.notification_snapshot(secs_or_now("2026-03-01T00:00:00Z"), "UTC", true);
    let mut value = serde_json::to_value(&snapshot).unwrap();
    value.as_object_mut().unwrap().remove("role_client");
    let old: NotificationSnapshot = serde_json::from_value(value).unwrap();
    assert!(!old.role_client);
    assert_eq!(
        old.due(secs_or_now("2026-03-05T10:00:00Z"), &HashSet::new())
            .len(),
        1
    );
}

#[test]
fn hidden_details_and_an_alert_checked_in_another_currency() {
    let (_dir, mut vault) = empty_vault();
    vault.doc.settings.notifications.hide_details_when_locked = true;
    vault
        .create_reminder("Секретный платёж", "2026-03-05T09:00:00Z", None)
        .unwrap();
    vault.doc.fx_cache.push(crate::model::vault::FxPoint {
        currency: "rub".into(),
        per_usd: amount("100"),
        ts: "2026-03-01T00:00:00Z".into(),
        source: "test".into(),
    });
    vault
        .doc
        .price_alerts
        .push(crate::model::planning::PriceAlert {
            id: "alert".into(),
            asset: btc(),
            condition: AlertCondition::Above,
            price: amount("6000000"),
            currency: "rub".into(),
            active: true,
            triggered_at: None,
            triggered_for: None,
            store_id: None,
            repeats: false,
        });
    let snapshot = vault.notification_snapshot(secs_or_now("2026-03-01T00:00:00Z"), "UTC", true);
    let due = snapshot.due(secs_or_now("2026-03-05T10:00:00Z"), &HashSet::new());
    assert!(!due[0].body.contains("Секретный"));

    let prices = HashMap::from([(btc().key(), amount("59000"))]);
    assert!(
        snapshot
            .alert_hits(&prices, &HashSet::new(), true)
            .is_empty()
    );
    let prices = HashMap::from([(btc().key(), amount("61000"))]);
    let hits = snapshot.alert_hits(&prices, &HashSet::new(), true);
    assert_eq!(hits.len(), 1);

    vault
        .apply_locked_deliveries(vec![hits[0].1.clone()], &[(hits[0].0.clone(), now_iso())])
        .unwrap();
    assert!(vault.doc.price_alerts[0].triggered_at.is_some());
    assert_eq!(vault.doc.notification_log.len(), 1);
}

#[test]
fn a_locked_firing_does_not_override_a_newer_threshold() {
    let (_dir, mut vault) = empty_vault();
    vault
        .doc
        .price_alerts
        .push(crate::model::planning::PriceAlert {
            id: "alert".into(),
            asset: btc(),
            condition: AlertCondition::Above,
            price: amount("60000"),
            currency: "usd".into(),
            active: true,
            triggered_at: None,
            triggered_for: None,
            store_id: None,
            repeats: false,
        });
    let snapshot = vault.notification_snapshot(secs_or_now("2026-03-01T00:00:00Z"), "UTC", true);
    let prices = HashMap::from([(btc().key(), amount("61000"))]);
    let hits = snapshot.alert_hits(&prices, &HashSet::new(), true);
    let fired = [(hits[0].0.clone(), now_iso())];
    let seen = HashMap::from([(
        snapshot.alerts[0].alert_id.clone(),
        snapshot.alerts[0].base(),
    )]);

    // A newer threshold arrived by sync before the unlock merged the firing.
    vault.doc.price_alerts[0].price = amount("70000");
    vault
        .apply_locked_deliveries_seen(vec![hits[0].1.clone()], &fired, &seen)
        .unwrap();
    assert!(
        vault.doc.price_alerts[0].triggered_at.is_none(),
        "the old firing switched the new threshold off"
    );
    // It is still told: the notice was shown.
    assert_eq!(vault.doc.notification_log.len(), 1);

    // Unchanged since it was seen: marked as before.
    vault.doc.price_alerts[0].price = amount("60000");
    vault
        .apply_locked_deliveries_seen(vec![], &fired, &seen)
        .unwrap();
    assert!(vault.doc.price_alerts[0].triggered_at.is_some());
}
