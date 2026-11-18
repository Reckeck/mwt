//! The shapes of the utilities area: what the user edits, what the views
//! return and what the bill dialog sends back.

use crate::Amount;
use crate::model::planning::{MeterReading, UtilityItem, UtilityKind};
use crate::model::transaction::Transaction;

/// Months the average and the forecast look back over.
pub const TREND_MONTHS: usize = 6;

/// What a utility item is: everything the user edits.
#[derive(Debug, Clone, PartialEq)]
pub struct UtilityDraft {
    pub name: String,
    pub active: bool,
    pub kind: UtilityKind,
    pub currency: String,
    /// A utility always belongs to a property.
    pub property_id: String,
    pub store_id: Option<String>,
    pub category: String,
}

/// One month of an item's bills.
#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct UtilityMonth {
    /// `YYYY-MM`.
    pub period: String,
    /// Metered: the meter reading of the month.
    pub reading: Option<Amount>,
    /// Metered: reading minus the previous month's.
    pub consumption: Option<Amount>,
    /// The bill in the item's currency: consumption × tariff, or what was paid.
    pub amount: Amount,
    /// Metered: the reading's id, to correct it.
    pub reading_id: Option<String>,
    /// The payments booked for this month, to open and edit them.
    pub payments: Vec<Transaction>,
    /// Metered: every reading of the month, oldest first, with its day.
    pub readings: Vec<MeterReading>,
    /// A multi-rate meter: each zone's reading and consumption.
    #[serde(default)]
    pub zones: Vec<ZoneMonth>,
    /// Ids of this month's readings lower than the previous reading of the
    /// same zone by date (after a sync). Computed, never stored.
    #[serde(default)]
    pub out_of_order: Vec<String>,
}

/// One tariff zone of a month ("day": 800, used 200).
#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct ZoneMonth {
    pub zone: String,
    pub reading: Amount,
    pub consumption: Option<Amount>,
}

#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct UtilityTrend {
    pub item_id: String,
    pub currency: String,
    /// Oldest first.
    pub months: Vec<UtilityMonth>,
    /// Average bill of the last six months with a bill.
    pub average: Option<Amount>,
    /// Last bill against the one before, percent.
    pub delta_pct: Option<Amount>,
    /// Next month by a straight line through the last six.
    pub forecast: Option<Amount>,
    /// The last bill is above 1.5 × the average of the ones before it.
    pub anomaly: bool,
    /// What the item costs a month as far as is known: the last bill,
    /// the fixed amount, or the rent.
    pub monthly: Amount,
}

/// A payment the form opens with, for one month of one item.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct UtilityPaymentDraft {
    pub item_id: String,
    pub period: String,
    /// A `payment` ready to be edited and saved; the store is empty when
    /// the item names none.
    pub transaction: Transaction,
    /// Metered: what was consumed this period.
    pub consumption: Option<Amount>,
    /// Housing: the deposit is part of this (first) payment.
    pub includes_deposit: bool,
}

/// One property at a glance: what it is worth, what is owed on it and what
/// it costs a month, all in the base currency.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct PropertySummary {
    pub store_id: String,
    pub base_currency: String,
    pub value_base: Amount,
    /// Balance of the loans secured by it, as a positive number.
    pub debt_base: Amount,
    pub equity_base: Amount,
    /// Utilities a month, as far as is known (last bill, fixed amount, rent).
    pub utilities_month_base: Amount,
    /// Mandatory loan payments a month, from the loans' terms.
    pub loan_month_base: Amount,
    pub monthly_cost_base: Amount,
    pub utility_count: u32,
    /// Rent received over the last 12 months (operations linked to it).
    #[serde(default)]
    pub rent_12m_base: Amount,
    /// Linked operations, utility bills and loan payments over 12 months.
    #[serde(default)]
    pub expenses_12m_base: Amount,
    /// Rent over 12 months to the current value, %; `None` without a value.
    #[serde(default)]
    pub rent_yield_pct: Option<Amount>,
    /// A car: expenses and the loss of value over the year, a month.
    #[serde(default)]
    pub ownership_month_base: Option<Amount>,
    /// The value lost over the year (a car's wear); zero when it grew.
    #[serde(default)]
    pub wear_12m_base: Amount,
}

/// One line of a month's utility bill.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct UtilityBillLine {
    pub item: UtilityItem,
    /// Metered: last month's reading, the base of this month's consumption.
    pub previous_reading: Option<Amount>,
    /// Metered: this month's reading, when already entered.
    pub reading: Option<Amount>,
    pub consumption: Option<Amount>,
    /// The suggested amount; `None` for a meter still waiting for its reading.
    pub amount: Option<Amount>,
    /// Already paid for this month, in the item's currency.
    pub paid: Amount,
    pub includes_deposit: bool,
    /// A multi-rate meter: the previous and this month's reading per zone.
    #[serde(default)]
    pub zones: Vec<ZoneBillLine>,
    /// A meter still waiting for its reading: what it will likely cost —
    /// the trend's forecast, else its average («≈ 1 270 ₽»).
    #[serde(default)]
    pub estimate: Option<Amount>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct ZoneBillLine {
    pub zone: String,
    pub previous_reading: Option<Amount>,
    pub reading: Option<Amount>,
}

/// A zone's reading typed into the bill.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct ZoneReadingInput {
    pub zone: String,
    pub reading: Amount,
}

/// Every active utility of a property for one month, to be paid at once.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct UtilityBill {
    pub property_id: String,
    pub period: String,
    /// The property's usual payment day.
    pub pay_day: Option<u8>,
    /// The date the payments get: the pay day of that month, or today when
    /// the month is the current one and the day has passed. RFC3339.
    pub due_at: String,
    pub lines: Vec<UtilityBillLine>,
    /// What the month will cost: the amounts, the estimates of the meters
    /// still unread; rent an automation pays is left out (Wallets-2 · 10).
    #[serde(default)]
    pub total: Amount,
    /// Part of `total` is an estimate: some meter waits for its reading.
    #[serde(default)]
    pub estimated: bool,
    /// Meters waiting for this month's reading.
    #[serde(default)]
    pub unread: u32,
    /// The bill of the last six months before this one, on average, counted
    /// the same way as `total`.
    #[serde(default)]
    pub average: Option<Amount>,
    /// `total` against `average`, percent.
    #[serde(default)]
    pub delta_pct: Option<Amount>,
}

/// One line the user confirmed in the bill dialog.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct PayUtilityLine {
    pub item_id: String,
    pub amount: Amount,
    /// Metered: the reading to record for the month.
    pub reading: Option<Amount>,
    /// Metered: the day of the month the meter was read; `None` = its end.
    #[serde(default)]
    pub reading_day: Option<u8>,
    /// A multi-rate meter: the readings per zone (instead of `reading`).
    #[serde(default)]
    pub zone_readings: Vec<ZoneReadingInput>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct PayUtilityBillInput {
    pub property_id: String,
    /// `YYYY-MM`.
    pub period: String,
    /// The store the money leaves; an item's own store wins when it has one.
    pub store_id: String,
    /// RFC3339; empty means the bill's `due_at`.
    #[serde(default)]
    pub timestamp: Option<String>,
    pub lines: Vec<PayUtilityLine>,
}
