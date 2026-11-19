//! Shim of the `peer_sync/bad_record` area: moved to `crate::sync::repair::bad_record` (spec core-strangler, D1).

use super::UnlockedVault;
use crate::service::forward::forward;
use crate::sync::SyncSummary;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// Journal records left out of a batch, unless the same line is already
        /// there. True when a line was added (the caller saves).
        pub(crate) fn journal_skipped(&mut self, peer_name: &str, summary: &SyncSummary) -> bool
            => crate::usecases::sync::repair::journal_skipped;
    }
    // forward:end
}
