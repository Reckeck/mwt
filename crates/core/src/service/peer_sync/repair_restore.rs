//! Shim of the `peer_sync/repair_restore` area: moved to `crate::sync::repair::repair_restore` (spec core-strangler, D1).

use super::UnlockedVault;
use crate::service::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {}
    // forward:end
}
