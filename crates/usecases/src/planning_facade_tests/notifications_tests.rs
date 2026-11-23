//! Tests of the notifications area: the old inline tests (names and bodies unchanged).

use crate::model::notify::{MAX_NOTICE_LOG, NoticeCategory, NoticeRecord};
use crate::model::vault::now_iso;
use crate::planning::notifications::*;
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
use crate::time::secs_or_now;

#[test]
fn a_due_reminder_is_told_once_and_survives_in_the_log() {
    let (_dir, mut vault) = empty_vault();
    crate::usecases::planning::agenda::create_reminder(
        &mut vault,
        "Позвонить в банк",
        "2026-03-05T09:00:00Z",
        None,
    )
    .unwrap();
    let now = secs_or_now("2026-03-05T09:00:30Z");
    let first = crate::usecases::planning::notifications::due_notices(&mut vault, now, "UTC", true)
        .unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].category, NoticeCategory::Reminder);
    assert_eq!(first[0].body, "Позвонить в банк");
    assert!(
        crate::usecases::planning::notifications::due_notices(&mut vault, now + 60, "UTC", true)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        crate::planning::notifications::unread_notifications(&vault.doc),
        1
    );
    crate::usecases::planning::notifications::mark_notifications_read(&mut vault, None).unwrap();
    assert_eq!(
        crate::planning::notifications::unread_notifications(&vault.doc),
        0
    );
    assert_eq!(
        crate::planning::notifications::notifications(&vault.doc).len(),
        1
    );
}

#[test]
fn a_switched_off_kind_is_silent_and_an_old_reminder_is_not_announced() {
    let (_dir, mut vault) = empty_vault();
    crate::usecases::planning::agenda::create_reminder(
        &mut vault,
        "Old",
        "2026-01-01T09:00:00Z",
        None,
    )
    .unwrap();
    let now = secs_or_now("2026-03-05T09:00:00Z");
    assert!(
        crate::usecases::planning::notifications::due_notices(&mut vault, now, "UTC", false)
            .unwrap()
            .is_empty()
    );

    crate::usecases::planning::agenda::create_reminder(
        &mut vault,
        "Fresh",
        "2026-03-05T08:00:00Z",
        None,
    )
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
    assert!(
        crate::usecases::planning::notifications::due_notices(&mut vault, now, "UTC", false)
            .unwrap()
            .is_empty()
    );
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
    assert!(
        crate::usecases::planning::notifications::record_notice(&mut vault, record(1)).unwrap()
    );
    assert!(
        !crate::usecases::planning::notifications::record_notice(&mut vault, record(1)).unwrap()
    );
    let many: Vec<NoticeRecord> = (0..MAX_NOTICE_LOG + 10).map(record).collect();
    crate::usecases::planning::notifications::log_notices(&mut vault, many).unwrap();
    assert_eq!(vault.doc.notification_log.len(), MAX_NOTICE_LOG);
    assert_eq!(
        crate::planning::notifications::notifications(&vault.doc)[0].key,
        format!("alert:{}", MAX_NOTICE_LOG + 9)
    );
}

