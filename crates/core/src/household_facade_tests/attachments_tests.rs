//! Tests of the attachments area, kept as they were in the service module.

use crate::error::Error;
use crate::household::attachments::*;
use crate::model::attachment::MAX_ATTACHMENT_BYTES;
use crate::model::attachment::{Attachment, AttachmentKey};
use crate::model::entity_ref::{EntityKind, EntityRef};
use crate::model::planning::Goal;
use crate::model::store::StoreScope;
use crate::model::vault::VaultDoc;
use crate::service::Session;
use crate::service::test_support::*;

fn tx_owner(vault: &mut Session) -> EntityRef {
    let card = store(vault, "Card", crate::model::store::StoreKind::Bank, None);
    let tx = crate::usecases::money::transactions::add_transaction(
        vault,
        income(&card, "100", "salary"),
    )
    .unwrap();
    EntityRef::new(EntityKind::Tx, tx.meta().id.clone())
}

fn receipt<'a>(bytes: &'a [u8]) -> NewFile<'a> {
    NewFile {
        name: "чек.pdf",
        mime: Some("application/pdf"),
        bytes,
    }
}

#[test]
fn a_file_is_sealed_next_to_the_vault_and_opens_back() {
    let (_dir, mut vault) = setup();
    let owner = tx_owner(&mut vault);
    let item = crate::usecases::household::attachments::add_attachment_file(
        &mut vault,
        owner.clone(),
        receipt(b"%PDF-1.7 receipt"),
    )
    .unwrap();

    let on_disk = std::fs::read(
        crate::usecases::household::attachments::attachments_dir(&vault).join(&item.id),
    )
    .unwrap();
    assert!(
        !on_disk.windows(7).any(|part| part == b"receipt"),
        "the file is not sealed"
    );
    let (_, bytes) =
        crate::usecases::household::attachments::read_attachment(&vault, &item.id).unwrap();
    assert_eq!(bytes, b"%PDF-1.7 receipt");
    assert_eq!(item.size, 16);
    assert_eq!(
        crate::household::attachments::list_attachments(&vault.doc, Some(&owner)),
        vec![item.clone()]
    );
    assert_eq!(
        crate::household::attachments::transactions_with_attachments(&vault.doc),
        vec![owner.id.clone()]
    );

    // The same file again is the same attachment.
    let again = crate::usecases::household::attachments::add_attachment_file(
        &mut vault,
        owner,
        receipt(b"%PDF-1.7 receipt"),
    )
    .unwrap();
    assert_eq!(again.id, item.id);
}

#[test]
fn a_new_password_keeps_the_files_readable() {
    let (_dir, mut vault) = setup();
    let owner = tx_owner(&mut vault);
    let item = crate::usecases::household::attachments::add_attachment_file(
        &mut vault,
        owner,
        receipt(b"bytes"),
    )
    .unwrap();
    vault.flush_pending().unwrap();
    let path = vault.path.clone();
    vault.change_password("pw", "another").unwrap();
    drop(vault);
    let reopened = Session::open(path, "another").unwrap();
    assert_eq!(
        crate::usecases::household::attachments::read_attachment(&reopened, &item.id)
            .unwrap()
            .1,
        b"bytes"
    );
}

#[test]
fn removal_waits_for_undo_until_the_next_open() {
    let (_dir, mut vault) = setup();
    let owner = tx_owner(&mut vault);
    let item = crate::usecases::household::attachments::add_attachment_file(
        &mut vault,
        owner.clone(),
        receipt(b"bytes"),
    )
    .unwrap();
    let file = crate::usecases::household::attachments::attachments_dir(&vault).join(&item.id);

    crate::usecases::household::attachments::remove_attachment(&mut vault, &item.id).unwrap();
    assert!(crate::household::attachments::list_attachments(&vault.doc, Some(&owner)).is_empty());
    assert!(file.exists(), "undo still needs the file");
    crate::usecases::household::attachments::restore_attachment(&mut vault, &item.id).unwrap();
    assert_eq!(
        crate::household::attachments::list_attachments(&vault.doc, Some(&owner)).len(),
        1
    );

    crate::usecases::household::attachments::remove_attachment(&mut vault, &item.id).unwrap();
    vault.flush_pending().unwrap();
    let path = vault.path.clone();
    drop(vault);
    let reopened = Session::open(path, "pw").unwrap();
    assert!(reopened.doc.attachments.is_empty());
    assert!(reopened.doc.attachment_keys.is_empty());
    assert!(!file.exists());
}

