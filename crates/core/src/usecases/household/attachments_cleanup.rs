//! Scenarios of the attachment cleanup when the vault is opened (catalog 96):
//! the decisions are in `household::attachments_cleanup`, the files are wiped
//! here. Local work: it writes no record, so nothing is stamped and no
//! tombstone is made (the caller persists what it changed).

use std::path::Path;

use crate::household::attachments_cleanup::{
    drop_attachments, gone_attachment_ids, is_file_name, named_ids, owner_history_key,
};
use crate::model::entity_ref::EntityRef;
use crate::service::UnlockedVault;
use crate::sync::repair::repair_restore::deleted_value;
use crate::usecases::household::attachments::attachments_dir;

/// Drop the attachments the user removed and those whose owner is gone
/// for good, files included. Runs when the vault is opened: no "Undo" of
/// the last session can be waiting any more. Returns whether anything went.
pub fn prune_attachments(vault: &mut UnlockedVault) -> bool {
    let gone = gone_attachment_ids(&vault.doc, |owner| owner_may_return(vault, owner));
    if gone.is_empty() {
        return false;
    }
    if !vault.sandbox {
        let dir = attachments_dir(vault);
        for id in &gone {
            // A file synced from another device may never have been here.
            if is_file_name(id) {
                remove_plain_file(&dir.join(id));
            }
        }
    }
    drop_attachments(&mut vault.doc, &gone);
    true
}

/// Whether a missing owner may still be back: another device deleted it
/// and this one holds its last live value.
pub fn owner_may_return(vault: &UnlockedVault, owner: &EntityRef) -> bool {
    owner_history_key(owner)
        .is_some_and(|key| deleted_value::<serde_json::Value>(&vault.doc, &key).is_some())
}

/// Delete the files of the attachments directory that no record names.
/// Only plain files named exactly like an attachment id (a canonical
/// UUID) directly inside `<vault>.files/` are touched; a symlink, a
/// directory, another name or a path outside never is.
pub fn sweep_orphan_attachment_files(vault: &UnlockedVault) {
    if vault.sandbox {
        return;
    }
    let dir = attachments_dir(vault);
    // A symlinked directory could lead anywhere.
    match std::fs::symlink_metadata(&dir) {
        Ok(meta) if meta.file_type().is_dir() => {}
        _ => return,
    }
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    let named = named_ids(&vault.doc);
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if kind.is_file() && is_file_name(name) && !named.contains(name) {
            remove_plain_file(&entry.path());
        }
    }
}

/// Remove a regular file; a symlink (even to a file) is left alone.
fn remove_plain_file(path: &Path) {
    if std::fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_file()) {
        let _ = std::fs::remove_file(path);
    }
}
