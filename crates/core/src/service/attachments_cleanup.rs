//! Shim of the `attachments_cleanup` area: moved to `crate::household::attachments_cleanup` (spec core-strangler, D1).
//!
//! No old method of this area is public, so the generated block above stays
//! empty; the two internal callers (open and external merge) keep these
//! delegates until their files move.

use super::UnlockedVault;
use super::forward::forward;

impl UnlockedVault {
    // forward:begin
    // forward:end

    forward! {
        /// Drop the attachments the user removed and those whose owner is gone
        /// for good, files included. Runs when the vault is opened: no "Undo" of
        /// the last session can be waiting any more. Returns whether anything went.
        pub(crate) fn prune_attachments(&mut self) -> bool
            => crate::usecases::household::attachments_cleanup::prune_attachments;
        /// Delete the files of the attachments directory that no record names.
        pub(crate) fn sweep_orphan_attachment_files(&self)
            => crate::usecases::household::attachments_cleanup::sweep_orphan_attachment_files;
    }
}
