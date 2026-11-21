//! The month's bill: the payment the form opens with for one item, the bill
//! of a whole property and its comparison with the months before.

use std::collections::BTreeMap;

use crate::Amount;
use crate::calendar::{self, Calendar, first_of_month};
use crate::error::{Error, Result};
use crate::model::VaultDoc;
use crate::model::planning::{UtilityItem, UtilityKind};
use crate::model::store::AssetRef;
use crate::model::transaction::{KindDetails, PaymentTx, PriceStamp, Transaction, TxMeta};
use crate::money::books::Books;

use super::items::utility_item;
use super::month::{
    MonthReadings, billable_month, metered_amount, monthly_readings, zone_months, zones_of,
};
use super::payments::paid_by_month;
use super::trend::{hundred, meter_trend};
use super::types::{TREND_MONTHS, UtilityBill, UtilityBillLine, UtilityPaymentDraft, ZoneBillLine};

/// Rent an automation pays: shown on the bill, never part of what is paid by hand.
fn paid_by_automation(item: &UtilityItem) -> bool {
    matches!(
        &item.kind,
        UtilityKind::Housing {
            automation_id: Some(_),
            ..
        }
    )
}

fn is_metered(item: &UtilityItem) -> bool {
    matches!(item.kind, UtilityKind::Metered { .. })
}

/// The "Pay" for one month of an item (plan §7.5): metered —
/// consumption × tariff; fixed — what was paid last time, else the set
/// amount; rent — the monthly amount plus the deposit with the first one.
/// `now` (unix seconds) decides which day of the month the payment is dated.
pub fn utility_payment_draft(
    books: &Books<'_>,
    item_id: &str,
    period: &str,
    zone: &str,
    now: i64,
) -> Result<UtilityPaymentDraft> {
    let item = utility_item(books.doc(), item_id)?.clone();
    let month = calendar::parse_month(period)
        .ok_or_else(|| Error::InvalidInput(format!("month {period}: expected YYYY-MM")))?;
    let period = calendar::month_key(month);
    let cal = Calendar::new(zone);
    let paid = paid_by_month(books, &item, &cal);
    let mut consumption = None;
    let mut includes_deposit = false;
    let amount = match &item.kind {
        UtilityKind::Metered { .. } => {
            match billable_month(books.doc(), &item.id, &period, &paid) {
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
            }
        }
        UtilityKind::Static { amount } => paid
            .range(..period.clone())
            .next_back()
            .map_or(*amount, |(_, last)| *last),
        UtilityKind::Housing {
            amount, deposit, ..
        } => {
            includes_deposit = deposit.is_some_and(|value| value.is_positive()) && paid.is_empty();
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
    let day = cal.date_of(now).clamp(first_of_month(month), last_day);
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

/// The month's bill of a property: every active item with its suggested
/// amount, meters with their previous reading (plan: "everything is paid on one day").
/// `now` (unix seconds) is today for the due day.
pub fn utility_bill(
    books: &Books<'_>,
    property_id: &str,
    period: &str,
    zone: &str,
    now: i64,
) -> Result<UtilityBill> {
    let doc = books.doc();
    let property = doc
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
    let items: Vec<UtilityItem> = doc
        .utility_items
        .iter()
        .filter(|item| item.property_id == property_id && item.active)
        .cloned()
        .collect();
    let mut lines = Vec::with_capacity(items.len());
    for item in items {
        let paid = paid_by_month(books, &item, &cal)
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
            estimate: None,
        };
        let mut unread = false;
        if let UtilityKind::Metered { .. } = item.kind {
            let paid_months = paid_by_month(books, &item, &cal);
            let billable = billable_month(books.doc(), &item.id, &period, &paid_months);
            line.zones = zone_lines(books.doc(), &item, &period, billable.as_ref());
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
                    line.previous_reading = monthly_readings(doc, &item.id)
                        .range(..period.clone())
                        .next_back()
                        .map(|(_, month)| month.end);
                }
            }
        }
        if !unread
            && let Ok(draft) = utility_payment_draft(books, &item.id, &period, zone, now)
            && let Transaction::Payment(payment) = &draft.transaction
        {
            line.amount = Some(payment.amount);
            line.consumption = draft.consumption;
            line.includes_deposit = draft.includes_deposit;
        }
        if unread && let Ok(trend) = meter_trend(books, &item.id, zone) {
            line.estimate = trend.forecast.filter(Amount::is_positive).or(trend.average);
        }
        lines.push(line);
    }
    let counted: Vec<&UtilityBillLine> = lines
        .iter()
        .filter(|line| !paid_by_automation(&line.item))
        .collect();
    let total = counted
        .iter()
        .filter_map(|line| line.amount.or(line.estimate))
        .fold(Amount::ZERO, |acc, value| acc + value);
    let unread_count = counted
        .iter()
        .filter(|line| is_metered(&line.item) && line.amount.is_none())
        .count();
    let average = bill_average(books, &counted, &period, zone);
    let delta_pct = average
        .filter(|mean| mean.is_positive() && total.is_positive())
        .map(|mean| ((total - mean) / mean * hundred()).round(0));
    let last_day = calendar::next_month(month) - chrono::Duration::days(1);
    let today = cal.date_of(now);
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
        total,
        estimated: unread_count > 0,
        unread: u32::try_from(unread_count).unwrap_or(u32::MAX),
        average,
        delta_pct,
    })
}

/// The average of the last six months before `period` that had a bill,
/// summing the given lines' items month by month.
pub fn bill_average(
    books: &Books<'_>,
    lines: &[&UtilityBillLine],
    period: &str,
    zone: &str,
) -> Option<Amount> {
    let mut by_month: BTreeMap<String, Amount> = BTreeMap::new();
    for line in lines {
        let Ok(trend) = meter_trend(books, &line.item.id, zone) else {
            continue;
        };
        for month in trend
            .months
            .iter()
            .filter(|month| month.period.as_str() < period)
        {
            let sum = by_month.entry(month.period.clone()).or_insert(Amount::ZERO);
            *sum += month.amount;
        }
    }
    let recent: Vec<Amount> = by_month
        .into_values()
        .filter(Amount::is_positive)
        .rev()
        .take(TREND_MONTHS)
        .collect();
    (!recent.is_empty()).then(|| {
        let sum = recent.iter().fold(Amount::ZERO, |acc, value| acc + *value);
        (sum / Amount::new(rust_decimal::Decimal::from(recent.len() as u64))).round(2)
    })
}

/// A multi-rate meter's zones on the bill: the month's readings when it is
/// read, else each zone's last reading before it as the base.
pub fn zone_lines(
    doc: &VaultDoc,
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
                    previous_reading: zone_months(doc, &item.id, Some(&zone))
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
