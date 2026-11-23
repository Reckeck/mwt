//! Shim of the `peer_sync/bad_record` area: moved to `crate::sync::repair::bad_record` (spec core-strangler, D1).

use super::UnlockedVault;
use crate::service::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {}
    // forward:end
}
