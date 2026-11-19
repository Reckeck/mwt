//! Shim of the `peer_sync/repoint` area: moved to `crate::sync::repair::repoint` (spec core-strangler, D1).

use super::UnlockedVault;
use crate::error::Result;
use crate::service::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// Move every reference to the operation `old` onto the operation `new`.
        /// Returns whether anything changed.
        #[cfg(test)]
        pub(crate) fn repoint_tx(&mut self, old: &str, new: &str) -> bool
            => crate::usecases::sync::repair::repoint_tx;
        /// `repoint_tx` for every pair, as one tracked edit of this
        /// device: the records it changed are stamped (so the other devices get
        /// them) and recorded in the batch for its undo.
        pub(crate) fn repoint_removed_copies(&mut self, pairs: &[(String, String)], batch_id: &str) -> Result<()>
            => crate::usecases::sync::repair::repoint_removed_copies;
    }
    // forward:end
}