#[test]
fn notices_are_wanted_until_their_kind_is_switched_off() {
    let (_dir, mut vault) = empty_vault();
    assert!(crate::planning::notifications::notices_enabled(
        &vault.doc,
        NoticeCategory::Budget
    ));
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
    assert!(!crate::planning::notifications::notices_enabled(
        &vault.doc,
        NoticeCategory::Budget
    ));
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

fn record(key: &str, category: NoticeCategory) -> NoticeRecord {
    NoticeRecord {
        key: key.into(),
        category,
        title: "Title".into(),
        body: "Body".into(),
        link: None,
        at: "2026-03-05T09:00:00Z".into(),
        delivered_at: "2026-03-05T09:00:30Z".into(),
        read: false,
    }
}

#[test]
fn the_notice_delegates_equal_the_scenarios() {
    use crate::usecases::planning::notifications as scenario;
    let (_a, mut by_method) = empty_vault();
    let (_b, mut by_scenario) = empty_vault();
    let batch = || {
        vec![
            record("a", NoticeCategory::Reminder),
            record("a", NoticeCategory::Reminder),
            record("b", NoticeCategory::PriceAlert),
        ]
    };
    let method =
        crate::usecases::planning::notifications::log_notices(&mut by_method, batch()).unwrap();
    let made = scenario::log_notices(&mut by_scenario, batch()).unwrap();
    assert_eq!(method, made);
    assert_eq!(method.len(), 2);
    crate::usecases::planning::notifications::mark_notifications_read(
        &mut by_method,
        Some(&["a".to_string()]),
    )
    .unwrap();
    scenario::mark_notifications_read(&mut by_scenario, Some(&["a".to_string()])).unwrap();
    assert_eq!(
        crate::planning::notifications::notifications(&by_method.doc),
        crate::planning::notifications::notifications(&by_scenario.doc)
    );
    assert_eq!(
        crate::planning::notifications::unread_notifications(&by_method.doc),
        1
    );
    assert!(
        crate::usecases::planning::notifications::record_notice(
            &mut by_method,
            record("c", NoticeCategory::Reminder)
        )
        .unwrap()
    );
    assert!(
        scenario::record_notice(&mut by_scenario, record("c", NoticeCategory::Reminder)).unwrap()
    );
    assert!(
        !crate::usecases::planning::notifications::record_notice(
            &mut by_method,
            record("c", NoticeCategory::Reminder)
        )
        .unwrap()
    );
    assert!(
        !scenario::record_notice(&mut by_scenario, record("c", NoticeCategory::Reminder)).unwrap()
    );
    assert!(
        crate::usecases::planning::notifications::record_notice(
            &mut by_method,
            record(" ", NoticeCategory::Reminder)
        )
        .is_err()
    );
    assert!(
        scenario::record_notice(&mut by_scenario, record(" ", NoticeCategory::Reminder)).is_err()
    );
    assert_eq!(
        crate::planning::notifications::notifications(&by_method.doc),
        crate::planning::notifications::notifications(&by_scenario.doc)
    );
}

#[test]
fn the_due_sweep_delegate_equals_the_scenario() {
    use crate::usecases::planning::notifications as scenario;
    let (_a, mut by_method) = empty_vault();
    let (_b, mut by_scenario) = empty_vault();
    for vault in [&mut by_method, &mut by_scenario] {
        crate::usecases::planning::agenda::create_reminder(
            vault,
            "Позвонить в банк",
            "2026-03-05T09:00:00Z",
            None,
        )
        .unwrap();
    }
    let now = secs_or_now("2026-03-05T09:00:30Z");
    let keys = |notices: Vec<NoticeRecord>| -> Vec<(String, String)> {
        notices
            .into_iter()
            .map(|notice| (format!("{:?}", notice.category), notice.body))
            .collect()
    };
    let method = keys(
        crate::usecases::planning::notifications::due_notices(&mut by_method, now, "UTC", true)
            .unwrap(),
    );
    let made = keys(scenario::due_notices(&mut by_scenario, now, "UTC", true).unwrap());
    assert_eq!(method, made);
    assert_eq!(method.len(), 1);
}

#[test]
fn the_log_rules_work_on_a_bare_document() {
    let mut doc = crate::model::vault::VaultDoc::default();
    let fresh = fresh_notices(
        &doc,
        vec![
            record("a", NoticeCategory::Reminder),
            record("a", NoticeCategory::Reminder),
        ],
    );
    assert_eq!(fresh.len(), 1);
    append_notices(&mut doc.notification_log, &fresh);
    assert!(fresh_notices(&doc, vec![record("a", NoticeCategory::Reminder)]).is_empty());
    assert_eq!(unread_notifications(&doc), 1);
    mark_read(&mut doc.notification_log, Some(&["other".to_string()]));
    assert_eq!(unread_notifications(&doc), 1);
    mark_read(&mut doc.notification_log, None);
    assert_eq!(unread_notifications(&doc), 0);
    let full: Vec<NoticeRecord> = (0..MAX_NOTICE_LOG + 3)
        .map(|at| record(&format!("k{at}"), NoticeCategory::Reminder))
        .collect();
    let mut log = Vec::new();
    append_notices(&mut log, &full);
    assert_eq!(log.len(), MAX_NOTICE_LOG);
    assert_eq!(log[0].key, "k3");
}
