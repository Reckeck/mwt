//! Utilities (plan §7.5): metered bills (consumption × tariff), fixed ones
//! repeated from last time, and rent with a deposit in the first payment.
//! A bill becomes an ordinary «Оплатить» the user confirms in the form.

use std::collections::BTreeMap;

use crate::Amount;
use crate::calendar::{self, Calendar, first_of_month};
use crate::error::{Error, Result};
use crate::model::planning::{MeterReading, UtilityItem, UtilityKind};
use crate::model::store::AssetRef;
use crate::model::transaction::{KindDetails, PaymentTx, PriceStamp, Transaction, TxMeta};
use crate::model::vault::now_iso;
use crate::pricing::history::secs_or_now;
use crate::utilities_formula::{self, FormulaInput, FormulaIssue, ZoneValues};

use super::UnlockedVault;

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
}

/// One tariff zone of a month («день»: 800, used 200).
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

/// The meter at the start and at the end of one month.
#[derive(Debug, Clone, PartialEq)]
struct MonthReadings {
    /// `None` for the first and only reading of a meter: nothing was used yet.
    start: Option<Amount>,
    end: Amount,
    end_id: String,
    /// When the start and the end were read (`sort_key`), for «дней».
    start_at: Option<(String, u8)>,
    end_at: (String, u8),
    /// A multi-rate meter: the same per zone; the totals above sum them.
    zones: Vec<(String, MonthReadings)>,
}

impl MonthReadings {
    fn consumption(&self) -> Option<Amount> {
        self.start.map(|start| self.end - start)
    }

    /// Days between the start and the end reading; a month when unknown.
    fn days(&self) -> Amount {
        let date = |(period, day): &(String, u8)| {
            let month = calendar::parse_month(period)?;
            let day = u32::from(*day).clamp(1, calendar::days_in_month(month));
            Some(first_of_month(month) + chrono::Duration::days(i64::from(day) - 1))
        };
        let days = self
            .start_at
            .as_ref()
            .and_then(date)
            .zip(date(&self.end_at))
            .map_or(30, |(start, end)| (end - start).num_days().max(0));
        Amount::new(rust_decimal::Decimal::from(days))
    }

    fn formula_input(&self, tariff: Amount) -> FormulaInput {
        let values = |month: &MonthReadings| ZoneValues {
            current: month.end,
            previous: month.start.unwrap_or(month.end),
        };
        FormulaInput {
            total: values(self),
            zones: self
                .zones
                .iter()
                .map(|(zone, month)| (zone.clone(), values(month)))
                .collect(),
            tariff,
            days: self.days(),
        }
    }
}

/// The bill of a metered item for a month: consumption × tariff, or its
/// own formula. `None` while there is no consumption yet.
fn metered_amount(
    item: &UtilityItem,
    month: &MonthReadings,
) -> std::result::Result<Option<Amount>, FormulaIssue> {
    let UtilityKind::Metered {
        tariff, formula, ..
    } = &item.kind
    else {
        return Ok(None);
    };
    let Some(used) = month.consumption() else {
        return Ok(None);
    };
    match formula
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
    {
        None => Ok(Some((used * *tariff).round(2))),
        Some(text) => utilities_formula::evaluate(text, &month.formula_input(*tariff)).map(Some),
    }
}

/// One zone's readings for a formula preview.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct FormulaZoneReading {
    /// `None` for a one-rate meter.
    #[serde(default)]
    pub zone: Option<String>,
    pub previous: Amount,
    pub current: Amount,
}

/// What a meter's bill would be: the form's preview and the bill dialog's
/// re-pricing as a reading is typed. Needs no vault.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct UtilityFormulaInput {
    pub tariff: Amount,
    /// Empty = consumption × tariff.
    #[serde(default)]
    pub formula: Option<String>,
    #[serde(default)]
    pub zones: Vec<String>,
    pub readings: Vec<FormulaZoneReading>,
    /// Days between the readings; a month when not given.
    #[serde(default)]
    pub days: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct UtilityFormulaPreview {
    pub amount: Option<Amount>,
    pub consumption: Option<Amount>,
    pub issue: Option<FormulaIssue>,
}

/// The bill by a meter's formula (or tariff) for the readings given.
pub fn preview_utility_formula(input: &UtilityFormulaInput) -> UtilityFormulaPreview {
    let zoned = |zone: Option<&str>| {
        input
            .readings
            .iter()
            .find(|reading| {
                reading.zone.as_deref().map(utilities_formula::zone_key)
                    == zone.map(utilities_formula::zone_key)
            })
            .map_or(ZoneValues::default(), |reading| ZoneValues {
                current: reading.current,
                previous: reading.previous,
            })
    };
    let zones: Vec<(String, ZoneValues)> = input
        .zones
        .iter()
        .map(|zone| (zone.clone(), zoned(Some(zone))))
        .collect();
    let total = if zones.is_empty() {
        zoned(None)
    } else {
        zones
            .iter()
            .fold(ZoneValues::default(), |sum, (_, values)| ZoneValues {
                current: sum.current + values.current,
                previous: sum.previous + values.previous,
            })
    };
    let consumption = Some(total.current - total.previous);
    let formula = input
        .formula
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty());
    let Some(formula) = formula else {
        return UtilityFormulaPreview {
            amount: consumption.map(|used| (used * input.tariff).round(2)),
            consumption,
            issue: None,
        };
    };
    let days = Amount::new(rust_decimal::Decimal::from(input.days.unwrap_or(30)));
    let formula_input = FormulaInput {
        total,
        zones,
        tariff: input.tariff,
        days,
    };
    match utilities_formula::evaluate(formula, &formula_input) {
        Ok(amount) => UtilityFormulaPreview {
            amount: Some(amount),
            consumption,
            issue: None,
        },
        Err(issue) => UtilityFormulaPreview {
            amount: None,
            consumption,
            issue: Some(issue),
        },
    }
}

/// A meter as stored: zones trimmed, an empty formula dropped.
fn tidy_meter(kind: &mut UtilityKind) {
    if let UtilityKind::Metered { formula, zones, .. } = kind {
        *formula = formula
            .as_deref()
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_string);
        *zones = zones
            .iter()
            .map(|zone| zone.trim().to_string())
            .filter(|zone| !zone.is_empty())
            .collect();
    }
}

/// The zone names of an item: trimmed, without blanks.
fn zones_of(item: &UtilityItem) -> Vec<String> {
    match &item.kind {
        UtilityKind::Metered { zones, .. } => zones
            .iter()
            .map(|zone| zone.trim().to_string())
            .filter(|zone| !zone.is_empty())
            .collect(),
        _ => Vec::new(),
    }
}

fn hundred() -> Amount {
    Amount::new(rust_decimal::Decimal::from(100))
}

