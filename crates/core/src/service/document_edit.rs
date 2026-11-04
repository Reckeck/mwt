//! Applying a whole edited document (the raw JSON editor, restoring a copy)
//! as the difference the user made, not as a replacement.
//!
//! `replace_doc` swaps the document, so everything the device holds that the
//! supplied document lacks (a record a peer sent while the editor was open)
//! looks deleted and is stamped as such. Here only the keys where the edited
//! document differs from the base the user saw are written.

use serde_json::{Value, json};

use super::UnlockedVault;
use crate::error::{Error, Result};
use crate::model::vault::VaultDoc;
use crate::sync::view;

impl UnlockedVault {
    /// Three-way apply of an editor save: a record `edited` changes, adds or
    /// removes against `base` is written to the current document; a record
    /// equal to its base is left as it is now (what a peer sent after the
    /// buffer was loaded stays). Device-only parts (device settings,
    /// `exchange.terminal`) follow the same rule. `id`, `created_at`,
    /// `version` and `sync` stay the vault's own, holdings are re-derived, and
    /// the usual change tracking then stamps only what was really edited.
    /// Nothing changes when the result does not fit the schema.
    pub fn apply_document_edit(&mut self, base: &VaultDoc, edited: VaultDoc) -> Result<()> {
        let base = to_value(base)?;
        let edited = to_value(&edited)?;
        self.merge_edit(&base, &edited)
    }

    /// Write the records of `copy` over the current ones by value and delete
    /// nothing: the restore of a backup in a group of devices.
    pub(super) fn apply_records_of(&mut self, copy: &VaultDoc) -> Result<()> {
        let copy = to_value(copy)?;
        self.merge_edit(&json!({}), &copy)
    }

    /// Whether a trusted device (not forgotten) shares this vault.
    pub fn has_live_peers(&self) -> bool {
        self.doc
            .sync
            .peers
            .iter()
            .any(|peer| peer.revoked.is_none())
    }

    fn merge_edit(&mut self, base: &Value, edited: &Value) -> Result<()> {
        self.track_changes()?;
        let mut value = self.doc_value()?;
        let base_records = view::records(base);
        let edited_records = view::records(edited);
        let current = view::records(&value);
        for (key, record) in &edited_records {
            if base_records.get(key) != Some(record) && current.get(key) != Some(record) {
                view::write(&mut value, key, Some(record));
            }
        }
        for key in base_records
            .keys()
            .filter(|key| !edited_records.contains_key(*key))
        {
            if current.contains_key(key) {
                view::write(&mut value, key, None);
            }
        }
        view::apply_device_edits(base, edited, &mut value);
        view::trim(&mut value);
        // Rebuilding the typed document is the schema check.
        let mut doc: VaultDoc = serde_json::from_value(value)
            .map_err(|err| Error::InvalidInput(format!("document: {err}")))?;
        doc.id = self.doc.id.clone();
        doc.created_at = self.doc.created_at.clone();
        doc.version = crate::VAULT_FORMAT_VERSION;
        doc.sync = std::mem::take(&mut self.doc.sync);
        self.doc = doc;
        self.recompute_holdings();
        self.persist()
    }
}

fn to_value(doc: &VaultDoc) -> Result<Value> {
    serde_json::to_value(doc).map_err(|err| Error::Serialize(err.to_string()))
}
