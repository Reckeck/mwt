//! Shim of the `month_balance` area: moved to `crate::money::month_balance` (spec core-strangler, D1).

pub use crate::money::month_balance::{
    MonthBalance, MonthBalanceDay, MonthBalanceMark, MonthBalanceRow,
};

use crate::error::Result;

use super::UnlockedVault;
use super::forward::forward;
use super::reports::ReportFilter;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// The balance of the filter's months (the last twelve by default). The
        /// carry starts at `settings.carry_since`, even before the period does.
        pub fn month_balance(&self, filter: &ReportFilter, now: i64, zone: &str) -> Result<MonthBalance>
            => crate::usecases::household::month_balance::month_balance;
        /// Carry the unspent over from `month` (`YYYY-MM`), or stop carrying (`None`).
        pub fn set_carry_over(&mut self, month: Option<&str>) -> Result<()>
            => crate::usecases::household::month_balance::set_carry_over;
    }
    // forward:end
}