#[test]
fn a_deleted_operation_takes_its_files_on_the_next_open() {
    let (_dir, mut vault) = setup();
    let owner = tx_owner(&mut vault);
    let item = crate::usecases::household::attachments::add_attachment_file(
        &mut vault,
        owner.clone(),
        receipt(b"bytes"),
    )
    .unwrap();
    crate::usecases::household::attachments::add_attachment_link(
        &mut vault,
        owner.clone(),
        "bank.ru/check/1",
        "",
    )
    .unwrap();
    let file = crate::usecases::household::attachments::attachments_dir(&vault).join(&item.id);
    crate::usecases::money::transactions::delete_transaction(&mut vault, &owner.id).unwrap();
    vault.flush_pending().unwrap();
    let path = vault.path.clone();
    drop(vault);
    let reopened = Session::open(path, "pw").unwrap();
    assert!(reopened.doc.attachments.is_empty());
    assert!(!file.exists());
}

#[test]
fn links_are_web_addresses_with_a_caption() {
    let (_dir, mut vault) = setup();
    let owner = tx_owner(&mut vault);
    let link = crate::usecases::household::attachments::add_attachment_link(
        &mut vault,
        owner.clone(),
        " bank.ru/check/1 ",
        "",
    )
    .unwrap();
    assert_eq!(link.url.as_deref(), Some("https://bank.ru/check/1"));
    assert_eq!(link.title, "https://bank.ru/check/1");
    let named = crate::usecases::household::attachments::add_attachment_link(
        &mut vault,
        owner.clone(),
        "https://x.io",
        "Договор",
    )
    .unwrap();
    assert_eq!(named.title, "Договор");
    assert!(
        crate::usecases::household::attachments::add_attachment_link(
            &mut vault,
            owner.clone(),
            "file:///etc/passwd",
            ""
        )
        .is_err()
    );
    assert!(
        crate::usecases::household::attachments::add_attachment_link(
            &mut vault,
            owner,
            "not an address",
            ""
        )
        .is_err()
    );
    assert!(crate::usecases::household::attachments::read_attachment(&vault, &link.id).is_err());
}

