//! Cleanup of attachments when the vault is opened (catalog 96).
//!
//! - Records the user removed (`removed_at`) go, files included.
//! - A record whose owner is missing stays while the owner may come back
//!   ([`UnlockedVault::owner_may_return`]); it goes only when the owner is
//!   gone for good. A dedupe or a merge must not turn into a deletion that
//!   other devices receive as a tombstone.
//! - Files no record names are deleted from the disk. That is local work: it
//!   never touches the document, so nothing is stamped and no tombstone is made.

use std::collections::HashSet;
use std::path::Path;

use super::UnlockedVault;
use crate::model::entity_ref::{EntityKind, EntityRef};

impl UnlockedVault {
    /// Drop the attachments the user removed and those whose owner is gone
    /// for good, files included. Runs when the vault is opened: no "Undo" of
    /// the last session can be waiting any more. Returns whether anything went.
    pub(super) fn prune_attachments(&mut self) -> bool {
        let gone: Vec<String> = self
            .doc
            .attachments
            .iter()
            .filter(|item| {
                item.removed_at.is_some()
                    || (!self.owner_exists(&item.owner) && !self.owner_may_return(&item.owner))
            })
            .map(|item| item.id.clone())
            .collect();
        if gone.is_empty() {
            return false;
        }
        if !self.sandbox {
            let dir = self.attachments_dir();
            for id in &gone {
                // A file synced from another device may never have been here.
                if is_file_name(id) {
                    remove_plain_file(&dir.join(id));
                }
            }
        }
        self.doc.attachments.retain(|item| !gone.contains(&item.id));
        self.doc
            .attachment_keys
            .retain(|key| !gone.contains(&key.id));
        true
    }

    /// Whether a missing owner may still be back: another device deleted it
    /// and this one holds its last live value (the rule that restores a
    /// deleted record that money uses reads the same history). An owner this
    /// device deleted itself, or one nobody ever knew, does not come back.
    fn owner_may_return(&self, owner: &EntityRef) -> bool {
        let collection = match owner.kind {
            EntityKind::Tx => "transactions",
            EntityKind::Goal => "goals",
            _ => return false,
        };
        let key = format!("{collection}:{}", owner.id);
        self.deleted_value::<serde_json::Value>(&key).is_some()
    }

    /// Delete the files of the attachments directory that no record names.
    /// Only plain files named exactly like an attachment id (a canonical
    /// UUID) directly inside `<vault>.files/` are touched; a symlink, a
    /// directory, another name or a path outside never is.
    pub(super) fn sweep_orphan_attachment_files(&self) {
        if self.sandbox {
            return;
        }
        let dir = self.attachments_dir();
        // A symlinked directory could lead anywhere.
        match std::fs::symlink_metadata(&dir) {
            Ok(meta) if meta.file_type().is_dir() => {}
            _ => return,
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return;
        };
        let named: HashSet<&str> = self
            .doc
            .attachments
            .iter()
            .map(|item| item.id.as_str())
            .collect();
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
}

/// An attachment id as it is written by `add_attachment_file`: a canonical
/// (lowercase, hyphenated) UUID, so never a path.
fn is_file_name(name: &str) -> bool {
    uuid::Uuid::parse_str(name).is_ok_and(|id| id.hyphenated().to_string() == name)
}

/// Remove a regular file; a symlink (even to a file) is left alone.
fn remove_plain_file(path: &Path) {
    if std::fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_file()) {
        let _ = std::fs::remove_file(path);
    }
}
