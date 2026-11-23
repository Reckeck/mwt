//! Shim of the `peer_sync/repoint` area: moved to `crate::sync::repair::repoint` (spec core-strangler, D1).

use super::UnlockedVault;
use crate::service::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {}
    // forward:end
}
