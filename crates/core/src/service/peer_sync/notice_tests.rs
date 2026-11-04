//! A batch shows only the notices it added: a notice another device merely
//! updated (read) is not shown again.

use super::tests::{exchange, settle, two_devices};
use crate::model::notify::{NoticeCategory, NoticeRecord};

fn notice(key: &str) -> NoticeRecord {
    NoticeRecord {
        key: key.into(),
        category: NoticeCategory::Budget,
        title: key.into(),
        body: String::new(),
        link: None,
        at: "2026-11-01T10:00:00Z".into(),
        delivered_at: "2026-11-01T10:00:00Z".into(),
        read: false,
    }
}

fn titles(notices: Vec<NoticeRecord>) -> Vec<String> {
    let mut keys: Vec<String> = notices.into_iter().map(|record| record.key).collect();
    keys.sort();
    keys
}

#[test]
fn a_batch_lists_the_notices_it_added() {
    let (_dir, mut left, mut right) = two_devices();
    right.doc.notification_log.push(notice("reminder:a"));
    let (applied, _) = exchange(&mut left, &mut right);
    let batch = applied.expect("batch").id;
    assert_eq!(titles(left.sync_batch_notices(&batch)), vec!["reminder:a"]);
}

#[test]
fn a_notice_that_was_only_read_is_not_listed_again() {
    let (_dir, mut left, mut right) = two_devices();
    right.doc.notification_log.push(notice("reminder:a"));
    settle(&mut left, &mut right);
    right.doc.notification_log[0].read = true;
    right.doc.notification_log.push(notice("reminder:b"));
    right.persist().unwrap();
    let (applied, _) = exchange(&mut left, &mut right);
    let batch = applied.expect("batch").id;
    assert!(left.doc.notification_log.iter().any(|record| record.read));
    assert_eq!(titles(left.sync_batch_notices(&batch)), vec!["reminder:b"]);
}
