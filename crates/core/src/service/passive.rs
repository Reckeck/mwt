//! Shim of the `passive` area: the rules moved to `crate::planning::passive`, the default
//! categories to `crate::usecases::planning::passive` (spec core-strangler, D1).

pub use crate::planning::passive::{PassiveIncome, PassiveMonth, PassiveShare};

use crate::Amount;
use crate::error::Result;

use super::UnlockedVault;
use super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// Whether a category key books passive income: the user's flag on the
        /// category, or the default for keys with no category (`earn`).
        pub fn is_passive_category(&self, key: &str) -> bool
            => crate::planning::passive::is_passive_category @doc;
        /// Passive income over the last finished months and the freedom index.
        /// `expected_pct` is the yearly return the potential assumes; `None`
        /// takes the key rate of today.
        pub fn passive_income(&self, now: i64, zone: &str, expected_pct: Option<Amount>) -> Result<PassiveIncome>
            => crate::planning::passive::passive_income @books;
    }
    // forward:end

    // Called when a vault opens and when a peer's changes are merged.
    forward! {}
}
