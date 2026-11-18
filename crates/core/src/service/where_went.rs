//! Shim of the `where_went` area: moved to `crate::reports::where_went` (spec core-strangler, D1).

pub use crate::reports::where_went::{WHERE_ITEMS, WhereItem, WhereKind, WhereReport, WhereRow};

use super::UnlockedVault;
use super::forward::forward;
use super::reports::Ctx;

impl UnlockedVault {
    // forward:begin
    // forward:end

    // The tool forwards public methods only; the crate-private delegate is kept by hand.
    forward! {
        pub(crate) fn where_report(&self, ctx: &Ctx) -> WhereReport
            => crate::reports::where_went::where_report @books;
    }
}
