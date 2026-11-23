//! Shim of the `utilities` area: moved to `crate::household::utilities`
//! (spec core-strangler, D1).

pub use crate::household::utilities::{
    FormulaZoneReading, PayUtilityLine, UtilityBillLine, UtilityMonth, ZoneBillLine, ZoneMonth,
    ZoneReadingInput,
};
