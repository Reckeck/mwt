//! Shim of the `attachments` area: moved to `crate::household::attachments` (spec core-strangler, D1).

pub use crate::household::attachments::NewFile;

use std::path::PathBuf;

use crate::error::Result;
use crate::model::attachment::Attachment;
use crate::model::entity_ref::EntityRef;

use super::UnlockedVault;
use super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// The files and links of one entity (or of all), oldest first.
        pub fn list_attachments(&self, owner: Option<&EntityRef>) -> Vec<Attachment>
            => crate::household::attachments::list_attachments @doc;
        /// Ids of the operations that have at least one file or link: the paperclip
        /// in the list and the "With attachments" filter.
        pub fn transactions_with_attachments(&self) -> Vec<String>
            => crate::household::attachments::transactions_with_attachments @doc;
        /// Where the sealed files live: `vault.wallet` → `vault.wallet.files/`.
        pub fn attachments_dir(&self) -> PathBuf
            => crate::usecases::household::attachments::attachments_dir;
        pub fn add_attachment_link(&mut self, owner: EntityRef, url: &str, title: &str) -> Result<Attachment>
            => crate::usecases::household::attachments::add_attachment_link;
        /// Seal the file next to the vault and keep its record. The same file
        /// added to the same entity again gives back the one already there.
        pub fn add_attachment_file(&mut self, owner: EntityRef, file: NewFile<'_>) -> Result<Attachment>
            => crate::usecases::household::attachments::add_attachment_file;
        /// The file's bytes, opened; a link has none.
        pub fn read_attachment(&self, id: &str) -> Result<(Attachment, Vec<u8>)>
            => crate::usecases::household::attachments::read_attachment;
        /// Hide the attachment; [`restore_attachment`] brings it back until the vault
        /// is opened next, when the cleanup deletes the file.
        pub fn remove_attachment(&mut self, id: &str) -> Result<Attachment>
            => crate::usecases::household::attachments::remove_attachment;
        pub fn rename_attachment(&mut self, id: &str, title: &str) -> Result<Attachment>
            => crate::usecases::household::attachments::rename_attachment;
        pub fn restore_attachment(&mut self, id: &str) -> Result<Attachment>
            => crate::usecases::household::attachments::restore_attachment;
    }
    // forward:end
}