/// Least-squares line through `values` (x = 0, 1, …), evaluated one step on.
fn next_on_line(values: &[Amount]) -> Option<Amount> {
    if values.len() < 2 {
        return None;
    }
    let points: Vec<(f64, f64)> = values
        .iter()
        .enumerate()
        .map(|(index, value)| (index as f64, value.to_f64()))
        .collect();
    let count = points.len() as f64;
    let mean_x = points.iter().map(|(x_val, _)| x_val).sum::<f64>() / count;
    let mean_y = points.iter().map(|(_, y_val)| y_val).sum::<f64>() / count;
    let (num, den) = points
        .iter()
        .fold((0.0, 0.0), |(num, den), (x_val, y_val)| {
            (
                num + (x_val - mean_x) * (y_val - mean_y),
                den + (x_val - mean_x).powi(2),
            )
        });
    let slope = if den == 0.0 { 0.0 } else { num / den };
    let next = mean_y + slope * (count - mean_x);
    Some(Amount::from_f64(next.max(0.0)).round(2))
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

impl UnlockedVault {
    /// The property summary: value, debt, equity and monthly costs.
    pub fn property_summary(&self, store_id: &str, zone: &str) -> Result<PropertySummary> {
        let store = self
            .find_store(store_id)
            .filter(|store| store.kind == crate::model::store::StoreKind::Property)
            .ok_or_else(|| Error::Missing(format!("property {store_id}")))?;
        let (base, fx) = self.base_and_fx();
        let now = secs_or_now(&now_iso());
        let book = self.rate_book();
        let value_usd = self
            .doc
            .holdings
            .iter()
            .filter(|holding| holding.store_id == store.id)
            .fold(Amount::ZERO, |sum, holding| {
                sum + holding.quantity * self.price_usd_for(&holding.asset.key())
            });
        let loans: Vec<&crate::model::store::Store> = self
            .doc
            .stores
            .iter()
            .filter(|loan| !loan.archived && loan.secured_by.as_deref() == Some(store_id))
            .collect();
        let debt_usd = loans.iter().fold(Amount::ZERO, |sum, loan| {
            let balance = self
                .doc
                .holdings
                .iter()
                .filter(|holding| holding.store_id == loan.id)
                .fold(Amount::ZERO, |acc, holding| {
                    acc + holding.quantity * self.price_usd_for(&holding.asset.key())
                });
            sum - balance.min(Amount::ZERO)
        });
        let to_base = |amount: Amount, currency: &str| {
            book.convert(amount, currency, &base, now)
                .unwrap_or(Amount::ZERO)
        };
        let loan_month_base = loans.iter().fold(Amount::ZERO, |sum, loan| {
            let payment = loan
                .liability
                .as_ref()
                .and_then(|terms| terms.monthly_payment)
                .unwrap_or(Amount::ZERO);
            // A loan's currency is what it holds; an empty one is in the base currency.
            let currency = self
                .doc
                .holdings
                .iter()
                .find(|holding| holding.store_id == loan.id)
                .map(|holding| holding.asset.symbol.to_lowercase())
                .unwrap_or_else(|| base.clone());
            sum + to_base(payment, &currency)
        });
        let items: Vec<&UtilityItem> = self
            .doc
            .utility_items
            .iter()
            .filter(|item| item.active && item.property_id == store_id)
            .collect();
        let utilities_month_base = items.iter().fold(Amount::ZERO, |sum, item| {
            let monthly = self
                .meter_trend(&item.id, zone)
                .map(|trend| trend.monthly)
                .unwrap_or(Amount::ZERO);
            sum + to_base(monthly, &item.currency)
        });
        let value_base = value_usd * fx;
        let debt_base = debt_usd * fx;
        let year = self.property_year(store_id, now);
        let twelve = Amount::new(rust_decimal::Decimal::from(12));
        let is_car = store
            .property
            .as_ref()
            .is_some_and(|info| info.category == crate::model::store::PropertyCategory::Car);
        Ok(PropertySummary {
            rent_12m_base: year.rent.round(2),
            expenses_12m_base: year.expenses.round(2),
            rent_yield_pct: (value_base.is_positive() && year.rent.is_positive()).then(|| {
                (year.rent / value_base * Amount::new(rust_decimal::Decimal::from(100))).round(2)
            }),
            ownership_month_base: is_car.then(|| ((year.expenses + year.wear) / twelve).round(2)),
            wear_12m_base: year.wear.round(2),
            store_id: store.id.clone(),
            base_currency: base,
            value_base,
            debt_base,
            equity_base: value_base - debt_base,
            utilities_month_base,
            loan_month_base,
            monthly_cost_base: utilities_month_base + loan_month_base,
            utility_count: items.len() as u32,
        })
    }

    // -- CRUD -------------------------------------------------------------------

    pub fn utility_items(&self) -> Vec<UtilityItem> {
        let mut out = self.doc.utility_items.clone();
        out.sort_by_key(|item| item.name.to_lowercase());
        out
    }

    fn check_utility(&self, draft: &UtilityDraft) -> Result<()> {
        if draft.name.trim().is_empty() {
            return Err(Error::InvalidInput("name the bill".into()));
        }
        if draft.currency.trim().is_empty() {
            return Err(Error::InvalidInput("choose a currency".into()));
        }
        if !self
            .doc
            .categories
            .iter()
            .any(|category| category.key == draft.category)
        {
            return Err(Error::Missing(format!("category {}", draft.category)));
        }
        let is_property = self.find_store(&draft.property_id).is_some_and(|store| {
            store.kind == crate::model::store::StoreKind::Property && store.parent_id.is_none()
        });
        if !is_property {
            return Err(Error::Missing(format!("property {}", draft.property_id)));
        }
        if let Some(id) = &draft.store_id
            && self.find_store(id).is_none()
        {
            return Err(Error::Missing(format!("store {id}")));
        }
        match &draft.kind {
            UtilityKind::Metered {
                unit,
                tariff,
                formula,
                zones,
            } => {
                let formula = formula
                    .as_deref()
                    .map(str::trim)
                    .filter(|text| !text.is_empty());
                // With a formula the tariff is just one of its names, and may be 0.
                if unit.trim().is_empty()
                    || tariff.is_negative()
                    || (formula.is_none() && !tariff.is_positive())
                {
                    return Err(Error::InvalidInput(
                        "a meter needs a unit and a tariff".into(),
                    ));
                }
                let keys: Vec<String> = zones
                    .iter()
                    .map(|zone| utilities_formula::zone_key(zone))
                    .filter(|key| !key.is_empty())
                    .collect();
                let unique: std::collections::BTreeSet<&String> = keys.iter().collect();
                if unique.len() != keys.len() || keys.len() > 4 {
                    return Err(Error::InvalidInput(
                        "zones: up to 4, each named once".into(),
                    ));
                }
                if let Some(text) = formula
                    && let Err(issue) = utilities_formula::check(text, zones)
                {
                    return Err(Error::InvalidInput(format!(
                        "formula, character {}: {}",
                        issue.position, issue.text_en
                    )));
                }
            }
            UtilityKind::Static { amount } if !amount.is_positive() => {
                return Err(Error::InvalidInput("the amount must be positive".into()));
            }
            UtilityKind::Housing {
                amount,
                deposit,
                due_day,
                automation_id,
            } => {
                if !amount.is_positive() || deposit.is_some_and(|value| value.is_negative()) {
                    return Err(Error::InvalidInput("rent must be positive".into()));
                }
                if !(1..=31).contains(due_day) {
                    return Err(Error::InvalidInput("the due day is 1–31".into()));
                }
                if let Some(id) = automation_id
                    && self.automation(id).is_err()
                {
                    return Err(Error::Missing(format!("automation {id}")));
                }
            }
            UtilityKind::Static { .. } => {}
        }
        Ok(())
    }

    pub fn create_utility_item(&mut self, mut draft: UtilityDraft) -> Result<UtilityItem> {
        self.check_utility(&draft)?;
        tidy_meter(&mut draft.kind);
        let item = UtilityItem {
            id: uuid::Uuid::new_v4().to_string(),
            name: draft.name.trim().to_string(),
            active: draft.active,
            kind: draft.kind,
            currency: draft.currency.trim().to_lowercase(),
            property_id: draft.property_id,
            store_id: draft.store_id,
            category: draft.category,
        };
        self.doc.utility_items.push(item.clone());
        self.persist()?;
        Ok(item)
    }

    pub fn update_utility_item(
        &mut self,
        id: &str,
        mut draft: UtilityDraft,
    ) -> Result<UtilityItem> {
        self.check_utility(&draft)?;
        tidy_meter(&mut draft.kind);
        let item = self
            .doc
            .utility_items
            .iter_mut()
            .find(|item| item.id == id)
            .ok_or_else(|| Error::Missing(format!("utility {id}")))?;
        item.name = draft.name.trim().to_string();
        item.active = draft.active;
        item.kind = draft.kind;
        item.currency = draft.currency.trim().to_lowercase();
        item.property_id = draft.property_id;
        item.store_id = draft.store_id;
        item.category = draft.category;
        let out = item.clone();
        self.persist()?;
        Ok(out)
    }

    /// Remove an item with its meter readings; payments already made stay.
    pub fn delete_utility_item(&mut self, id: &str) -> Result<()> {
        let before = self.doc.utility_items.len();
        self.doc.utility_items.retain(|item| item.id != id);
        if self.doc.utility_items.len() == before {
            return Err(Error::Missing(format!("utility {id}")));
        }
        self.doc
            .meter_readings
            .retain(|reading| reading.item_id != id);
        self.persist()
    }

    fn utility_item(&self, id: &str) -> Result<&UtilityItem> {
        self.doc
            .utility_items
            .iter()
            .find(|item| item.id == id)
            .ok_or_else(|| Error::Missing(format!("utility {id}")))
    }

    // -- readings ---------------------------------------------------------------

    /// Record a meter reading for a month (`YYYY-MM`) and, optionally, a day of
    /// it; one already there for the same month and day is replaced. A month
    /// may hold several readings (a move-out on the 15th). A meter only counts
    /// up: a reading below an earlier one or above a later one is refused.
    pub fn add_meter_reading(
        &mut self,
        item_id: &str,
        value: Amount,
        period: &str,
        day: Option<u8>,
        zone: Option<&str>,
    ) -> Result<MeterReading> {
        let reading = self.insert_reading(item_id, value, period, day, zone)?;
        self.persist()?;
        Ok(reading)
    }

    /// Correct a reading: its value and, if moved, its month and day. The
    /// same rules as a new one apply against every other reading of the meter.
    pub fn update_meter_reading(
        &mut self,
        id: &str,
        value: Amount,
        period: &str,
        day: Option<u8>,
    ) -> Result<MeterReading> {
        let index = self
            .doc
            .meter_readings
            .iter()
            .position(|reading| reading.id == id)
            .ok_or_else(|| Error::Missing(format!("reading {id}")))?;
        let old = self.doc.meter_readings.remove(index);
        match self.insert_reading(&old.item_id, value, period, day, old.zone.as_deref()) {
            Ok(mut reading) => {
                // Same reading, corrected: it keeps its identity.
                reading.id = old.id.clone();
                if let Some(stored) = self.doc.meter_readings.iter_mut().find(|stored| {
                    stored.item_id == reading.item_id
                        && stored.zone == reading.zone
                        && stored.sort_key() == reading.sort_key()
                }) {
                    stored.id = old.id;
                }
                self.persist()?;
                Ok(reading)
            }
            Err(err) => {
                self.doc.meter_readings.insert(index, old);
                Err(err)
            }
        }
    }

    /// Validate and store a reading without writing the file.
    fn insert_reading(
        &mut self,
        item_id: &str,
        value: Amount,
        period: &str,
        day: Option<u8>,
        zone: Option<&str>,
    ) -> Result<MeterReading> {
        let item = self.utility_item(item_id)?;
        if !matches!(item.kind, UtilityKind::Metered { .. }) {
            return Err(Error::InvalidInput(
                "only a metered bill takes readings".into(),
            ));
        }
        let zones = zones_of(item);
        // A multi-rate meter takes a reading per zone, stored under the zone's own name.
        let zone = match zone.map(utilities_formula::zone_key) {
            None if zones.is_empty() => None,
            None => {
                return Err(Error::InvalidInput(format!(
                    "pick the zone: {}",
                    zones.join(", ")
                )));
            }
            Some(key) => Some(
                zones
                    .iter()
                    .find(|zone| utilities_formula::zone_key(zone) == key)
                    .cloned()
                    .ok_or_else(|| Error::InvalidInput(format!("the meter has no zone «{key}»")))?,
            ),
        };
        let month = calendar::parse_month(period)
            .ok_or_else(|| Error::InvalidInput(format!("month {period}: expected YYYY-MM")))?;
        let period = calendar::month_key(month);
        if let Some(day) = day
            && (day == 0 || u32::from(day) > calendar::days_in_month(month))
        {
            return Err(Error::InvalidInput(format!("day {day} is not in {period}")));
        }
        if value.is_negative() {
            return Err(Error::InvalidInput("a reading cannot be negative".into()));
        }
        // No day given means the month's end; it is stored as that day so the
        // reading keeps its place whenever it was entered.
        let day = day.or_else(|| u8::try_from(calendar::days_in_month(month)).ok());
        let reading = MeterReading {
            id: uuid::Uuid::new_v4().to_string(),
            item_id: item_id.to_string(),
            value,
            period,
            recorded_at: now_iso(),
            day,
            zone: zone.clone(),
        };
        let key = reading.sort_key();
        let readings = self.readings_in(item_id, zone.as_deref());
        if let Some(before) = readings
            .iter()
            .rev()
            .find(|reading| reading.sort_key() < key)
            && value < before.value
        {
            return Err(Error::InvalidInput(format!(
                "the reading is below an earlier one ({})",
                before.value
            )));
        }
        if let Some(after) = readings.iter().find(|reading| reading.sort_key() > key)
            && value > after.value
        {
            return Err(Error::InvalidInput(format!(
                "the reading is above a later one ({})",
                after.value
            )));
        }
        // One reading per meter per day: a second one that day corrects it.
        self.doc.meter_readings.retain(|stored| {
            !(stored.item_id == item_id && stored.zone == zone && stored.sort_key() == key)
        });
        self.doc.meter_readings.push(reading.clone());
        Ok(reading)
    }

    pub fn delete_meter_reading(&mut self, id: &str) -> Result<()> {
        let before = self.doc.meter_readings.len();
        self.doc.meter_readings.retain(|reading| reading.id != id);
        if self.doc.meter_readings.len() == before {
            return Err(Error::Missing(format!("reading {id}")));
        }
        self.persist()
    }

    /// Readings of an item, oldest first.
    fn readings_of(&self, item_id: &str) -> Vec<MeterReading> {
        let mut out: Vec<MeterReading> = self
            .doc
            .meter_readings
            .iter()
            .filter(|reading| reading.item_id == item_id)
            .cloned()
            .collect();
        out.sort_by_key(MeterReading::sort_key);
        out
    }

    /// Readings of one zone of an item (`None` = a one-rate meter), oldest first.
    fn readings_in(&self, item_id: &str, zone: Option<&str>) -> Vec<MeterReading> {
        let mut out = self.readings_of(item_id);
        out.retain(|reading| reading.zone.as_deref() == zone);
        out
    }

    /// Where the meter stood at the start and at the end of each month with a
    /// reading. A multi-rate meter counts a month once every zone is read in
    /// it, the totals summing the zones.
    fn monthly_readings(&self, item_id: &str) -> BTreeMap<String, MonthReadings> {
        let zones = self.utility_item(item_id).map(zones_of).unwrap_or_default();
        if zones.is_empty() {
            return self.zone_months(item_id, None);
        }
        let per_zone: Vec<(String, BTreeMap<String, MonthReadings>)> = zones
            .iter()
            .map(|zone| (zone.clone(), self.zone_months(item_id, Some(zone))))
            .collect();
        let mut out = BTreeMap::new();
        let Some((_, first_zone)) = per_zone.first() else {
            return out;
        };
        for (period, lead) in first_zone {
            let parts: Option<Vec<(String, MonthReadings)>> = per_zone
                .iter()
                .map(|(zone, months)| {
                    months
                        .get(period)
                        .map(|month| (zone.clone(), month.clone()))
                })
                .collect();
            let Some(parts) = parts else {
                continue;
            };
            let start = parts
                .iter()
                .map(|(_, month)| month.start)
                .try_fold(Amount::ZERO, |sum, start| start.map(|value| sum + value));
            out.insert(
                period.clone(),
                MonthReadings {
                    start,
                    end: parts
                        .iter()
                        .fold(Amount::ZERO, |sum, (_, month)| sum + month.end),
                    end_id: lead.end_id.clone(),
                    start_at: lead.start_at.clone(),
                    end_at: lead.end_at.clone(),
                    zones: parts,
                },
            );
        }
        out
    }

    /// One zone's months. A month starts from the last reading before it; the
    /// very first month of a meter starts from its own first reading, so a
    /// flat moved into on the 15th counts from the 15th.
    fn zone_months(&self, item_id: &str, zone: Option<&str>) -> BTreeMap<String, MonthReadings> {
        let mut out: BTreeMap<String, MonthReadings> = BTreeMap::new();
        let mut previous: Option<&MeterReading> = None;
        let readings = self.readings_in(item_id, zone);
        for group in readings.chunk_by(|left, right| left.period == right.period) {
            let (Some(first), Some(last)) = (group.first(), group.last()) else {
                continue;
            };
            let base = previous.or((group.len() > 1).then_some(first));
            out.insert(
                last.period.clone(),
                MonthReadings {
                    start: base.map(|reading| reading.value),
                    end: last.value,
                    end_id: last.id.clone(),
                    start_at: base.map(MeterReading::sort_key),
                    end_at: last.sort_key(),
                    zones: Vec::new(),
                },
            );
            previous = Some(last);
        }
        out
    }

    /// The month a meter is billed for in `period`: its own readings, else the
    /// latest month read before it that has a consumption and no payment from
    /// then on — a meter read in September is paid in October, once.
    fn billable_month(
        &self,
        item_id: &str,
        period: &str,
        paid: &BTreeMap<String, Amount>,
    ) -> Option<MonthReadings> {
        let mut monthly = self.monthly_readings(item_id);
        if let Some(month) = monthly.remove(period) {
            return Some(month);
        }
        monthly
            .range(..period.to_string())
            .next_back()
            .filter(|(read, month)| {
                month.start.is_some() && paid.range((*read).clone()..).next().is_none()
            })
            .map(|(_, month)| month.clone())
    }

    /// Payments made for an item: linked to it, or — from before the link —
    /// with its name as the comment, its category and its store.
    pub(crate) fn item_payments<'doc>(
        &'doc self,
        item: &'doc UtilityItem,
    ) -> impl Iterator<Item = &'doc PaymentTx> + 'doc {
        self.doc.transactions.iter().filter_map(move |tx| {
            let Transaction::Payment(payment) = tx else {
                return None;
            };
            let linked = match payment.meta.utility_item_id.as_deref() {
                Some(id) => id == item.id,
                None => {
                    payment.meta.comment.trim() == item.name
                        && payment.meta.categories.contains(&item.category)
                        && item
                            .store_id
                            .as_ref()
                            .is_none_or(|id| *id == payment.store_id)
                }
            };
            linked.then_some(payment)
        })
    }

    /// The item's payments by period, in its currency.
    fn paid_by_month(&self, item: &UtilityItem, cal: &Calendar) -> BTreeMap<String, Amount> {
        let book = self.rate_book();
        let mut out: BTreeMap<String, Amount> = BTreeMap::new();
        for payment in self.item_payments(item) {
            let secs = secs_or_now(&payment.meta.timestamp);
            let amount = if payment.asset.symbol.eq_ignore_ascii_case(&item.currency) {
                payment.amount
            } else {
                let usd = payment
                    .price
                    .price_usd
                    .or_else(|| book.price_at(&payment.asset.key(), secs))
                    .unwrap_or(Amount::ZERO);
                book.convert(payment.amount * usd, "usd", &item.currency, secs)
                    .unwrap_or(Amount::ZERO)
            };
            *out.entry(calendar::month_key(cal.date_of(secs)))
                .or_default() += amount;
        }
        out
    }

    /// The item's payment transactions by period.
    fn payments_by_month(
        &self,
        item: &UtilityItem,
        cal: &Calendar,
    ) -> BTreeMap<String, Vec<Transaction>> {
        let mut out: BTreeMap<String, Vec<Transaction>> = BTreeMap::new();
        for payment in self.item_payments(item) {
            let secs = secs_or_now(&payment.meta.timestamp);
            out.entry(calendar::month_key(cal.date_of(secs)))
                .or_default()
                .push(Transaction::Payment(payment.clone()));
        }
        out
    }

    // -- trend --------------------------------------------------------------------

    /// Bills by month with the average, change, forecast and anomaly (plan §7.5).
    pub fn meter_trend(&self, item_id: &str, zone: &str) -> Result<UtilityTrend> {
        let item = self.utility_item(item_id)?.clone();
        let cal = Calendar::new(zone);
        let mut payments = self.payments_by_month(&item, &cal);
        let mut months: Vec<UtilityMonth> = match &item.kind {
            UtilityKind::Metered { .. } => {
                let readings = self.readings_of(&item.id);
                self.monthly_readings(&item.id)
                    .into_iter()
                    .map(|(period, month)| {
                        let consumption = month.consumption();
                        UtilityMonth {
                            reading: Some(month.end),
                            consumption,
                            // A formula that cannot be worked out (÷ 0) bills nothing.
                            amount: metered_amount(&item, &month)
                                .ok()
                                .flatten()
                                .unwrap_or(Amount::ZERO),
                            reading_id: Some(month.end_id.clone()),
                            zones: month
                                .zones
                                .iter()
                                .map(|(zone, part)| ZoneMonth {
                                    zone: zone.clone(),
                                    reading: part.end,
                                    consumption: part.consumption(),
                                })
                                .collect(),
                            payments: Vec::new(),
                            readings: readings
                                .iter()
                                .filter(|reading| reading.period == period)
                                .cloned()
                                .collect(),
                            period,
                        }
                    })
                    .collect()
            }
            UtilityKind::Static { .. } | UtilityKind::Housing { .. } => self
                .paid_by_month(&item, &cal)
                .into_iter()
                .map(|(period, amount)| UtilityMonth {
                    period,
                    amount: amount.round(2),
                    ..UtilityMonth::default()
                })
                .collect(),
        };
        for month in &mut months {
            month.payments = payments.remove(&month.period).unwrap_or_default();
        }
        let billed: Vec<Amount> = months
            .iter()
            .filter(|month| month.amount.is_positive())
            .map(|month| month.amount)
            .collect();
        let recent: Vec<Amount> = billed
            .iter()
            .rev()
            .take(TREND_MONTHS)
            .rev()
            .copied()
            .collect();
        let average = (!recent.is_empty()).then(|| {
            let sum = recent.iter().fold(Amount::ZERO, |acc, value| acc + *value);
            (sum / Amount::new(rust_decimal::Decimal::from(recent.len() as u64))).round(2)
        });
        let delta_pct = match billed.as_slice() {
            [.., before, last] if before.is_positive() => {
                Some(((*last - *before) / *before * hundred()).round(1))
            }
            _ => None,
        };
        let anomaly = match recent.split_last() {
            Some((last, earlier)) if !earlier.is_empty() => {
                let sum = earlier.iter().fold(Amount::ZERO, |acc, value| acc + *value);
                let mean = sum / Amount::new(rust_decimal::Decimal::from(earlier.len() as u64));
                *last * Amount::new(rust_decimal::Decimal::from(2))
                    > mean * Amount::new(rust_decimal::Decimal::from(3))
            }
            _ => false,
        };
        let monthly = match &item.kind {
            UtilityKind::Housing { amount, .. } => *amount,
            UtilityKind::Static { amount } => billed.last().copied().unwrap_or(*amount),
            UtilityKind::Metered { .. } => billed.last().copied().unwrap_or(Amount::ZERO),
        };
        Ok(UtilityTrend {
            item_id: item.id.clone(),
            currency: item.currency.clone(),
            forecast: next_on_line(&recent),
            months,
            average,
            delta_pct,
            anomaly,
            monthly,
        })
    }

    // -- the payment ----------------------------------------------------------------

    /// The «Оплатить» for one month of an item (plan §7.5): metered —
    /// consumption × tariff; fixed — what was paid last time, else the set
    /// amount; rent — the monthly amount plus the deposit with the first one.
    pub fn utility_payment_draft(
        &self,
        item_id: &str,
        period: &str,
        zone: &str,
    ) -> Result<UtilityPaymentDraft> {
        let item = self.utility_item(item_id)?.clone();
        let month = calendar::parse_month(period)
            .ok_or_else(|| Error::InvalidInput(format!("month {period}: expected YYYY-MM")))?;
        let period = calendar::month_key(month);
        let cal = Calendar::new(zone);
        let paid = self.paid_by_month(&item, &cal);
        let mut consumption = None;
        let mut includes_deposit = false;
        let amount = match &item.kind {
            UtilityKind::Metered { .. } => match self.billable_month(&item.id, &period, &paid) {
                Some(month) if month.start.is_some() => {
                    consumption = month.consumption();
                    metered_amount(&item, &month)
                        .map_err(|issue| {
                            Error::InvalidInput(format!(
                                "{}: formula, character {}: {}",
                                item.name, issue.position, issue.text_en
                            ))
                        })?
                        .unwrap_or(Amount::ZERO)
                }
                _ => {
                    consumption = Some(Amount::ZERO);
                    Amount::ZERO
                }
            },
            UtilityKind::Static { amount } => paid
                .range(..period.clone())
                .next_back()
                .map_or(*amount, |(_, last)| *last),
            UtilityKind::Housing {
                amount, deposit, ..
            } => {
                includes_deposit =
                    deposit.is_some_and(|value| value.is_positive()) && paid.is_empty();
                *amount
                    + if includes_deposit {
                        deposit.unwrap_or_default()
                    } else {
                        Amount::ZERO
                    }
            }
        };
        let code = item.currency.to_uppercase();
        let mut meta = TxMeta::new(&item.name);
        meta.categories = vec![item.category.clone()];
        meta.utility_item_id = Some(item.id.clone());
        // Dated today when the month is this one, else inside that month.
        let last_day = calendar::next_month(month) - chrono::Duration::days(1);
        let day = cal
            .date_of(calendar::now_secs())
            .clamp(first_of_month(month), last_day);
        meta.timestamp = calendar::iso(cal.at_local(day, 12));
        Ok(UtilityPaymentDraft {
            item_id: item.id.clone(),
            period,
            transaction: Transaction::Payment(PaymentTx {
                meta,
                store_id: item.store_id.clone().unwrap_or_default(),
                asset: AssetRef::fiat(&code, &code),
                amount,
                fee: None,
                to: None,
                details: KindDetails::default(),
                price: PriceStamp::default(),
            }),
            consumption,
            includes_deposit,
        })
    }

    // -- the whole bill -------------------------------------------------------------

    /// The month's bill of a property: every active item with its suggested
    /// amount, meters with their previous reading (plan: «всё платится в один день»).
    pub fn utility_bill(&self, property_id: &str, period: &str, zone: &str) -> Result<UtilityBill> {
        let property = self
            .doc
            .stores
            .iter()
            .find(|store| store.id == property_id)
            .ok_or_else(|| Error::Missing(format!("store {property_id}")))?;
        let pay_day = property
            .property
            .as_ref()
            .and_then(|info| info.utilities_pay_day);
        let month = calendar::parse_month(period)
            .ok_or_else(|| Error::InvalidInput(format!("month {period}: expected YYYY-MM")))?;
        let period = calendar::month_key(month);
        let cal = Calendar::new(zone);
        let items: Vec<UtilityItem> = self
            .doc
            .utility_items
            .iter()
            .filter(|item| item.property_id == property_id && item.active)
            .cloned()
            .collect();
        let mut lines = Vec::with_capacity(items.len());
        for item in items {
            let paid = self
                .paid_by_month(&item, &cal)
                .get(&period)
                .copied()
                .unwrap_or(Amount::ZERO);
            let mut line = UtilityBillLine {
                item: item.clone(),
                previous_reading: None,
                reading: None,
                consumption: None,
                amount: None,
                paid,
                includes_deposit: false,
                zones: Vec::new(),
            };
            let mut unread = false;
            if let UtilityKind::Metered { .. } = item.kind {
                let paid_months = self.paid_by_month(&item, &cal);
                let billable = self.billable_month(&item.id, &period, &paid_months);
                line.zones = self.zone_lines(&item, &period, billable.as_ref());
                match billable {
                    // Read (this month or an unpaid one before): start to end.
                    Some(month) if month.start.is_some() => {
                        line.previous_reading = month.start;
                        line.reading = Some(month.end);
                    }
                    // Only a starting reading (the first one, or moved in this
                    // month): it is the base, the reading is entered here.
                    Some(month) => {
                        unread = true;
                        line.previous_reading = Some(month.end);
                    }
                    None => {
                        // Not read yet: the reading is entered on the bill itself.
                        unread = true;
                        line.previous_reading = self
                            .monthly_readings(&item.id)
                            .range(..period.clone())
                            .next_back()
                            .map(|(_, month)| month.end);
                    }
                }
            }
            if !unread
                && let Ok(draft) = self.utility_payment_draft(&item.id, &period, zone)
                && let Transaction::Payment(payment) = &draft.transaction
            {
                line.amount = Some(payment.amount);
                line.consumption = draft.consumption;
                line.includes_deposit = draft.includes_deposit;
            }
            lines.push(line);
        }
        let last_day = calendar::next_month(month) - chrono::Duration::days(1);
        let today = cal.date_of(calendar::now_secs());
        let day = match pay_day {
            Some(day) => {
                let wanted =
                    first_of_month(month) + chrono::Duration::days(i64::from(day.clamp(1, 31)) - 1);
                wanted.min(last_day)
            }
            None => today.clamp(first_of_month(month), last_day),
        };
        Ok(UtilityBill {
            property_id: property_id.to_string(),
            period,
            pay_day,
            due_at: calendar::iso(cal.at_local(day, 12)),
            lines,
        })
    }

    /// A multi-rate meter's zones on the bill: the month's readings when it is
    /// read, else each zone's last reading before it as the base.
    fn zone_lines(
        &self,
        item: &UtilityItem,
        period: &str,
        billable: Option<&MonthReadings>,
    ) -> Vec<ZoneBillLine> {
        zones_of(item)
            .into_iter()
            .map(|zone| {
                let read = billable
                    .filter(|month| month.start.is_some())
                    .and_then(|month| month.zones.iter().find(|(name, _)| *name == zone))
                    .map(|(_, part)| part);
                match read {
                    Some(part) => ZoneBillLine {
                        previous_reading: part.start,
                        reading: Some(part.end),
                        zone,
                    },
                    None => ZoneBillLine {
                        previous_reading: self
                            .zone_months(&item.id, Some(&zone))
                            .range(..=period.to_string())
                            .next_back()
                            .map(|(_, month)| month.end),
                        reading: None,
                        zone,
                    },
                }
            })
            .collect()
    }

    /// Pay a month's bill in one go: record the meter readings, then one
    /// payment per line, linked to its item. All or nothing — a line that
    /// fails leaves the vault as it was.
    pub fn pay_utility_bill(
        &mut self,
        input: PayUtilityBillInput,
        zone: &str,
    ) -> Result<Vec<Transaction>> {
        let bill = self.utility_bill(&input.property_id, &input.period, zone)?;
        let timestamp = input
            .timestamp
            .clone()
            .filter(|raw| !raw.trim().is_empty())
            .unwrap_or_else(|| bill.due_at.clone());
        let backup = self.doc.clone();
        let result = (|| -> Result<Vec<Transaction>> {
            let mut saved = Vec::new();
            for line in &input.lines {
                if !line.amount.is_positive() {
                    continue;
                }
                let item = self.utility_item(&line.item_id)?.clone();
                if item.property_id != input.property_id {
                    return Err(Error::InvalidInput(format!(
                        "{} belongs to another property",
                        item.name
                    )));
                }
                if let Some(reading) = line.reading
                    && line.zone_readings.is_empty()
                {
                    self.insert_reading(&item.id, reading, &bill.period, line.reading_day, None)?;
                }
                for zoned in &line.zone_readings {
                    self.insert_reading(
                        &item.id,
                        zoned.reading,
                        &bill.period,
                        line.reading_day,
                        Some(&zoned.zone),
                    )?;
                }
                let store_id = item
                    .store_id
                    .clone()
                    .filter(|id| !id.is_empty())
                    .unwrap_or_else(|| input.store_id.clone());
                if store_id.trim().is_empty() {
                    return Err(Error::InvalidInput(format!(
                        "{}: pick the store to pay from",
                        item.name
                    )));
                }
                let code = item.currency.to_uppercase();
                let mut meta = TxMeta::new(&item.name);
                meta.categories = vec![item.category.clone()];
                meta.utility_item_id = Some(item.id.clone());
                meta.timestamp = timestamp.clone();
                let mut tx = Transaction::Payment(PaymentTx {
                    meta,
                    store_id,
                    asset: AssetRef::fiat(&code, &code),
                    amount: line.amount,
                    fee: None,
                    to: None,
                    details: KindDetails::default(),
                    price: PriceStamp::default(),
                });
                self.prepare_transaction(&mut tx)?;
                self.doc.transactions.push(tx.clone());
                saved.push(tx);
            }
            Ok(saved)
        })();
        match result {
            Ok(saved) => {
                self.recompute_holdings();
                self.persist()?;
                Ok(saved)
            }
            Err(err) => {
                self.doc = backup;
                Err(err)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::store::StoreKind;
    use crate::service::test_support::*;

    fn metered(vault: &mut UnlockedVault) -> UtilityItem {
        metered_in(vault, "Flat")
    }

    fn metered_in(vault: &mut UnlockedVault, flat: &str) -> UtilityItem {
        let flat = store(vault, flat, StoreKind::Property, None);
        vault
            .create_utility_item(UtilityDraft {
                name: "Electricity".into(),
                active: true,
                kind: UtilityKind::Metered {
                    unit: "kWh".into(),
                    tariff: amount("5"),
                    formula: None,
                    zones: Vec::new(),
                },
                currency: "rub".into(),
                property_id: flat,
                store_id: None,
                category: "utilities".into(),
            })
            .unwrap()
    }

    #[test]
    fn a_meter_bills_consumption_times_tariff() {
        let (_dir, mut vault) = empty_vault();
        let item = metered(&mut vault);
        vault
            .add_meter_reading(&item.id, amount("1000"), "2026-01", None, None)
            .unwrap();
        vault
            .add_meter_reading(&item.id, amount("1150"), "2026-02", None, None)
            .unwrap();
        let draft = vault
            .utility_payment_draft(&item.id, "2026-02", "UTC")
            .unwrap();
        assert_eq!(draft.consumption, Some(amount("150")));
        let Transaction::Payment(payment) = &draft.transaction else {
            panic!()
        };
        assert_eq!(payment.amount, amount("750"));
        assert_eq!(payment.meta.categories, vec!["utilities"]);
        // A meter only counts up.
        assert!(
            vault
                .add_meter_reading(&item.id, amount("900"), "2026-03", None, None)
                .is_err()
        );
        let trend = vault.meter_trend(&item.id, "UTC").unwrap();
        assert_eq!(trend.months[1].amount, amount("750"));
    }

    #[test]
    fn a_month_without_a_reading_bills_the_last_unpaid_one() {
        let (_dir, mut vault) = empty_vault();
        let (flat, item, _internet, card) = flat_with_bills(&mut vault);
        // Nothing read yet: an empty draft, not an error.
        let empty = vault
            .utility_payment_draft(&item.id, "2026-03", "UTC")
            .unwrap();
        assert_eq!(empty.consumption, Some(Amount::ZERO));
        vault
            .add_meter_reading(&item.id, amount("1000"), "2026-01", None, None)
            .unwrap();
        vault
            .add_meter_reading(&item.id, amount("1150"), "2026-02", None, None)
            .unwrap();
        // Read in February, paid in March.
        let draft = vault
            .utility_payment_draft(&item.id, "2026-03", "UTC")
            .unwrap();
        assert_eq!(draft.consumption, Some(amount("150")));
        // The month's bill picks up the same readings.
        let bill = vault.utility_bill(&flat, "2026-03", "UTC").unwrap();
        let line = bill
            .lines
            .iter()
            .find(|line| line.item.id == item.id)
            .unwrap();
        assert_eq!(line.previous_reading, Some(amount("1000")));
        assert_eq!(line.reading, Some(amount("1150")));
        assert_eq!(line.amount, Some(amount("750")));
        let mut paid = draft.transaction;
        if let Transaction::Payment(payment) = &mut paid {
            payment.store_id = card;
        }
        paid.meta_mut().timestamp = "2026-02-20T12:00:00Z".into();
        vault.add_transaction(paid).unwrap();
        // February is paid: March has nothing left to bill.
        let after = vault
            .utility_payment_draft(&item.id, "2026-03", "UTC")
            .unwrap();
        assert_eq!(after.consumption, Some(Amount::ZERO));
    }

    #[test]
    fn the_first_rent_payment_carries_the_deposit() {
        let (_dir, mut vault) = empty_vault();
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        let mut seed = income_of(
            &card,
            AssetRef::fiat("RUB", "RUB"),
            "200000",
            "0.01",
            "salary",
        );
        seed.meta_mut().timestamp = "2026-01-01T00:00:00Z".into();
        vault.add_transaction(seed).unwrap();
        let flat = store(&mut vault, "Flat", StoreKind::Property, None);
        let item = vault
            .create_utility_item(UtilityDraft {
                name: "Rent".into(),
                active: true,
                kind: UtilityKind::Housing {
                    amount: amount("40000"),
                    deposit: Some(amount("40000")),
                    due_day: 5,
                    automation_id: None,
                },
                currency: "rub".into(),
                property_id: flat,
                store_id: Some(card.clone()),
                category: "rent_mortgage".into(),
            })
            .unwrap();
        let first = vault
            .utility_payment_draft(&item.id, "2026-02", "UTC")
            .unwrap();
        assert!(first.includes_deposit);
        let Transaction::Payment(payment) = first.transaction.clone() else {
            panic!()
        };
        assert_eq!(payment.amount, amount("80000"));
        vault.add_transaction(first.transaction).unwrap();
        let second = vault
            .utility_payment_draft(&item.id, "2026-03", "UTC")
            .unwrap();
        assert!(!second.includes_deposit);
        let Transaction::Payment(payment) = second.transaction else {
            panic!()
        };
        assert_eq!(payment.amount, amount("40000"));
    }

    #[test]
    fn the_trend_forecasts_and_flags_a_spike() {
        let (_dir, mut vault) = empty_vault();
        let item = metered(&mut vault);
        // Consumption 100, 110, 120, 130 → bills 500, 550, 600, 650, then 2000.
        for (period, value) in [
            ("2026-01", "0"),
            ("2026-02", "100"),
            ("2026-03", "210"),
            ("2026-04", "330"),
            ("2026-05", "460"),
        ] {
            vault
                .add_meter_reading(&item.id, amount(value), period, None, None)
                .unwrap();
        }
        let steady = vault.meter_trend(&item.id, "UTC").unwrap();
        assert_eq!(steady.forecast, Some(amount("700")));
        assert!(!steady.anomaly);
        assert_eq!(steady.delta_pct, Some(amount("8.3")));
        assert_eq!(steady.average, Some(amount("575")));

        vault
            .add_meter_reading(&item.id, amount("860"), "2026-06", None, None)
            .unwrap();
        let spike = vault.meter_trend(&item.id, "UTC").unwrap();
        assert!(spike.anomaly);
    }

    #[test]
    fn a_utility_needs_a_property_and_the_summary_adds_it_up() {
        let (_dir, mut vault) = empty_vault();
        let bank = store(&mut vault, "Bank", StoreKind::Bank, None);
        let draft = |property_id: &str| UtilityDraft {
            name: "Internet".into(),
            active: true,
            kind: UtilityKind::Static {
                amount: amount("30"),
            },
            currency: "usd".into(),
            property_id: property_id.into(),
            store_id: None,
            category: "utilities".into(),
        };
        assert!(vault.create_utility_item(draft(&bank)).is_err());
        let flat = store(&mut vault, "Flat", StoreKind::Property, None);
        vault.create_utility_item(draft(&flat)).unwrap();
        let summary = vault.property_summary(&flat, "UTC").unwrap();
        assert_eq!(summary.utility_count, 1);
        assert_eq!(summary.utilities_month_base, amount("30"));
        assert_eq!(summary.monthly_cost_base, amount("30"));
    }

    /// A funded card and a flat with a meter and a fixed bill.
    fn flat_with_bills(vault: &mut UnlockedVault) -> (String, UtilityItem, UtilityItem, String) {
        let card = store(vault, "Card", StoreKind::Bank, None);
        let mut seed = income_of(
            &card,
            AssetRef::fiat("RUB", "RUB"),
            "100000",
            "0.01",
            "salary",
        );
        seed.meta_mut().timestamp = "2026-01-01T00:00:00Z".into();
        vault.add_transaction(seed).unwrap();
        let meter = metered(vault);
        let flat = meter.property_id.clone();
        let internet = vault
            .create_utility_item(UtilityDraft {
                name: "Internet".into(),
                active: true,
                kind: UtilityKind::Static {
                    amount: amount("700"),
                },
                currency: "rub".into(),
                property_id: flat.clone(),
                store_id: None,
                category: "utilities".into(),
            })
            .unwrap();
        (flat, meter, internet, card)
    }

    #[test]
    fn a_whole_bill_is_paid_at_once_and_linked_to_its_items() {
        let (_dir, mut vault) = empty_vault();
        let (flat, meter, internet, card) = flat_with_bills(&mut vault);
        vault
            .add_meter_reading(&meter.id, amount("1000"), "2026-01", None, None)
            .unwrap();

        let bill = vault.utility_bill(&flat, "2026-02", "UTC").unwrap();
        assert_eq!(bill.lines.len(), 2);
        let meter_line = bill
            .lines
            .iter()
            .find(|line| line.item.id == meter.id)
            .unwrap();
        assert_eq!(meter_line.previous_reading, Some(amount("1000")));
        assert_eq!(meter_line.amount, None, "no reading for February yet");

        let paid = vault
            .pay_utility_bill(
                PayUtilityBillInput {
                    property_id: flat.clone(),
                    period: "2026-02".into(),
                    store_id: card,
                    timestamp: Some("2026-02-10T12:00:00Z".into()),
                    lines: vec![
                        PayUtilityLine {
                            item_id: meter.id.clone(),
                            amount: amount("500"),
                            reading: Some(amount("1100")),
                            reading_day: None,
                            zone_readings: Vec::new(),
                        },
                        PayUtilityLine {
                            item_id: internet.id.clone(),
                            amount: amount("700"),
                            reading: None,
                            reading_day: None,
                            zone_readings: Vec::new(),
                        },
                    ],
                },
                "UTC",
            )
            .unwrap();
        assert_eq!(paid.len(), 2);
        assert!(paid.iter().all(|tx| tx.meta().utility_item_id.is_some()));
        // The comment no longer has to match: the link counts.
        let after = vault.utility_bill(&flat, "2026-02", "UTC").unwrap();
        let internet_line = after
            .lines
            .iter()
            .find(|line| line.item.id == internet.id)
            .unwrap();
        assert_eq!(internet_line.paid, amount("700"));
        let meter_line = after
            .lines
            .iter()
            .find(|line| line.item.id == meter.id)
            .unwrap();
        assert_eq!(meter_line.reading, Some(amount("1100")));
    }

    #[test]
    fn a_failing_line_leaves_the_vault_untouched() {
        let (_dir, mut vault) = empty_vault();
        let (flat, meter, internet, card) = flat_with_bills(&mut vault);
        vault
            .add_meter_reading(&meter.id, amount("1000"), "2026-01", None, None)
            .unwrap();
        let before = vault.doc.transactions.len();
        let result = vault.pay_utility_bill(
            PayUtilityBillInput {
                property_id: flat,
                period: "2026-02".into(),
                store_id: card,
                timestamp: None,
                lines: vec![
                    PayUtilityLine {
                        item_id: internet.id,
                        amount: amount("700"),
                        reading: None,
                        reading_day: None,
                        zone_readings: Vec::new(),
                    },
                    // Below last month's reading: refused.
                    PayUtilityLine {
                        item_id: meter.id.clone(),
                        amount: amount("1"),
                        reading: Some(amount("900")),
                        reading_day: None,
                        zone_readings: Vec::new(),
                    },
                ],
            },
            "UTC",
        );
        assert!(result.is_err());
        assert_eq!(vault.doc.transactions.len(), before);
        assert_eq!(vault.doc.meter_readings.len(), 1);
    }

    #[test]
    fn a_reading_can_be_corrected_within_the_meter_rules() {
        let (_dir, mut vault) = empty_vault();
        let item = metered(&mut vault);
        vault
            .add_meter_reading(&item.id, amount("1000"), "2026-01", None, None)
            .unwrap();
        let feb = vault
            .add_meter_reading(&item.id, amount("1200"), "2026-02", None, None)
            .unwrap();
        let fixed = vault
            .update_meter_reading(&feb.id, amount("1150"), "2026-02", None)
            .unwrap();
        assert_eq!(fixed.id, feb.id);
        assert_eq!(
            vault.monthly_readings(&item.id)["2026-02"].end,
            amount("1150")
        );
        assert!(
            vault
                .update_meter_reading(&feb.id, amount("900"), "2026-02", None)
                .is_err()
        );
        assert_eq!(
            vault.monthly_readings(&item.id)["2026-02"].end,
            amount("1150"),
            "a refused fix keeps the old value"
        );
    }

    /// Moved on the 15th: the old flat is read that day, the new flat's meter
    /// starts that day; each month bill runs from its own start.
    #[test]
    fn a_move_mid_month_bills_each_flat_from_its_own_readings() {
        let (_dir, mut vault) = empty_vault();
        let old_flat = metered_in(&mut vault, "Old flat");
        let new_flat = metered_in(&mut vault, "New flat");
        vault
            .add_meter_reading(&old_flat.id, amount("100"), "2026-09", None, None)
            .unwrap();
        vault
            .add_meter_reading(&old_flat.id, amount("130"), "2026-10", Some(15), None)
            .unwrap();
        vault
            .add_meter_reading(&new_flat.id, amount("500"), "2026-10", Some(15), None)
            .unwrap();

        // Before the month ends the new flat has only its start: nothing used yet.
        let line = |vault: &UnlockedVault, item: &UtilityItem| {
            let property = item.property_id.clone();
            vault
                .utility_bill(&property, "2026-10", "UTC")
                .unwrap()
                .lines
                .into_iter()
                .find(|line| line.item.id == item.id)
                .unwrap()
        };
        let started = line(&vault, &new_flat);
        assert_eq!(started.previous_reading, Some(amount("500")));
        assert_eq!(started.reading, None);

        vault
            .add_meter_reading(&new_flat.id, amount("520"), "2026-10", None, None)
            .unwrap();
        let old_bill = line(&vault, &old_flat);
        assert_eq!(old_bill.previous_reading, Some(amount("100")));
        assert_eq!(old_bill.consumption, Some(amount("30")));
        let new_bill = line(&vault, &new_flat);
        assert_eq!(new_bill.previous_reading, Some(amount("500")));
        assert_eq!(new_bill.consumption, Some(amount("20")));
        assert_eq!(new_bill.amount, Some(amount("100")));

        let trend = vault.meter_trend(&new_flat.id, "UTC").unwrap();
        assert_eq!(trend.months.len(), 1);
        assert_eq!(trend.months[0].readings.len(), 2);
        assert_eq!(trend.months[0].consumption, Some(amount("20")));
    }

    #[test]
    fn readings_of_one_month_keep_their_order_by_day() {
        let (_dir, mut vault) = empty_vault();
        let item = metered(&mut vault);
        vault
            .add_meter_reading(&item.id, amount("100"), "2026-10", Some(10), None)
            .unwrap();
        assert!(
            vault
                .add_meter_reading(&item.id, amount("90"), "2026-10", Some(20), None)
                .is_err(),
            "the 20th cannot be below the 10th"
        );
        assert!(
            vault
                .add_meter_reading(&item.id, amount("110"), "2026-10", Some(5), None)
                .is_err(),
            "the 5th cannot be above the 10th"
        );
        assert!(
            vault
                .add_meter_reading(&item.id, amount("110"), "2026-02", Some(30), None)
                .is_err(),
            "February has no 30th"
        );
        vault
            .add_meter_reading(&item.id, amount("120"), "2026-10", Some(20), None)
            .unwrap();
        assert_eq!(vault.readings_of(&item.id).len(), 2);
    }

    #[test]
    fn a_reading_without_a_day_does_not_block_a_later_one() {
        let (_dir, mut vault) = empty_vault();
        let item = metered(&mut vault);
        // Entered on the 5th without a day, as the old form did.
        vault.doc.meter_readings.push(MeterReading {
            id: "old".into(),
            item_id: item.id.clone(),
            value: amount("2531.5"),
            period: "2026-10".into(),
            recorded_at: "2026-10-05T09:00:00Z".into(),
            day: None,
            zone: None,
        });
        vault
            .add_meter_reading(&item.id, amount("2540"), "2026-10", Some(12), None)
            .unwrap();
        vault
            .add_meter_reading(&item.id, amount("2552"), "2026-10", Some(20), None)
            .unwrap();
        assert!(
            vault
                .add_meter_reading(&item.id, amount("2530"), "2026-10", Some(8), None)
                .is_err(),
            "the 8th still cannot be below the 5th"
        );
        let values: Vec<String> = vault
            .readings_of(&item.id)
            .iter()
            .map(|reading| reading.value.to_string())
            .collect();
        assert_eq!(values, ["2531.5", "2540", "2552"]);
    }

    #[test]
    fn a_second_reading_the_same_day_corrects_the_first() {
        let (_dir, mut vault) = empty_vault();
        let item = metered(&mut vault);
        vault
            .add_meter_reading(&item.id, amount("100"), "2026-10", Some(12), None)
            .unwrap();
        vault
            .add_meter_reading(&item.id, amount("101"), "2026-10", Some(12), None)
            .unwrap();
        let readings = vault.readings_of(&item.id);
        assert_eq!(readings.len(), 1);
        assert_eq!(readings[0].value, amount("101"));
    }

    fn meter_with(vault: &mut UnlockedVault, formula: &str, zones: &[&str]) -> Result<UtilityItem> {
        let name = format!("Dacha {}", vault.doc.stores.len());
        let flat = store(vault, &name, StoreKind::Property, None);
        vault.create_utility_item(UtilityDraft {
            name: "Свет".into(),
            active: true,
            kind: UtilityKind::Metered {
                unit: "kWh".into(),
                tariff: amount("0"),
                formula: Some(formula.into()),
                zones: zones.iter().map(|zone| (*zone).to_string()).collect(),
            },
            currency: "rub".into(),
            property_id: flat,
            store_id: None,
            category: "utilities".into(),
        })
    }

    fn draft_amount(vault: &UnlockedVault, item: &UtilityItem, period: &str) -> Amount {
        let draft = vault
            .utility_payment_draft(&item.id, period, "UTC")
            .unwrap();
        let Transaction::Payment(payment) = &draft.transaction else {
            panic!()
        };
        payment.amount
    }

    #[test]
    fn a_meter_bills_by_its_own_formula() {
        let (_dir, mut vault) = empty_vault();
        let item = meter_with(&mut vault, " (показание - предыдущее) * 3.5 * 10 ", &[]).unwrap();
        let UtilityKind::Metered { formula, .. } = &item.kind else {
            panic!()
        };
        assert_eq!(
            formula.as_deref(),
            Some("(показание - предыдущее) * 3.5 * 10")
        );
        vault
            .add_meter_reading(&item.id, amount("1200"), "2026-01", None, None)
            .unwrap();
        vault
            .add_meter_reading(&item.id, amount("1345"), "2026-02", None, None)
            .unwrap();
        assert_eq!(draft_amount(&vault, &item, "2026-02"), amount("5075"));
        let trend = vault.meter_trend(&item.id, "UTC").unwrap();
        assert_eq!(trend.months[1].amount, amount("5075"));
    }

    #[test]
    fn a_wrong_formula_is_not_saved() {
        let (_dir, mut vault) = empty_vault();
        let err = meter_with(&mut vault, "расход * х", &[]).unwrap_err();
        assert!(err.to_string().contains("character 10"), "{err}");
        assert!(meter_with(&mut vault, "расход_вечер", &["день", "ночь"]).is_err());
        assert!(meter_with(&mut vault, "расход", &["день", " День "]).is_err());
        // Without a formula a meter still needs a tariff.
        let flat = store(&mut vault, "Flat", StoreKind::Property, None);
        let no_tariff = vault.create_utility_item(UtilityDraft {
            name: "Water".into(),
            active: true,
            kind: UtilityKind::Metered {
                unit: "m3".into(),
                tariff: amount("0"),
                formula: Some("  ".into()),
                zones: Vec::new(),
            },
            currency: "rub".into(),
            property_id: flat,
            store_id: None,
            category: "utilities".into(),
        });
        assert!(no_tariff.is_err());
    }

    #[test]
    fn a_two_rate_meter_reads_and_bills_per_zone() {
        let (_dir, mut vault) = empty_vault();
        let item = meter_with(
            &mut vault,
            "расход_день * 6 + расход_ночь * 3 + 100",
            &["День", "Ночь"],
        )
        .unwrap();
        // A zone must be named, and be one of the meter's.
        assert!(
            vault
                .add_meter_reading(&item.id, amount("600"), "2026-01", None, None)
                .is_err()
        );
        assert!(
            vault
                .add_meter_reading(&item.id, amount("600"), "2026-01", None, Some("вечер"))
                .is_err()
        );
        for (value, period, zone) in [
            ("600", "2026-01", "день"),
            ("400", "2026-01", "ночь"),
            ("800", "2026-02", "день"),
            ("500", "2026-02", "ночь"),
        ] {
            vault
                .add_meter_reading(&item.id, amount(value), period, None, Some(zone))
                .unwrap();
        }
        // Each zone counts up on its own: night below day is fine.
        assert_eq!(draft_amount(&vault, &item, "2026-02"), amount("1600"));
        let trend = vault.meter_trend(&item.id, "UTC").unwrap();
        let february = &trend.months[1];
        assert_eq!(february.consumption, Some(amount("300")));
        assert_eq!(february.zones.len(), 2);
        assert_eq!(february.zones[0].zone, "День");
        assert_eq!(february.zones[0].consumption, Some(amount("200")));

        // February is unpaid: March's bill offers it, zone by zone.
        let bill = vault
            .utility_bill(&item.property_id, "2026-03", "UTC")
            .unwrap();
        let line = &bill.lines[0];
        assert_eq!(line.amount, Some(amount("1600")));
        let zones: Vec<_> = line
            .zones
            .iter()
            .map(|zone| (zone.previous_reading, zone.reading))
            .collect();
        assert_eq!(
            zones,
            vec![
                (Some(amount("600")), Some(amount("800"))),
                (Some(amount("400")), Some(amount("500"))),
            ]
        );
        let card = store(&mut vault, "Card", StoreKind::Cash, None);
        vault
            .add_transaction(income(&card, "5000", "opening"))
            .unwrap();
        vault
            .pay_utility_bill(
                PayUtilityBillInput {
                    property_id: item.property_id.clone(),
                    period: "2026-03".into(),
                    store_id: card,
                    timestamp: Some("2026-03-10T12:00:00Z".into()),
                    lines: vec![PayUtilityLine {
                        item_id: item.id.clone(),
                        amount: amount("1000"),
                        reading: None,
                        reading_day: None,
                        zone_readings: vec![
                            ZoneReadingInput {
                                zone: "день".into(),
                                reading: amount("900"),
                            },
                            ZoneReadingInput {
                                zone: "ночь".into(),
                                reading: amount("550"),
                            },
                        ],
                    }],
                },
                "UTC",
            )
            .unwrap();
        let trend = vault.meter_trend(&item.id, "UTC").unwrap();
        // 100 × 6 + 50 × 3 + 100.
        assert_eq!(trend.months[2].amount, amount("850"));
    }

    #[test]
    fn the_preview_prices_the_typed_readings() {
        let zoned = preview_utility_formula(&UtilityFormulaInput {
            tariff: amount("5"),
            formula: Some("расход_день * тариф + расход_ночь * 2".into()),
            zones: vec!["день".into(), "ночь".into()],
            readings: vec![
                FormulaZoneReading {
                    zone: Some("день".into()),
                    previous: amount("10"),
                    current: amount("20"),
                },
                FormulaZoneReading {
                    zone: Some("ночь".into()),
                    previous: amount("5"),
                    current: amount("10"),
                },
            ],
            days: None,
        });
        assert_eq!(zoned.amount, Some(amount("60")));
        assert_eq!(zoned.consumption, Some(amount("15")));
        let plain = preview_utility_formula(&UtilityFormulaInput {
            tariff: amount("5"),
            formula: None,
            zones: Vec::new(),
            readings: vec![FormulaZoneReading {
                zone: None,
                previous: amount("100"),
                current: amount("130"),
            }],
            days: None,
        });
        assert_eq!(plain.amount, Some(amount("150")));
        let broken = preview_utility_formula(&UtilityFormulaInput {
            formula: Some("расход / 0".into()),
            ..plain_input()
        });
        assert!(broken.amount.is_none() && broken.issue.is_some());
    }

    fn plain_input() -> UtilityFormulaInput {
        UtilityFormulaInput {
            tariff: amount("5"),
            formula: None,
            zones: Vec::new(),
            readings: Vec::new(),
            days: None,
        }
    }

    #[test]
    fn a_meter_from_an_older_app_still_reads() {
        let kind = serde_json::json!({ "metered": { "unit": "kWh", "tariff": "5" } });
        let reading = serde_json::json!({
            "id": "r", "item_id": "i", "value": "10", "period": "2026-01",
            "recorded_at": "2026-01-31T00:00:00Z",
        });
        let from_json: UtilityKind = serde_json::from_value(kind.clone()).unwrap();
        assert_eq!(
            from_json,
            UtilityKind::Metered {
                unit: "kWh".into(),
                tariff: amount("5"),
                formula: None,
                zones: Vec::new(),
            }
        );
        let packed = rmp_serde::to_vec_named(&kind).unwrap();
        assert_eq!(
            rmp_serde::from_slice::<UtilityKind>(&packed).unwrap(),
            from_json
        );
        let old: MeterReading = serde_json::from_value(reading.clone()).unwrap();
        assert_eq!(old.zone, None);
        let packed = rmp_serde::to_vec_named(&reading).unwrap();
        assert_eq!(rmp_serde::from_slice::<MeterReading>(&packed).unwrap(), old);
    }
}
