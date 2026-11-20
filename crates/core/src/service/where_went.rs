//! Shim of the `where_went` area: moved to `crate::reports::where_went` (spec core-strangler, D1).

pub use crate::reports::where_went::{WHERE_ITEMS, WhereItem, WhereKind, WhereReport, WhereRow};

use super::UnlockedVault;

impl UnlockedVault {
    // forward:begin
    // forward:end
}
