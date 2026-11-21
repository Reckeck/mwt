//! A meter's or an item's bills by month: the average, the change, the
//! forecast and the anomaly (plan §7.5).

use crate::Amount;
use crate::calendar::Calendar;
use crate::error::Result;
use crate::model::planning::UtilityKind;
use crate::money::books::Books;

use super::items::utility_item;
use super::month::{metered_amount, monthly_readings, out_of_order_ids, readings_of};
use super::payments::{paid_by_month, payments_by_month};
use super::types::{TREND_MONTHS, UtilityMonth, UtilityTrend, ZoneMonth};

pub(super) fn hundred() -> Amount {
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

/// Bills by month with the average, change, forecast and anomaly (plan §7.5).
pub fn meter_trend(books: &Books<'_>, item_id: &str, zone: &str) -> Result<UtilityTrend> {
    let doc = books.doc();
    let item = utility_item(doc, item_id)?.clone();
    let cal = Calendar::new(zone);
    let mut payments = payments_by_month(books, &item, &cal);
    let mut months: Vec<UtilityMonth> = match &item.kind {
        UtilityKind::Metered { .. } => {
            let readings = readings_of(&doc.meter_readings, &item.id);
            monthly_readings(doc, &item.id)
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
                        out_of_order: out_of_order_ids(&readings)
                            .into_iter()
                            .filter(|id| {
                                readings
                                    .iter()
                                    .any(|reading| &reading.id == id && reading.period == period)
                            })
                            .collect(),
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
        UtilityKind::Static { .. } | UtilityKind::Housing { .. } => {
            paid_by_month(books, &item, &cal)
                .into_iter()
                .map(|(period, amount)| UtilityMonth {
                    period,
                    amount: amount.round(2),
                    ..UtilityMonth::default()
                })
                .collect()
        }
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
