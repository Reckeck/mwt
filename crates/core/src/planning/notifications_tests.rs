//! Tests of the notifications area: the old inline tests (names and bodies unchanged).

use super::notifications::*;
use crate::model::notify::{MAX_NOTICE_LOG, NoticeCategory, NoticeRecord};
use crate::model::vault::now_iso;
use crate::service::test_support::*;
use crate::usecases::planning::notifications::notice;

#[test]
fn a_notice_shows_money_rounded_and_grouped() {
    let amount = |text: &str| crate::Amount(text.parse().unwrap());
    assert_eq!(
        money_text(amount("22767.963690399998338875775458"), true),
        "22\u{a0}767,96"
    );
    assert_eq!(money_text(amount("15000.00"), true), "15\u{a0}000");
    assert_eq!(money_text(amount("-1234.5"), false), "-1\u{a0}234.50");
}

use crate::model::notify::NoticeCategoryCfg;
use crate::pricing::history::secs_or_now;

#[test]
fn a_due_reminder_is_told_once_and_survives_in_the_log() {
    let (_dir, mut vault) = empty_vault();
    vault
        .create_reminder("Позвонить в банк", "2026-03-05T09:00:00Z", None)
        .unwrap();
    let now = secs_or_now("2026-03-05T09:00:30Z");
    let first = vault.due_notices(now, "UTC", true).unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].category, NoticeCategory::Reminder);
    assert_eq!(first[0].body, "Позвонить в банк");
    assert!(vault.due_notices(now + 60, "UTC", true).unwrap().is_empty());
    assert_eq!(vault.unread_notifications(), 1);
    vault.mark_notifications_read(None).unwrap();
    assert_eq!(vault.unread_notifications(), 0);
    assert_eq!(vault.notifications().len(), 1);
}

#[test]
fn a_switched_off_kind_is_silent_and_an_old_reminder_is_not_announced() {
    let (_dir, mut vault) = empty_vault();
    vault
        .create_reminder("Old", "2026-01-01T09:00:00Z", None)
        .unwrap();
    let now = secs_or_now("2026-03-05T09:00:00Z");
    assert!(vault.due_notices(now, "UTC", false).unwrap().is_empty());

    vault
        .create_reminder("Fresh", "2026-03-05T08:00:00Z", None)
        .unwrap();
    vault
        .doc
        .settings
        .notifications
        .categories
        .push(NoticeCategoryCfg {
            category: NoticeCategory::Reminder,
            enabled: false,
            lead_days: 0,
            auto_reminders: true,
        });
    assert!(vault.due_notices(now, "UTC", false).unwrap().is_empty());
}

#[test]
fn the_log_keeps_the_newest_and_never_repeats_a_key() {
    let (_dir, mut vault) = empty_vault();
    let record = |index: usize| {
        notice(
            NoticeCategory::PriceAlert,
            format!("alert:{index}"),
            "Alert",
            String::new(),
            None,
            now_iso(),
        )
    };
    assert!(vault.record_notice(record(1)).unwrap());
    assert!(!vault.record_notice(record(1)).unwrap());
    let many: Vec<NoticeRecord> = (0..MAX_NOTICE_LOG + 10).map(record).collect();
    vault.log_notices(many).unwrap();
    assert_eq!(vault.doc.notification_log.len(), MAX_NOTICE_LOG);
    assert_eq!(
        vault.notifications()[0].key,
        format!("alert:{}", MAX_NOTICE_LOG + 9)
    );
}

#[test]
fn notices_are_wanted_until_their_kind_is_switched_off() {
    let (_dir, mut vault) = empty_vault();
    assert!(vault.notices_enabled(NoticeCategory::Budget));
    vault
        .doc
        .settings
        .notifications
        .categories
        .push(crate::model::notify::NoticeCategoryCfg {
            category: NoticeCategory::Budget,
            enabled: false,
            lead_days: 0,
            auto_reminders: true,
        });
    assert!(!vault.notices_enabled(NoticeCategory::Budget));
    assert!(!notices_enabled(&vault.doc, NoticeCategory::Budget));
}

#[test]
fn fresh_notices_skip_known_keys_and_the_log_is_capped() {
    let (_dir, mut vault) = empty_vault();
    let record = |key: &str| {
        notice_at(
            NoticeCategory::PriceAlert,
            key.into(),
            "Alert",
            String::new(),
            None,
            "2026-03-05T09:00:00Z".into(),
            "2026-03-05T09:00:01Z".into(),
        )
    };
    let first = fresh_notices(&vault.doc, vec![record("a"), record("a"), record("b")]);
    assert_eq!(first.len(), 2);
    let mut log = Vec::new();
    append_notices(&mut log, &first);
    vault.doc.notification_log = log;
    assert!(fresh_notices(&vault.doc, vec![record("a")]).is_empty());
    let mut many: Vec<NoticeRecord> = (0..MAX_NOTICE_LOG + 5)
        .map(|index| record(&index.to_string()))
        .collect();
    let extra = many.split_off(MAX_NOTICE_LOG);
    append_notices(&mut many, &extra);
    assert_eq!(many.len(), MAX_NOTICE_LOG);
}
