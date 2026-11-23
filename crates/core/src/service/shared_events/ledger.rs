//! Shim of the ledger side of an event: moved to `crate::household::shared_events::ledger`
//! and `crate::usecases::household::shared_events` (spec core-strangler, D1).

use super::super::UnlockedVault;
use super::super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {}
    // forward:end
}
