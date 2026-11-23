//! Shim of the `month_balance` area: moved to `crate::money::month_balance` (spec core-strangler, D1).

pub use crate::money::month_balance::{
    MonthBalance, MonthBalanceDay, MonthBalanceMark, MonthBalanceRow,
};
