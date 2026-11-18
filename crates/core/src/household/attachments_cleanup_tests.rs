//! Tests of the pure cleanup decisions, on a bare document.

use super::attachments::mark_removed;
use super::attachments_cleanup::*;
use super::attachments_tests::{bare_doc, link_in};
use crate::model::attachment::AttachmentKey;
use crate::model::entity_ref::{EntityKind, EntityRef};

#[test]
fn removed_records_and_owners_gone_for_good_go() {
    let (mut doc, owner) = bare_doc();
    let orphan = EntityRef::new(EntityKind::Tx, "t-gone");
    link_in(&mut doc, &owner, "live");
    link_in(&mut doc, &owner, "removed");
    mark_removed(&mut doc, "removed", "2026-02-01T00:00:00Z").unwrap();
    link_in(&mut doc, &orphan, "orphan");

    assert_eq!(gone_attachment_ids(&doc, |_| true), vec!["removed"]);
    assert_eq!(
        gone_attachment_ids(&doc, |_| false),
        vec!["removed", "orphan"]
    );
}

#[test]
fn drop_attachments_removes_records_and_keys() {
    let (mut doc, owner) = bare_doc();
    link_in(&mut doc, &owner, "a1");
    link_in(&mut doc, &owner, "a2");
    for id in ["a1", "a2"] {
        doc.attachment_keys.push(AttachmentKey {
            id: id.into(),
            key: "k".into(),
        });
    }
    drop_attachments(&mut doc, &["a1".to_string()]);
    assert_eq!(named_ids(&doc), ["a2"].into_iter().collect());
    assert_eq!(doc.attachment_keys.len(), 1);
    assert_eq!(doc.attachment_keys[0].id, "a2");
}

#[test]
fn only_money_owners_have_a_history_key() {
    let key = |kind| owner_history_key(&EntityRef::new(kind, "x1"));
    assert_eq!(key(EntityKind::Tx).as_deref(), Some("transactions:x1"));
    assert_eq!(key(EntityKind::Goal).as_deref(), Some("goals:x1"));
    assert_eq!(key(EntityKind::Store), None);
}

#[test]
fn a_file_name_is_a_canonical_uuid() {
    let id = "67e55044-10b1-426f-9247-bb680e5fe0c8";
    assert!(is_file_name(id));
    assert!(!is_file_name(&id.to_uppercase()));
    assert!(!is_file_name(&id.replace('-', "")));
    assert!(!is_file_name(&format!("{{{id}}}")));
    assert!(!is_file_name("../x"));
    assert!(!is_file_name(""));
}
