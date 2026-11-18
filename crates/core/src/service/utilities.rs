//! Shim of the `utilities` area: moved to `crate::household::utilities`
//! (spec core-strangler, D1).

pub use crate::household::utilities::{
    FormulaZoneReading, PayUtilityBillInput, PayUtilityLine, PropertySummary, UtilityBill,
    UtilityBillLine, UtilityDraft, UtilityFormulaInput, UtilityFormulaPreview, UtilityMonth,
    UtilityPaymentDraft, UtilityTrend, ZoneBillLine, ZoneMonth, ZoneReadingInput,
    preview_utility_formula,
};

use crate::Amount;
use crate::error::Result;
use crate::model::planning::{MeterReading, UtilityItem};
use crate::model::transaction::Transaction;

use super::UnlockedVault;
use super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// The property summary: value, debt, equity and monthly costs.
        pub fn property_summary(&self, store_id: &str, zone: &str) -> Result<PropertySummary>
            => crate::household::utilities::property_summary @books;
        pub fn utility_items(&self) -> Vec<UtilityItem>
            => crate::household::utilities::utility_items @doc;
        pub fn create_utility_item(&mut self, draft: UtilityDraft) -> Result<UtilityItem>
            => crate::usecases::household::utilities::create_utility_item;
        pub fn update_utility_item(&mut self, id: &str, draft: UtilityDraft) -> Result<UtilityItem>
            => crate::usecases::household::utilities::update_utility_item;
        /// Remove an item with its meter readings; payments already made stay.
        pub fn delete_utility_item(&mut self, id: &str) -> Result<()>
            => crate::usecases::household::utilities::delete_utility_item;
        /// Record a meter reading for a month (`YYYY-MM`) and, optionally, a day of
        /// it; one already there for the same month and day is replaced. A month
        /// may hold several readings (a move-out on the 15th). A meter only counts
        /// up: a reading below an earlier one or above a later one is refused.
        pub fn add_meter_reading(&mut self, item_id: &str, value: Amount, period: &str, day: Option<u8>, zone: Option<&str>) -> Result<MeterReading>
            => crate::usecases::household::utilities::add_meter_reading;
        /// Correct a reading: its value and, if moved, its month and day. The
        /// same rules as a new one apply against every other reading of the meter.
        pub fn update_meter_reading(&mut self, id: &str, value: Amount, period: &str, day: Option<u8>) -> Result<MeterReading>
            => crate::usecases::household::utilities::update_meter_reading;
        pub fn delete_meter_reading(&mut self, id: &str) -> Result<()>
            => crate::usecases::household::utilities::delete_meter_reading;
        /// Bills by month with the average, change, forecast and anomaly (plan §7.5).
        pub fn meter_trend(&self, item_id: &str, zone: &str) -> Result<UtilityTrend>
            => crate::household::utilities::meter_trend @books;
        /// The "Pay" for one month of an item (plan §7.5): metered —
        /// consumption × tariff; fixed — what was paid last time, else the set
        /// amount; rent — the monthly amount plus the deposit with the first one.
        pub fn utility_payment_draft(&self, item_id: &str, period: &str, zone: &str) -> Result<UtilityPaymentDraft>
            => crate::household::utilities::utility_payment_draft @books;
        /// The month's bill of a property: every active item with its suggested
        /// amount, meters with their previous reading (plan: "everything is paid on one day").
        pub fn utility_bill(&self, property_id: &str, period: &str, zone: &str) -> Result<UtilityBill>
            => crate::household::utilities::utility_bill @books;
        /// Pay a month's bill in one go: record the meter readings, then one
        /// payment per line, linked to its item. All or nothing — a line that
        /// fails leaves the vault as it was.
        pub fn pay_utility_bill(&mut self, input: PayUtilityBillInput, zone: &str) -> Result<Vec<Transaction>>
            => crate::usecases::household::utilities::pay_utility_bill;
    }
    // forward:end
}
