//! Shim of the `utilities` area: moved to `crate::household::utilities`
//! (spec core-strangler, D1).

pub use crate::household::utilities::{
    FormulaZoneReading, PayUtilityBillInput, PayUtilityLine, PropertySummary, UtilityBill,
    UtilityBillLine, UtilityDraft, UtilityFormulaInput, UtilityFormulaPreview, UtilityMonth,
    UtilityPaymentDraft, UtilityTrend, ZoneBillLine, ZoneMonth, ZoneReadingInput,
    preview_utility_formula,
};
