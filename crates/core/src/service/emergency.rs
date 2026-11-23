//! Shim of the `emergency` area: the rules moved to `crate::planning::emergency`, the scenarios to
//! `crate::usecases::planning::emergency` (spec core-strangler, D1).

pub use crate::planning::emergency::{EmergencyStatus, EmergencyStore, MAX_FUND_MONTHS};

use crate::error::Result;
use crate::model::planning::EmergencyFund;

use super::UnlockedVault;
use super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// What the fund is now.
        pub fn emergency_fund(&self) -> EmergencyFund
            => crate::planning::emergency::emergency_fund @doc;
        /// Replace the fund. Only live savings accounts fit (decision 29); an
        /// empty list removes the fund.
        pub fn set_emergency_fund(&mut self, fund: EmergencyFund) -> Result<EmergencyFund>
            => crate::usecases::planning::emergency::set_emergency_fund;
        /// Add or remove one account, the switch in the store form.
        pub fn emergency_fund_toggle(&mut self, store_id: &str, on: bool) -> Result<EmergencyFund>
            => crate::usecases::planning::emergency::emergency_fund_toggle;
        /// The fund's value, coverage and hints.
        pub fn emergency_fund_status(&self, now: i64, zone: &str) -> EmergencyStatus
            => crate::planning::emergency::emergency_fund_status @books;
    }
    // forward:end

    // Read by the store, sync and crypto-track code until they move; not in the baseline
    // (crate-private), so outside the regenerated block.
    forward! {}
}
