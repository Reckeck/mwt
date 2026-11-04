//! Catalog 96: what the cleanup on open removes and what it keeps.

use super::tests::{exchange, two_devices};
use super::*;
use crate::model::entity_ref::{EntityKind, EntityRef};
use crate::model::store::StoreKind;
use crate::service::NewFile;
use crate::service::test_support::*;

fn reopen(vault: &UnlockedVault) -> UnlockedVault {
    UnlockedVault::open(vault.path.clone(), "pw").unwrap()
}

fn tx_of(vault: &mut UnlockedVault) -> String {
    let card = store(vault, "Card", StoreKind::Bank, None);
    vault
        .add_transaction(income(&card, "100", "salary"))
        .unwrap()
        .meta()
        .id
        .clone()
}

fn attach(vault: &mut UnlockedVault, tx_id: &str) -> String {
    vault
        .add_attachment_file(
            EntityRef::new(EntityKind::Tx, tx_id),
            NewFile {
                name: "a.pdf",
                mime: None,
                bytes: b"bytes",
            },
        )
        .unwrap()
        .id
}

#[test]
fn a_removed_attachment_is_pruned_with_its_file() {
    let (_dir, mut vault) = setup();
    let tx = tx_of(&mut vault);
    let id = attach(&mut vault, &tx);
    vault.remove_attachment(&id).unwrap();
    let file = vault.attachments_dir().join(&id);
    assert!(file.exists(), "kept until the next open");
    let vault = reopen(&vault);
    assert!(vault.doc.attachments.is_empty());
    assert!(vault.doc.attachment_keys.is_empty());
    assert!(!file.exists());
}

#[test]
fn an_attachment_of_a_gone_owner_is_pruned() {
    let (_dir, mut vault) = setup();
    let tx = tx_of(&mut vault);
    let id = attach(&mut vault, &tx);
    vault.delete_transaction(&tx).unwrap();
    let file = vault.attachments_dir().join(&id);
    let vault = reopen(&vault);
    assert!(
        vault.doc.attachments.is_empty(),
        "this device deleted it itself"
    );
    assert!(!file.exists());
}

#[test]
fn an_attachment_of_an_owner_that_may_return_is_kept() {
    let (_dir, mut left, mut right) = two_devices();
    let tx = tx_of(&mut left);
    exchange(&mut left, &mut right);
    let id = attach(&mut right, &tx);
    left.delete_transaction(&tx).unwrap();
    exchange(&mut left, &mut right);
    assert!(right.doc.transactions.is_empty(), "the deletion arrived");

    let file = right.attachments_dir().join(&id);
    let right = reopen(&right);
    assert!(right.doc.attachments.iter().any(|item| item.id == id));
    assert!(file.exists());
}

#[test]
fn an_orphan_file_is_deleted_and_other_files_are_not() {
    let (_dir, mut vault) = setup();
    let tx = tx_of(&mut vault);
    let kept = attach(&mut vault, &tx);
    let dir = vault.attachments_dir();
    let orphan = dir.join(uuid::Uuid::new_v4().to_string());
    let upper = dir.join(uuid::Uuid::new_v4().to_string().to_uppercase());
    let stranger = dir.join("notes.txt");
    let part = dir.join(format!("{}.part", uuid::Uuid::new_v4()));
    let nested = dir.join(uuid::Uuid::new_v4().to_string());
    for path in [&orphan, &upper, &stranger, &part] {
        std::fs::write(path, b"x").unwrap();
    }
    std::fs::create_dir(&nested).unwrap();
    // Next to the directory, not in it.
    let outside = dir.with_file_name(uuid::Uuid::new_v4().to_string());
    std::fs::write(&outside, b"x").unwrap();
    #[cfg(unix)]
    let link = {
        let target = _dir.path().join("precious");
        std::fs::write(&target, b"precious").unwrap();
        let link = dir.join(uuid::Uuid::new_v4().to_string());
        std::os::unix::fs::symlink(&target, &link).unwrap();
        (link, target)
    };

    let vault = reopen(&vault);
    assert!(!orphan.exists(), "a file no record names goes");
    assert!(dir.join(&kept).exists());
    assert!(vault.doc.attachments.iter().any(|item| item.id == kept));
    for path in [&upper, &stranger, &part, &nested, &outside] {
        assert!(path.exists(), "{path:?} is not an attachment file");
    }
    #[cfg(unix)]
    {
        assert!(
            link.0.exists() || link.0.symlink_metadata().is_ok(),
            "the link stays"
        );
        assert!(link.1.exists(), "what a link points to stays");
    }
}

#[test]
fn a_second_open_changes_nothing() {
    let (_dir, mut vault) = setup();
    let tx = tx_of(&mut vault);
    let removed = attach(&mut vault, &tx);
    vault.remove_attachment(&removed).unwrap();
    std::fs::write(
        vault
            .attachments_dir()
            .join(uuid::Uuid::new_v4().to_string()),
        b"x",
    )
    .unwrap();
    let first = reopen(&vault);
    let second = reopen(&first);
    assert_eq!(first.doc.sync.stamps, second.doc.sync.stamps);
    assert_eq!(first.doc.sync.tombstones, second.doc.sync.tombstones);
    assert_eq!(first.doc.attachments, second.doc.attachments);
    let stable = reopen(&second);
    assert_eq!(second.doc.sync.stamps, stable.doc.sync.stamps);
    assert_eq!(second.doc.sync.tombstones, stable.doc.sync.tombstones);
}
