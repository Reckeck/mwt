//! Tests of the attachments area, kept as they were in the service module.

use crate::error::Error;
use crate::household::attachments::NewFile;
use crate::model::attachment::MAX_ATTACHMENT_BYTES;
use crate::model::entity_ref::{EntityKind, EntityRef};
use crate::service::UnlockedVault;
use crate::service::test_support::*;

fn tx_owner(vault: &mut UnlockedVault) -> EntityRef {
    let card = store(vault, "Card", crate::model::store::StoreKind::Bank, None);
    let tx = vault
        .add_transaction(income(&card, "100", "salary"))
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
    let item = vault
        .add_attachment_file(owner.clone(), receipt(b"%PDF-1.7 receipt"))
        .unwrap();

    let on_disk = std::fs::read(vault.attachments_dir().join(&item.id)).unwrap();
    assert!(
        !on_disk.windows(7).any(|part| part == b"receipt"),
        "the file is not sealed"
    );
    let (_, bytes) = vault.read_attachment(&item.id).unwrap();
    assert_eq!(bytes, b"%PDF-1.7 receipt");
    assert_eq!(item.size, 16);
    assert_eq!(vault.list_attachments(Some(&owner)), vec![item.clone()]);
    assert_eq!(
        vault.transactions_with_attachments(),
        vec![owner.id.clone()]
    );

    // The same file again is the same attachment.
    let again = vault
        .add_attachment_file(owner, receipt(b"%PDF-1.7 receipt"))
        .unwrap();
    assert_eq!(again.id, item.id);
}

#[test]
fn a_new_password_keeps_the_files_readable() {
    let (_dir, mut vault) = setup();
    let owner = tx_owner(&mut vault);
    let item = vault.add_attachment_file(owner, receipt(b"bytes")).unwrap();
    vault.flush_pending().unwrap();
    let path = vault.path.clone();
    vault.change_password("pw", "another").unwrap();
    drop(vault);
    let reopened = UnlockedVault::open(path, "another").unwrap();
    assert_eq!(reopened.read_attachment(&item.id).unwrap().1, b"bytes");
}

#[test]
fn removal_waits_for_undo_until_the_next_open() {
    let (_dir, mut vault) = setup();
    let owner = tx_owner(&mut vault);
    let item = vault
        .add_attachment_file(owner.clone(), receipt(b"bytes"))
        .unwrap();
    let file = vault.attachments_dir().join(&item.id);

    vault.remove_attachment(&item.id).unwrap();
    assert!(vault.list_attachments(Some(&owner)).is_empty());
    assert!(file.exists(), "undo still needs the file");
    vault.restore_attachment(&item.id).unwrap();
    assert_eq!(vault.list_attachments(Some(&owner)).len(), 1);

    vault.remove_attachment(&item.id).unwrap();
    vault.flush_pending().unwrap();
    let path = vault.path.clone();
    drop(vault);
    let reopened = UnlockedVault::open(path, "pw").unwrap();
    assert!(reopened.doc.attachments.is_empty());
    assert!(reopened.doc.attachment_keys.is_empty());
    assert!(!file.exists());
}

#[test]
fn a_deleted_operation_takes_its_files_on_the_next_open() {
    let (_dir, mut vault) = setup();
    let owner = tx_owner(&mut vault);
    let item = vault
        .add_attachment_file(owner.clone(), receipt(b"bytes"))
        .unwrap();
    vault
        .add_attachment_link(owner.clone(), "bank.ru/check/1", "")
        .unwrap();
    let file = vault.attachments_dir().join(&item.id);
    vault.delete_transaction(&owner.id).unwrap();
    vault.flush_pending().unwrap();
    let path = vault.path.clone();
    drop(vault);
    let reopened = UnlockedVault::open(path, "pw").unwrap();
    assert!(reopened.doc.attachments.is_empty());
    assert!(!file.exists());
}

#[test]
fn links_are_web_addresses_with_a_caption() {
    let (_dir, mut vault) = setup();
    let owner = tx_owner(&mut vault);
    let link = vault
        .add_attachment_link(owner.clone(), " bank.ru/check/1 ", "")
        .unwrap();
    assert_eq!(link.url.as_deref(), Some("https://bank.ru/check/1"));
    assert_eq!(link.title, "https://bank.ru/check/1");
    let named = vault
        .add_attachment_link(owner.clone(), "https://x.io", "Договор")
        .unwrap();
    assert_eq!(named.title, "Договор");
    assert!(
        vault
            .add_attachment_link(owner.clone(), "file:///etc/passwd", "")
            .is_err()
    );
    assert!(
        vault
            .add_attachment_link(owner, "not an address", "")
            .is_err()
    );
    assert!(vault.read_attachment(&link.id).is_err());
}

#[test]
fn a_renamed_file_is_found_by_its_new_name() {
    let (_dir, mut vault) = setup();
    let owner = tx_owner(&mut vault);
    let item = vault
        .add_attachment_file(owner.clone(), receipt(b"bytes"))
        .unwrap();
    vault
        .rename_attachment(&item.id, "  Чек skycapital  ")
        .unwrap();
    assert!(vault.rename_attachment(&item.id, " ").is_err());
    let filter = crate::service::TxFilter {
        text: Some("skycapital".into()),
        ..Default::default()
    };
    let found = vault.list_transactions(&filter, "UTC").unwrap();
    assert_eq!(found.len(), 1);
    let only_attached = crate::service::TxFilter {
        with_attachments: true,
        ..Default::default()
    };
    assert_eq!(
        vault
            .list_transactions(&only_attached, "UTC")
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
        vault
            .add_attachment_link(missing, "https://x.io", "")
            .is_err()
    );
    let store = EntityRef::new(EntityKind::Store, vault.doc.stores[0].id.clone());
    assert!(
        vault
            .add_attachment_link(store, "https://x.io", "")
            .is_err()
    );
    let owner = tx_owner(&mut vault);
    let big = vec![0u8; MAX_ATTACHMENT_BYTES + 1];
    assert!(matches!(
        vault.add_attachment_file(owner, receipt(&big)),
        Err(Error::TooLarge(_))
    ));
}