#[test]
fn a_renamed_file_is_found_by_its_new_name() {
    let (_dir, mut vault) = setup();
    let owner = tx_owner(&mut vault);
    let item = crate::usecases::household::attachments::add_attachment_file(
        &mut vault,
        owner.clone(),
        receipt(b"bytes"),
    )
    .unwrap();
    crate::usecases::household::attachments::rename_attachment(
        &mut vault,
        &item.id,
        "  Чек skycapital  ",
    )
    .unwrap();
    assert!(
        crate::usecases::household::attachments::rename_attachment(&mut vault, &item.id, " ")
            .is_err()
    );
    let filter = crate::money::ledger::TxFilter {
        text: Some("skycapital".into()),
        ..Default::default()
    };
    let found =
        crate::usecases::household::ledger::list_transactions(&vault, &filter, "UTC").unwrap();
    assert_eq!(found.len(), 1);
    let only_attached = crate::money::ledger::TxFilter {
        with_attachments: true,
        ..Default::default()
    };
    assert_eq!(
        crate::usecases::household::ledger::list_transactions(&vault, &only_attached, "UTC")
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn only_operations_and_goals_that_exist_take_files() {
    let (_dir, mut vault) = setup();
    let missing = EntityRef::new(EntityKind::Tx, "nope");
    assert!(
        crate::usecases::household::attachments::add_attachment_link(
            &mut vault,
            missing,
            "https://x.io",
            ""
        )
        .is_err()
    );
    let store = EntityRef::new(EntityKind::Store, vault.doc.stores[0].id.clone());
    assert!(
        crate::usecases::household::attachments::add_attachment_link(
            &mut vault,
            store,
            "https://x.io",
            ""
        )
        .is_err()
    );
    let owner = tx_owner(&mut vault);
    let big = vec![0u8; MAX_ATTACHMENT_BYTES + 1];
    assert!(matches!(
        crate::usecases::household::attachments::add_attachment_file(
            &mut vault,
            owner,
            receipt(&big)
        ),
        Err(Error::TooLarge(_))
    ));
}

// Pure functions on a bare document, no vault.

fn goal(id: &str) -> Goal {
    Goal {
        id: id.into(),
        name: "Trip".into(),
        active: true,
        target_amount: amount("1000"),
        currency: "USD".into(),
        starts_at: "2026-01-01".into(),
        deadline: None,
        priority: 3,
        monthly_commitment: None,
        scope: StoreScope {
            store_ids: vec![],
            include_children: true,
        },
        created_at: "2026-01-01T00:00:00Z".into(),
        scenario_id: None,
        expected_return: None,
    }
}

/// A document with one goal, `g1`, and its owner reference.
pub(super) fn bare_doc() -> (VaultDoc, EntityRef) {
    let mut doc = VaultDoc::default();
    doc.goals.push(goal("g1"));
    (doc, EntityRef::new(EntityKind::Goal, "g1"))
}

const NOW: &str = "2026-02-01T00:00:00Z";

pub(super) fn link_in(doc: &mut VaultDoc, owner: &EntityRef, id: &str) -> Attachment {
    // Built against the bare owner, so a record may name an owner the doc lacks.
    let mut item = plan_link(
        &bare_doc().0,
        bare_doc().1,
        "example.com",
        "",
        id.into(),
        NOW,
    )
    .unwrap();
    item.owner = owner.clone();
    doc.attachments.push(item.clone());
    item
}

#[test]
fn plan_link_normalises_the_address_and_defaults_the_title() {
    let (doc, owner) = bare_doc();
    let item = plan_link(
        &doc,
        owner.clone(),
        " example.com/x ",
        "  ",
        "a1".into(),
        NOW,
    )
    .unwrap();
    assert_eq!(item.url.as_deref(), Some("https://example.com/x"));
    assert_eq!(item.title, "https://example.com/x");
    assert_eq!(item.owner, owner);
    assert_eq!(item.created_at, NOW);
    let kept = plan_link(
        &doc,
        owner.clone(),
        "mailto:a@b.c",
        " Mail ",
        "a2".into(),
        NOW,
    )
    .unwrap();
    assert_eq!(kept.url.as_deref(), Some("mailto:a@b.c"));
    assert_eq!(kept.title, "Mail");
    for bad in ["", "a b.com", "ftp://x.com", "localhost"] {
        assert!(
            matches!(
                plan_link(&doc, owner.clone(), bad, "", "a3".into(), NOW),
                Err(Error::InvalidInput(_))
            ),
            "{bad}"
        );
    }
}

#[test]
fn plan_link_checks_the_owner() {
    let (doc, _) = bare_doc();
    let missing = EntityRef::new(EntityKind::Goal, "nope");
    let wrong_kind = EntityRef::new(EntityKind::Store, "g1");
    for owner in [missing, wrong_kind] {
        assert!(plan_link(&doc, owner.clone(), "a.com", "", "a1".into(), NOW).is_err());
    }
}

#[test]
fn plan_file_checks_name_bytes_and_size() {
    let (doc, owner) = bare_doc();
    let plan = |name, bytes: &[u8]| {
        let file = NewFile {
            name,
            mime: None,
            bytes,
        };
        plan_file(&doc, owner.clone(), &file, "a1".into(), NOW)
    };
    assert!(matches!(plan(" ", b"x"), Err(Error::InvalidInput(_))));
    assert!(matches!(plan("a", b""), Err(Error::InvalidInput(_))));
    let big = vec![0u8; MAX_ATTACHMENT_BYTES + 1];
    assert!(matches!(plan("a", &big), Err(Error::TooLarge(_))));
    let Ok(FilePlan::New(item)) = plan(" a.pdf ", b"abc") else {
        panic!("expected a new file")
    };
    assert_eq!(item.title, "a.pdf");
    assert_eq!(item.size, 3);
    assert_eq!(item.mime, None);
    assert_eq!(
        item.sha256.as_deref(),
        Some("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
    );
}

#[test]
fn plan_file_checks_the_owner_before_the_file() {
    let (doc, _) = bare_doc();
    let file = NewFile {
        name: " ",
        mime: None,
        bytes: b"",
    };
    let missing = EntityRef::new(EntityKind::Goal, "nope");
    let wrong_kind = EntityRef::new(EntityKind::Store, "g1");
    assert!(matches!(
        plan_file(&doc, missing, &file, "a1".into(), NOW),
        Err(Error::Missing(_))
    ));
    assert!(matches!(
        plan_file(&doc, wrong_kind, &file, "a1".into(), NOW),
        Err(Error::InvalidInput(_))
    ));
}

#[test]
fn plan_file_gives_back_the_same_file_of_the_same_owner() {
    let (mut doc, owner) = bare_doc();
    let file = NewFile {
        name: "a",
        mime: Some(" "),
        bytes: b"abc",
    };
    let Ok(FilePlan::New(item)) = plan_file(&doc, owner.clone(), &file, "a1".into(), NOW) else {
        panic!("expected a new file")
    };
    assert_eq!(item.mime, None);
    doc.attachments.push(item.clone());
    let Ok(FilePlan::Existing(same)) = plan_file(&doc, owner.clone(), &file, "a2".into(), NOW)
    else {
        panic!("expected the existing record")
    };
    assert_eq!(same.id, "a1");
    // A removed record does not count; another owner gets its own.
    doc.attachments[0].removed_at = Some(NOW.into());
    assert!(matches!(
        plan_file(&doc, owner, &file, "a3".into(), NOW),
        Ok(FilePlan::New(_))
    ));
}

#[test]
fn mark_functions_change_the_record_and_refuse_missing_ids() {
    let (mut doc, owner) = bare_doc();
    link_in(&mut doc, &owner, "a1");
    assert_eq!(mark_renamed(&mut doc, "a1", " New ").unwrap().title, "New");
    assert!(matches!(
        mark_renamed(&mut doc, "a1", " "),
        Err(Error::InvalidInput(_))
    ));
    assert_eq!(
        mark_removed(&mut doc, "a1", NOW)
            .unwrap()
            .removed_at
            .as_deref(),
        Some(NOW)
    );
    // A removed id is not found by remove or rename, but restore finds it.
    assert!(matches!(
        mark_removed(&mut doc, "a1", NOW),
        Err(Error::Missing(_))
    ));
    assert!(matches!(
        mark_renamed(&mut doc, "a1", "x"),
        Err(Error::Missing(_))
    ));
    assert_eq!(mark_restored(&mut doc, "a1").unwrap().removed_at, None);
    assert!(matches!(
        mark_removed(&mut doc, "zz", NOW),
        Err(Error::Missing(_))
    ));
    assert!(matches!(
        mark_renamed(&mut doc, "zz", "x"),
        Err(Error::Missing(_))
    ));
    assert!(matches!(
        mark_restored(&mut doc, "zz"),
        Err(Error::Missing(_))
    ));
}

#[test]
fn the_household_file_key_length_is_the_crypto_one() {
    assert_eq!(KEY_LEN, crate::crypto::KEY_LEN);
}

#[test]
fn the_file_key_round_trips_through_the_document() {
    let (mut doc, _) = bare_doc();
    let key = [7u8; crate::crypto::KEY_LEN];
    doc.attachment_keys.push(AttachmentKey {
        id: "a1".into(),
        key: encode_key(&key),
    });
    assert_eq!(*attachment_key(&doc, "a1").unwrap(), key);
    assert!(matches!(attachment_key(&doc, "a2"), Err(Error::Crypto(_))));
    doc.attachment_keys.push(AttachmentKey {
        id: "a3".into(),
        key: "!!".into(),
    });
    doc.attachment_keys.push(AttachmentKey {
        id: "a4".into(),
        key: encode_key(&key)[..8].to_string(),
    });
    assert!(matches!(attachment_key(&doc, "a3"), Err(Error::Crypto(_))));
    assert!(matches!(attachment_key(&doc, "a4"), Err(Error::Crypto(_))));
}

#[test]
fn the_link_delegate_matches_the_scenario() {
    let (_dir, mut vault) = setup();
    let owner = tx_owner(&mut vault);
    let via_delegate = crate::usecases::household::attachments::add_attachment_link(
        &mut vault,
        owner.clone(),
        "example.com",
        " Docs ",
    )
    .unwrap();
    let via_scenario = crate::usecases::household::attachments::add_attachment_link(
        &mut vault,
        owner.clone(),
        "example.com",
        " Docs ",
    )
    .unwrap();
    let strip = |item: &Attachment| Attachment {
        id: String::new(),
        created_at: String::new(),
        ..item.clone()
    };
    assert_eq!(strip(&via_delegate), strip(&via_scenario));
    assert_eq!(
        crate::household::attachments::list_attachments(&vault.doc, Some(&owner)).len(),
        2
    );
}

#[test]
fn the_file_delegate_matches_the_scenario() {
    let (_dir, mut vault) = setup();
    let owner = tx_owner(&mut vault);
    // Different bytes, so both calls take the fresh-write path (seal, write, key record).
    let via_delegate = crate::usecases::household::attachments::add_attachment_file(
        &mut vault,
        owner.clone(),
        receipt(b"%PDF-1 receipt one"),
    )
    .unwrap();
    let via_scenario = crate::usecases::household::attachments::add_attachment_file(
        &mut vault,
        owner.clone(),
        receipt(b"%PDF-1 receipt two"),
    )
    .unwrap();
    assert_ne!(via_delegate.id, via_scenario.id);
    let strip = |item: &Attachment| Attachment {
        id: String::new(),
        created_at: String::new(),
        sha256: None,
        ..item.clone()
    };
    assert_eq!(strip(&via_delegate), strip(&via_scenario));
    assert_ne!(via_delegate.sha256, via_scenario.sha256);

    let (read_delegate, bytes_delegate) =
        crate::usecases::household::attachments::read_attachment(&vault, &via_delegate.id).unwrap();
    let (read_scenario, bytes_scenario) =
        crate::usecases::household::attachments::read_attachment(&vault, &via_scenario.id).unwrap();
    assert_eq!(strip(&read_delegate), strip(&read_scenario));
    assert_eq!(bytes_delegate, b"%PDF-1 receipt one");
    assert_eq!(bytes_scenario, b"%PDF-1 receipt two");
    assert_eq!(
        crate::household::attachments::list_attachments(&vault.doc, Some(&owner)).len(),
        2
    );
}

#[test]
fn the_remove_rename_restore_delegates_match_their_scenarios() {
    use crate::usecases::household::attachments as scenario;
    let (_dir, mut vault) = setup();
    let owner = tx_owner(&mut vault);
    let first = crate::usecases::household::attachments::add_attachment_link(
        &mut vault,
        owner.clone(),
        "example.com",
        "One",
    )
    .unwrap();
    let second = crate::usecases::household::attachments::add_attachment_link(
        &mut vault,
        owner.clone(),
        "example.org",
        "Two",
    )
    .unwrap();

    let removed_a =
        crate::usecases::household::attachments::remove_attachment(&mut vault, &first.id).unwrap();
    let removed_b = scenario::remove_attachment(&mut vault, &second.id).unwrap();
    assert!(removed_a.removed_at.is_some() && removed_b.removed_at.is_some());

    let restored_a =
        crate::usecases::household::attachments::restore_attachment(&mut vault, &first.id).unwrap();
    let restored_b = scenario::restore_attachment(&mut vault, &second.id).unwrap();
    assert!(restored_a.removed_at.is_none() && restored_b.removed_at.is_none());

    let renamed_a =
        crate::usecases::household::attachments::rename_attachment(&mut vault, &first.id, " New ")
            .unwrap();
    let renamed_b = scenario::rename_attachment(&mut vault, &second.id, " New ").unwrap();
    assert_eq!(renamed_a.title, renamed_b.title);

    assert!(matches!(
        crate::usecases::household::attachments::remove_attachment(&mut vault, "missing"),
        Err(Error::Missing(_))
    ));
    assert!(matches!(
        crate::usecases::household::attachments::rename_attachment(&mut vault, "missing", "x"),
        Err(Error::Missing(_))
    ));
    assert!(matches!(
        crate::usecases::household::attachments::restore_attachment(&mut vault, "missing"),
        Err(Error::Missing(_))
    ));
    assert!(matches!(
        scenario::remove_attachment(&mut vault, "missing"),
        Err(Error::Missing(_))
    ));
    assert!(matches!(
        scenario::rename_attachment(&mut vault, "missing", "x"),
        Err(Error::Missing(_))
    ));
    assert!(matches!(
        scenario::restore_attachment(&mut vault, "missing"),
        Err(Error::Missing(_))
    ));
}

#[test]
fn list_attachments_filters_by_owner_and_hides_removed() {
    let (mut doc, goal_owner) = bare_doc();
    let tx_owner = EntityRef::new(EntityKind::Tx, "t1");
    link_in(&mut doc, &goal_owner, "a1");
    link_in(&mut doc, &tx_owner, "a2");
    link_in(&mut doc, &goal_owner, "a3");
    doc.attachments[2].removed_at = Some(NOW.into());

    let ids =
        |items: Vec<Attachment>| -> Vec<String> { items.into_iter().map(|item| item.id).collect() };
    assert_eq!(ids(list_attachments(&doc, None)), ["a1", "a2"]);
    assert_eq!(ids(list_attachments(&doc, Some(&goal_owner))), ["a1"]);
    assert_eq!(ids(list_attachments(&doc, Some(&tx_owner))), ["a2"]);
    assert!(list_attachments(&doc, Some(&EntityRef::new(EntityKind::Tx, "t9"))).is_empty());
}

#[test]
fn transactions_with_attachments_lists_each_operation_once_sorted() {
    let (mut doc, goal_owner) = bare_doc();
    link_in(&mut doc, &EntityRef::new(EntityKind::Tx, "t2"), "a1");
    link_in(&mut doc, &EntityRef::new(EntityKind::Tx, "t1"), "a2");
    link_in(&mut doc, &EntityRef::new(EntityKind::Tx, "t2"), "a3");
    link_in(&mut doc, &goal_owner, "a4");
    link_in(&mut doc, &EntityRef::new(EntityKind::Tx, "t3"), "a5");
    doc.attachments[4].removed_at = Some(NOW.into());

    assert_eq!(transactions_with_attachments(&doc), ["t1", "t2"]);
    assert!(transactions_with_attachments(&VaultDoc::default()).is_empty());
}

#[test]
fn find_attachment_finds_live_records_only() {
    let (mut doc, owner) = bare_doc();
    link_in(&mut doc, &owner, "a1");
    link_in(&mut doc, &owner, "a2");
    doc.attachments[1].removed_at = Some(NOW.into());

    assert_eq!(find_attachment(&doc, "a1").unwrap().id, "a1");
    assert!(matches!(
        find_attachment(&doc, "a2"),
        Err(Error::Missing(_))
    ));
    assert!(matches!(
        find_attachment(&doc, "nope"),
        Err(Error::Missing(_))
    ));
}

#[test]
fn owner_exists_knows_operations_and_goals_only() {
    let (mut doc, goal_owner) = bare_doc();
    let tx = income("s1", "100", "salary");
    let tx_id = tx.meta().id.clone();
    doc.transactions.push(tx);

    assert!(owner_exists(&doc, &goal_owner));
    assert!(owner_exists(
        &doc,
        &EntityRef::new(EntityKind::Tx, tx_id.clone())
    ));
    assert!(!owner_exists(&doc, &EntityRef::new(EntityKind::Tx, "g1")));
    assert!(!owner_exists(
        &doc,
        &EntityRef::new(EntityKind::Goal, tx_id)
    ));
    assert!(!owner_exists(
        &doc,
        &EntityRef::new(EntityKind::Goal, "nope")
    ));
    assert!(!owner_exists(
        &doc,
        &EntityRef::new(EntityKind::Store, "g1")
    ));
}

#[test]
fn attachments_dir_appends_files_to_the_vault_path() {
    use std::path::Path;
    assert_eq!(
        attachments_dir(Path::new("/data/vault.wallet")),
        Path::new("/data/vault.wallet.files")
    );
    assert_eq!(
        attachments_dir(Path::new("vault")),
        Path::new("vault.files")
    );
}
