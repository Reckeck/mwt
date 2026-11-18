//! Utilities (plan §7.5): metered bills (consumption × tariff), fixed ones
//! repeated from last time, and rent with a deposit in the first payment.
//! A bill becomes an ordinary "Pay" the user confirms in the form.
//!
//! Pure part of the area: the checks behind each write and the reads over a
//! document or a read context. The writes are the scenarios in
//! `usecases::household::utilities`. What was paid for an item is read through
//! `household::property_links`, which counts the same bills in the property's
//! year, so this module depends on it and not the other way round.

mod bill;
mod formula;
mod items;
mod month;
mod pay;
mod payments;
mod readings;
mod summary;
mod trend;
mod types;

pub use bill::{utility_bill, utility_payment_draft};
pub use formula::{
    FormulaZoneReading, UtilityFormulaInput, UtilityFormulaPreview, preview_utility_formula,
};
pub use items::{
    plan_create_utility, plan_delete_utility, plan_update_utility, utility_item, utility_items,
};
pub use month::{MonthReadings, monthly_readings, out_of_order_ids, readings_of};
pub use pay::{BillPlan, plan_pay_utility_bill};
pub use readings::{
    ReadingInput, ReadingMove, ReadingNamer, plan_add_reading, plan_update_reading, require_reading,
};
pub use summary::property_summary;
pub use trend::meter_trend;
pub use types::{
    PayUtilityBillInput, PayUtilityLine, PropertySummary, TREND_MONTHS, UtilityBill,
    UtilityBillLine, UtilityDraft, UtilityMonth, UtilityPaymentDraft, UtilityTrend, ZoneBillLine,
    ZoneMonth, ZoneReadingInput,
};
