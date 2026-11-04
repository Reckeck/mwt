//! Notification settings merge per category, `default_time` on its own; the
//! two switches that describe the device (`system`, `hide_details_when_locked`)
//! never travel. An older build sends the whole object under one stamp.

use serde_json::json;

use super::tests::{settle, two_devices};
use super::*;
use crate::model::notify::{NoticeCategory, NoticeCategoryCfg};
use crate::sync::stamp::now_ms;

fn peer(name: &str) -> PeerRef {
    PeerRef {
        device_id: format!("{name}-id"),
        name: name.into(),
    }
}

fn set_lead(vault: &mut UnlockedVault, category: NoticeCategory, lead_days: u16) {
    let mut config = vault.doc.settings.notifications.clone();
    let mut entry = config.category(category);
    entry.lead_days = lead_days;
    config.categories.retain(|cfg| cfg.category != category);
    config.categories.push(entry);
    vault.doc.settings.notifications = config;
    vault.persist().unwrap();
}

fn lead(vault: &UnlockedVault, category: NoticeCategory) -> u16 {
    vault
        .doc
        .settings
        .notifications
        .category(category)
        .lead_days
}

#[test]
fn switches_of_two_categories_saved_on_two_devices_both_survive() {
    let (_dir, mut left, mut right) = two_devices();
    // The form sends the whole object; the other category rides along.
    set_lead(&mut left, NoticeCategory::Loan, 5);
    settle(&mut left, &mut right);
    set_lead(&mut left, NoticeCategory::Tax, 21);
    set_lead(&mut right, NoticeCategory::Loan, 9);
    settle(&mut left, &mut right);
    for vault in [&left, &right] {
        assert_eq!(lead(vault, NoticeCategory::Tax), 21);
        assert_eq!(lead(vault, NoticeCategory::Loan), 9);
    }
}

#[test]
fn a_saved_object_does_not_overwrite_a_category_it_did_not_change() {
    let (_dir, mut left, mut right) = two_devices();
    set_lead(&mut left, NoticeCategory::Loan, 5);
    settle(&mut left, &mut right);
    // Right saves the whole object, with a stale Loan entry, after only Tax changed.
    set_lead(&mut left, NoticeCategory::Loan, 6);
    set_lead(&mut right, NoticeCategory::Tax, 30);
    settle(&mut left, &mut right);
    for vault in [&left, &right] {
        assert_eq!(lead(vault, NoticeCategory::Loan), 6);
        assert_eq!(lead(vault, NoticeCategory::Tax), 30);
    }
}

#[test]
fn the_two_device_switches_never_travel() {
    let (_dir, mut left, mut right) = two_devices();
    left.doc.settings.notifications.system = false;
    left.doc.settings.notifications.hide_details_when_locked = true;
    left.doc.settings.notifications.default_time = "08:30".into();
    left.persist().unwrap();
    right.doc.settings.notifications.hide_details_when_locked = false;
    right.persist().unwrap();
    set_lead(&mut right, NoticeCategory::Rent, 4);
    settle(&mut left, &mut right);
    assert!(!left.doc.settings.notifications.system);
    assert!(left.doc.settings.notifications.hide_details_when_locked);
    assert!(right.doc.settings.notifications.system);
    assert!(!right.doc.settings.notifications.hide_details_when_locked);
    // The rest of the object is the group's.
    assert_eq!(right.doc.settings.notifications.default_time, "08:30");
    assert_eq!(lead(&left, NoticeCategory::Rent), 4);
}

