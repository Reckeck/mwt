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
        Ok(PropertySummary {
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
            UtilityKind::Metered { unit, tariff } => {
                if unit.trim().is_empty() || !tariff.is_positive() {
                    return Err(Error::InvalidInput(
                        "a meter needs a unit and a tariff".into(),
                    ));
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
                    && !self
                        .doc
                        .automations
                        .iter()
                        .any(|automation| &automation.id == id)
                {
                    return Err(Error::Missing(format!("automation {id}")));
                }
            }
            UtilityKind::Static { .. } => {}
        }
        Ok(())
    }

    pub fn create_utility_item(&mut self, draft: UtilityDraft) -> Result<UtilityItem> {
        self.check_utility(&draft)?;
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

    pub fn update_utility_item(&mut self, id: &str, draft: UtilityDraft) -> Result<UtilityItem> {
        self.check_utility(&draft)?;
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

    /// Record the meter reading of a month (`YYYY-MM`), replacing one already
    /// there. A meter only counts up: a reading below the previous month's or
    /// above the next month's is refused.
    pub fn add_meter_reading(
        &mut self,
        item_id: &str,
        value: Amount,
        period: &str,
    ) -> Result<MeterReading> {
        let item = self.utility_item(item_id)?;
        if !matches!(item.kind, UtilityKind::Metered { .. }) {
            return Err(Error::InvalidInput(
                "only a metered bill takes readings".into(),
            ));
        }
        let month = calendar::parse_month(period)
            .ok_or_else(|| Error::InvalidInput(format!("month {period}: expected YYYY-MM")))?;
        let period = calendar::month_key(month);
        if value.is_negative() {
            return Err(Error::InvalidInput("a reading cannot be negative".into()));
        }
        let readings = self.readings_of(item_id);
        if let Some((_, before)) = readings.range(..period.clone()).next_back()
            && value < *before
        {
            return Err(Error::InvalidInput(format!(
                "the reading is below last time's ({before})"
            )));
        }
        if let Some((_, after)) = readings
            .range(period.clone()..)
            .find(|(key, _)| **key != period)
            && value > *after
        {
            return Err(Error::InvalidInput(format!(
                "the reading is above a later one ({after})"
            )));
        }
        self.doc
            .meter_readings
            .retain(|reading| !(reading.item_id == item_id && reading.period == period));
        let reading = MeterReading {
            id: uuid::Uuid::new_v4().to_string(),
            item_id: item_id.to_string(),
            value,
            period,
            recorded_at: now_iso(),
        };
        self.doc.meter_readings.push(reading.clone());
        self.persist()?;
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

    /// Readings of an item by period.
    fn readings_of(&self, item_id: &str) -> BTreeMap<String, Amount> {
        self.doc
            .meter_readings
            .iter()
            .filter(|reading| reading.item_id == item_id)
            .map(|reading| (reading.period.clone(), reading.value))
            .collect()
    }

    /// Payments made for an item — the ones «Записать расход» created carry
    /// its name as the comment and its category — by period, in its currency.
    fn paid_by_month(&self, item: &UtilityItem, cal: &Calendar) -> BTreeMap<String, Amount> {
        let book = self.rate_book();
        let mut out: BTreeMap<String, Amount> = BTreeMap::new();
        for tx in &self.doc.transactions {
            let Transaction::Payment(payment) = tx else {
                continue;
            };
            if payment.meta.comment.trim() != item.name
                || !payment.meta.categories.contains(&item.category)
                || item
                    .store_id
                    .as_ref()
                    .is_some_and(|id| *id != payment.store_id)
            {
                continue;
            }
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

    // -- trend --------------------------------------------------------------------

    /// Bills by month with the average, change, forecast and anomaly (plan §7.5).
    pub fn meter_trend(&self, item_id: &str, zone: &str) -> Result<UtilityTrend> {
        let item = self.utility_item(item_id)?.clone();
        let cal = Calendar::new(zone);
        let months: Vec<UtilityMonth> = match &item.kind {
            UtilityKind::Metered { tariff, .. } => {
                let readings = self.readings_of(&item.id);
                let mut previous: Option<Amount> = None;
                readings
                    .iter()
                    .map(|(period, value)| {
                        let consumption = previous.map(|before| *value - before);
                        previous = Some(*value);
                        UtilityMonth {
                            period: period.clone(),
                            reading: Some(*value),
                            consumption,
                            amount: consumption
                                .map_or(Amount::ZERO, |used| (used * *tariff).round(2)),
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
            UtilityKind::Metered { tariff, .. } => {
                let readings = self.readings_of(&item.id);
                let current = readings
                    .get(&period)
                    .copied()
                    .ok_or_else(|| Error::InvalidInput(format!("no meter reading for {period}")))?;
                let before = readings
                    .range(..period.clone())
                    .next_back()
                    .map(|(_, value)| *value);
                let used = current - before.unwrap_or(current);
                consumption = Some(used);
                (used * *tariff).round(2)
            }
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::store::StoreKind;
    use crate::service::test_support::*;

    fn metered(vault: &mut UnlockedVault) -> UtilityItem {
        let flat = store(vault, "Flat", StoreKind::Property, None);
        vault
            .create_utility_item(UtilityDraft {
                name: "Electricity".into(),
                active: true,
                kind: UtilityKind::Metered {
                    unit: "kWh".into(),
                    tariff: amount("5"),
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
            .add_meter_reading(&item.id, amount("1000"), "2026-01")
            .unwrap();
        vault
            .add_meter_reading(&item.id, amount("1150"), "2026-02")
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
                .add_meter_reading(&item.id, amount("900"), "2026-03")
                .is_err()
        );
        let trend = vault.meter_trend(&item.id, "UTC").unwrap();
        assert_eq!(trend.months[1].amount, amount("750"));
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
                .add_meter_reading(&item.id, amount(value), period)
                .unwrap();
        }
        let steady = vault.meter_trend(&item.id, "UTC").unwrap();
        assert_eq!(steady.forecast, Some(amount("700")));
        assert!(!steady.anomaly);
        assert_eq!(steady.delta_pct, Some(amount("8.3")));
        assert_eq!(steady.average, Some(amount("575")));

        vault
            .add_meter_reading(&item.id, amount("860"), "2026-06")
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
}
