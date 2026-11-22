//! Cleanup of attachments when the vault is opened (catalog 96): the pure
//! decisions. The scenarios that wipe the files are in
//! `usecases::household::attachments_cleanup`.
//!
//! - Records the user removed (`removed_at`) go, files included.
//! - A record whose owner is missing stays while the owner may come back
//!   ([`owner_history_key`]); it goes only when the owner is gone for good. A
//!   dedupe or a merge must not turn into a deletion that other devices
//!   receive as a tombstone.
//! - Files no record names are deleted from the disk. That is local work: it
//!   never touches the document, so nothing is stamped and no tombstone is made.

use std::collections::HashSet;

use super::attachments::owner_exists;
use mwt_model::VaultDoc;
use mwt_model::{EntityKind, EntityRef};

/// The ids of the attachments that go: removed by the user, or whose owner is
/// gone for good (`owner_may_return` says whether a missing owner may still be
/// back).
pub fn gone_attachment_ids(
    doc: &VaultDoc,
    owner_may_return: impl Fn(&EntityRef) -> bool,
) -> Vec<String> {
    doc.attachments
        .iter()
        .filter(|item| {
            item.removed_at.is_some()
                || (!owner_exists(doc, &item.owner) && !owner_may_return(&item.owner))
        })
        .map(|item| item.id.clone())
        .collect()
}

/// Drop the records (and keys) of the gone attachments from the document.
pub fn drop_attachments(doc: &mut VaultDoc, gone: &[String]) {
    doc.attachments.retain(|item| !gone.contains(&item.id));
    doc.attachment_keys.retain(|key| !gone.contains(&key.id));
}

/// The key under which the sync history keeps a deleted owner, when an owner of
/// this kind can come back at all: another device deleted it and this one holds
/// its last live value (the rule that restores a deleted record that money
/// uses reads the same history). An owner this device deleted itself, or one
/// nobody ever knew, does not come back.
pub fn owner_history_key(owner: &EntityRef) -> Option<String> {
    let collection = match owner.kind {
        EntityKind::Tx => "transactions",
        EntityKind::Goal => "goals",
        _ => return None,
    };
    Some(format!("{collection}:{}", owner.id))
}

/// Ids of the attachments the document names.
pub fn named_ids(doc: &VaultDoc) -> HashSet<&str> {
    doc.attachments
        .iter()
        .map(|item| item.id.as_str())
        .collect()
}

/// An attachment id as it is written by `add_attachment_file`: a canonical
/// (lowercase, hyphenated) UUID, so never a path.
pub fn is_file_name(name: &str) -> bool {
    uuid::Uuid::parse_str(name).is_ok_and(|id| id.hyphenated().to_string() == name)
}