#[test]
fn a_device_switch_change_stamps_nothing() {
    use crate::sync::stamp::Clock;
    use crate::sync::state::SyncState;
    use crate::sync::track::Tracker;
    let (_dir, mut vault, _right) = two_devices();
    let mut tracker = Tracker::capture(&vault.doc_value().unwrap());
    let mut state = SyncState::default();
    let mut clock = Clock::new("dev");
    vault.doc.settings.notifications.system = false;
    vault.doc.settings.notifications.hide_details_when_locked = true;
    let changed = tracker.stamp(&vault.doc_value().unwrap(), &mut state, &mut clock);
    assert!(changed.is_empty(), "{changed:?}");
    set_lead(&mut vault, NoticeCategory::Loan, 3);
    let changed = tracker.stamp(&vault.doc_value().unwrap(), &mut state, &mut clock);
    assert_eq!(changed, vec!["settings:".to_string()]);
    let stamps = &state.stamps["settings:"];
    assert!(
        stamps
            .fields
            .keys()
            .any(|name| name.starts_with("notifications.categories"))
    );
    assert!(!stamps.fields.keys().any(|name| name.ends_with(".system")));
}

/// An older build sends the whole object under one stamp, with its own
/// device switches: the categories are taken, the switches are not.
#[test]
fn a_whole_notification_object_from_an_older_build_merges() {
    let (_dir, mut left, mut right) = two_devices();
    set_lead(&mut right, NoticeCategory::Tax, 12);
    right.doc.settings.notifications.system = true;
    right.persist().unwrap();
    let mut records = left.sync_records(&["settings:".to_string()]).unwrap();
    let state = records.get_mut("settings:").unwrap();
    let value = state.value.as_mut().unwrap().as_object_mut().unwrap();
    value.insert(
        "notifications".into(),
        json!({
            "system": false,
            "hide_details_when_locked": true,
            "default_time": "07:15",
            "categories": [{
                "category": "loan", "enabled": false, "lead_days": 2, "auto_reminders": true
            }],
        }),
    );
    let stamps = state.stamps.as_mut().unwrap();
    stamps
        .fields
        .retain(|name, _| !name.starts_with("notifications"));
    stamps.fields.insert(
        "notifications".into(),
        Stamp::new(now_ms() + 1_000, 0, "aaaaaaaa"),
    );
    right.sync_apply(&records, &peer("old")).unwrap();
    let config = &right.doc.settings.notifications;
    assert_eq!(config.default_time, "07:15");
    let loan = config.category(NoticeCategory::Loan);
    assert!(!loan.enabled);
    assert_eq!(loan.lead_days, 2);
    assert!(config.system, "the old device's switch is not taken");
    assert!(!config.hide_details_when_locked);
    // What this side sends still carries a head stamp an old peer reads.
    let sent = right.sync_records(&["settings:".to_string()]).unwrap();
    let sent = &sent["settings:"];
    assert!(
        sent.stamps
            .as_ref()
            .unwrap()
            .fields
            .contains_key("notifications")
    );
    let object = &sent.value.as_ref().unwrap()["notifications"];
    assert!(object["categories"].is_array(), "{object}");
    assert!(object.get("system").is_none(), "{object}");
}

/// A vault file saved before the split keeps its notifications: the stored
/// shape did not change.
#[test]
fn notifications_saved_by_an_older_build_load_intact() {
    let (_dir, mut vault, _right) = two_devices();
    let stored = json!({
        "system": false,
        "hide_details_when_locked": true,
        "default_time": "06:45",
        "categories": [{
            "category": "rent", "enabled": true, "lead_days": 3, "auto_reminders": false
        }],
    });
    let mut doc = vault.doc_value().unwrap();
    doc["settings"]["notifications"] = stored.clone();
    vault.doc = serde_json::from_value(doc).unwrap();
    vault.persist().unwrap();
    let reopened = UnlockedVault::open(vault.path.clone(), "pw").unwrap();
    let config = &reopened.doc.settings.notifications;
    assert!(!config.system);
    assert!(config.hide_details_when_locked);
    assert_eq!(config.default_time, "06:45");
    assert_eq!(
        config.categories,
        vec![NoticeCategoryCfg {
            category: NoticeCategory::Rent,
            enabled: true,
            lead_days: 3,
            auto_reminders: false,
        }]
    );
    assert_eq!(serde_json::to_value(config).unwrap(), stored);
}
