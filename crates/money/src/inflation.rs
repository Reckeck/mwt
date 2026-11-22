//! Inflation as the app uses it (plan I1): the fetched monthly series of a
//! currency, or the user's flat rate.
//!
//! The flat table (`flat_inflation`: the shipped rates under
//! `Settings.inflation_manual`), the series and each currency's mode all live
//! in the vault, so every synced device reads the same numbers. Everything
//! here reads the document; the scenarios that write it and the notices live
//! in `usecases::money::inflation`.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};

use crate::books::Books;
use mwt_base::Amount;
use mwt_base::calendar;
use mwt_base::error::{Error, Result};
use mwt_model::KeyRatePoint;
use mwt_model::VaultDoc;
use mwt_model::{InflationMode, InflationReading, InflationSource};

/// Months of series the settings screen draws.
pub const INFLATION_CHART_MONTHS: usize = 24;

/// A month the series did not have before: what the notice says.
#[derive(Debug, Clone, PartialEq)]
pub struct InflationNews {
    pub currency: String,
    pub month: String,
    pub yoy_pct: Amount,
    /// The reading of the month before, when there is one.
    pub previous: Option<Amount>,
}

/// One currency on the inflation settings screen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct InflationCurrency {
    pub currency: String,
    pub mode: InflationMode,
    /// The mode was chosen by the user rather than inferred.
    pub chosen: bool,
    /// The built-in source, if the currency has one; `Custom` in source mode.
    pub source: Option<InflationSource>,
    /// The user's data source of this currency (plan I2), even when another
    /// mode is chosen.
    pub source_id: Option<String>,
    pub latest: Option<InflationReading>,
    /// The reading of the month before `latest`.
    pub previous: Option<Amount>,
    /// The user's flat rate, percent.
    pub manual: Option<Amount>,
    /// What reports and the calculator use now.
    pub effective: Option<Amount>,
    /// The last [`INFLATION_CHART_MONTHS`] readings, oldest first.
    pub series: Vec<InflationReading>,
}

/// What a background fetch brought, for `apply_rates`.
#[derive(Debug, Clone, Default)]
pub struct FetchedRates {
    /// `None` when the key rate could not be fetched.
    pub key_rates: Option<Vec<KeyRatePoint>>,
    /// Readings of every source that answered.
    pub inflation: Vec<InflationReading>,
}

/// Reference annual inflation the user starts with, percent. Rough recent
/// figures; the settings screen says so and lets the user change them.
/// `appconfig::default_inflation` keeps the same table for the app config
/// (a test pins them equal).
const SHIPPED_INFLATION: [(&str, &str); 4] =
    [("rub", "8"), ("usd", "3"), ("eur", "2.5"), ("thb", "1")];

/// The shipped table, percent by currency.
pub fn shipped_inflation() -> BTreeMap<String, Amount> {
    SHIPPED_INFLATION
        .iter()
        .filter_map(|(currency, rate)| Some((currency.to_string(), Amount::parse(rate).ok()?)))
        .collect()
}

pub fn code(currency: &str) -> String {
    currency.trim().to_lowercase()
}

/// Readings of a currency, oldest first: the fetched ones, or in source
/// mode the ones read from the user's data source.
pub fn inflation_series(doc: &VaultDoc, currency: &str) -> Vec<InflationReading> {
    let currency = code(currency);
    if doc.settings.inflation_mode.get(&currency) == Some(&InflationMode::Source) {
        return source_inflation(doc, &currency);
    }
    let mut rows: Vec<InflationReading> = doc
        .inflation_points
        .iter()
        .filter(|row| row.currency == currency)
        .cloned()
        .collect();
    rows.sort_by(|left, right| left.month.cmp(&right.month));
    rows
}

/// Monthly readings from the currency's data source: a series whose
/// labels are dates gives a month per row; a number gives the latest
/// value of each month it was fetched in. They are derived each time,
/// not stored, so fixing the source fixes the history.
pub fn source_inflation(doc: &VaultDoc, currency: &str) -> Vec<InflationReading> {
    let Some(source) = doc
        .settings
        .inflation_source
        .get(currency)
        .and_then(|id| doc.data_sources.iter().find(|source| source.id == *id))
    else {
        return Vec::new();
    };
    let mut by_month: BTreeMap<String, Amount> = BTreeMap::new();
    let values = source.history.iter().chain(source.last.iter());
    for value in values {
        for point in &value.series {
            if let Some(date) = super::date_label::parse_date_label(&point.label) {
                by_month.insert(date.format("%Y-%m").to_string(), point.value);
            }
        }
        if let (Some(number), Some(month)) = (value.number, value.at.get(..7)) {
            by_month.insert(month.to_string(), number);
        }
    }
    by_month
        .into_iter()
        .map(|(month, yoy_pct)| InflationReading {
            currency: currency.to_string(),
            month,
            yoy_pct,
            source: InflationSource::Custom,
        })
        .collect()
}

/// The flat annual inflation of the group, percent by currency: the shipped
/// table with the vault's own rates over it. The only source of flat rates
/// (screens, reports, workflow nodes, the monthly checkup), so every device
/// of the group computes with one table.
pub fn flat_inflation(doc: &VaultDoc) -> BTreeMap<String, Amount> {
    let mut table = shipped_inflation();
    table.extend(doc.settings.inflation_manual.clone());
    table
}

/// The rates of an old device's table (`normalized`: lower-case codes, sane
/// values) worth copying into the vault: only values that differ from the
/// shipped table, and never over a rate the group already has.
pub fn flat_rates_to_adopt(
    doc: &VaultDoc,
    normalized: BTreeMap<String, Amount>,
) -> BTreeMap<String, Amount> {
    let shipped = shipped_inflation();
    normalized
        .into_iter()
        .filter(|(currency, rate)| {
            shipped.get(currency) != Some(rate)
                && !doc.settings.inflation_manual.contains_key(currency)
        })
        .collect()
}

/// The mode the user chose, else: no built-in source → manual; a flat rate
/// the user changed from the shipped default → manual (they meant it);
/// otherwise automatic.
pub fn inflation_mode(doc: &VaultDoc, currency: &str) -> InflationMode {
    let currency = code(currency);
    if let Some(mode) = doc.settings.inflation_mode.get(&currency) {
        return *mode;
    }
    if InflationSource::for_currency(&currency).is_none() {
        return InflationMode::Manual;
    }
    let shipped = shipped_inflation();
    match flat_inflation(doc).get(&currency) {
        Some(rate) if shipped.get(&currency) != Some(rate) => InflationMode::Manual,
        _ => InflationMode::Auto,
    }
}

/// Inflation of a currency in a month (`YYYY-MM`), percent: the reading of
/// that month or the latest one before it in auto mode, else the flat
/// rate. A month before the first reading takes the first one.
pub fn inflation_on(doc: &VaultDoc, currency: &str, month: &str) -> Option<Amount> {
    let currency = code(currency);
    let flat = flat_inflation(doc).get(&currency).copied();
    if inflation_mode(doc, &currency) == InflationMode::Manual {
        return flat;
    }
    let series = inflation_series(doc, &currency);
    series
        .iter()
        .rev()
        .find(|row| row.month.as_str() <= month)
        .or_else(|| series.first())
        .map(|row| row.yoy_pct)
        .or(flat)
}

/// Latest inflation per currency as reports and the calculator use it:
/// every currency with a flat rate or a built-in source.
pub fn effective_inflation(doc: &VaultDoc) -> BTreeMap<String, Amount> {
    inflation_currencies(doc)
        .into_iter()
        .filter_map(|currency| {
            let rate = inflation_on(doc, &currency, "9999-12")?;
            Some((currency, rate))
        })
        .collect()
}

pub fn inflation_currencies(doc: &VaultDoc) -> BTreeSet<String> {
    flat_inflation(doc)
        .into_keys()
        .chain(doc.settings.inflation_source.keys().cloned())
        .chain(
            InflationSource::ALL
                .iter()
                .map(|source| source.currency().to_string()),
        )
        .collect()
}

/// What one unit of each month from `from` to `today` is worth in
/// today's money, by the monthly series: the product of
/// `(1 + yoy)^(1/12)` over the months after it up to this one. `None`
/// when the currency has no series in use — the report then keeps its
/// flat rate. A month without a reading takes the latest one before it
/// (the current month is published only weeks later).
pub fn real_coefficients(
    doc: &VaultDoc,
    currency: &str,
    from: NaiveDate,
    today: NaiveDate,
) -> Option<BTreeMap<NaiveDate, Amount>> {
    if inflation_mode(doc, currency) == InflationMode::Manual {
        return None;
    }
    let series = inflation_series(doc, currency);
    if series.is_empty() {
        return None;
    }
    let one = Amount::from(1);
    let hundred = Amount::from(100);
    let mut roots: BTreeMap<Amount, Amount> = BTreeMap::new();
    let mut factor = |yoy: Amount| -> Amount {
        *roots
            .entry(yoy)
            .or_insert_with(|| (one + yoy / hundred).root_n(12))
    };
    let current = calendar::first_of_month(today);
    let mut month = calendar::first_of_month(from);
    let mut months = Vec::new();
    while month <= current {
        months.push(month);
        month = calendar::next_month(month);
    }
    let mut coefficients = BTreeMap::new();
    let mut running = one;
    for month in months.iter().rev() {
        coefficients.insert(*month, running);
        let key = format!("{:04}-{:02}", month.year(), month.month());
        let yoy = series
            .iter()
            .rev()
            .find(|row| row.month <= key)
            .or_else(|| series.first())
            .map(|row| row.yoy_pct)
            .unwrap_or_default();
        running = running * factor(yoy);
    }
    Some(coefficients)
}

/// Merge fetched readings into `known`: a revised month replaces the old
/// value. Returns the merged table and the months newer than what each
/// currency had (for the notice).
pub fn merge_readings(
    known: Vec<InflationReading>,
    fresh: &[InflationReading],
) -> (Vec<InflationReading>, Vec<InflationNews>) {
    let mut by_key: BTreeMap<(String, String), InflationReading> = known
        .into_iter()
        .map(|row| ((row.currency.clone(), row.month.clone()), row))
        .collect();
    let latest_before: BTreeMap<String, String> =
        by_key
            .keys()
            .fold(BTreeMap::new(), |mut acc, (currency, month)| {
                acc.insert(currency.clone(), month.clone());
                acc
            });
    for row in fresh {
        let row = InflationReading {
            currency: code(&row.currency),
            ..row.clone()
        };
        by_key.insert((row.currency.clone(), row.month.clone()), row);
    }
    let mut news = Vec::new();
    let currencies: BTreeSet<String> = by_key
        .keys()
        .map(|(currency, _)| currency.clone())
        .collect();
    for currency in currencies {
        let rows: Vec<&InflationReading> = by_key
            .range((currency.clone(), String::new())..)
            .take_while(|((owner, _), _)| *owner == currency)
            .map(|(_, row)| row)
            .collect();
        let Some(last) = rows.last() else { continue };
        let known = latest_before.get(&currency);
        if known.is_none_or(|month| last.month > *month) && known.is_some() {
            news.push(InflationNews {
                currency: currency.clone(),
                month: last.month.clone(),
                yoy_pct: last.yoy_pct,
                previous: rows.iter().rev().nth(1).map(|row| row.yoy_pct),
            });
        }
    }
    (by_key.into_values().collect(), news)
}

/// Check a mode choice before it is saved; returns the currency code.
/// `None` returns the currency to the inferred default.
pub fn plan_inflation_mode(
    doc: &VaultDoc,
    currency: &str,
    mode: Option<InflationMode>,
) -> Result<String> {
    let currency = code(currency);
    if currency.is_empty() {
        return Err(Error::InvalidInput("currency is empty".into()));
    }
    if mode == Some(InflationMode::Auto) && InflationSource::for_currency(&currency).is_none() {
        return Err(Error::InvalidInput(format!(
            "{currency}: no built-in inflation source"
        )));
    }
    if mode == Some(InflationMode::Source) && !doc.settings.inflation_source.contains_key(&currency)
    {
        return Err(Error::InvalidInput(format!(
            "{currency}: no data source chosen for inflation"
        )));
    }
    Ok(currency)
}

/// Check that the data source a currency's inflation would be read from
/// exists (read through `books`); returns the currency code.
pub fn plan_inflation_source(books: &Books<'_>, currency: &str, source_id: &str) -> Result<String> {
    let currency = code(currency);
    if currency.is_empty() {
        return Err(Error::InvalidInput("currency is empty".into()));
    }
    if !books
        .doc()
        .data_sources
        .iter()
        .any(|source| source.id == source_id)
    {
        return Err(Error::NotFound(format!("data source {source_id}")));
    }
    Ok(currency)
}

/// Every currency with a flat rate or a built-in source, for the settings
/// screen.
pub fn inflation_overview(doc: &VaultDoc) -> Vec<InflationCurrency> {
    let flat = flat_inflation(doc);
    inflation_currencies(doc)
        .into_iter()
        .map(|currency| {
            let series = inflation_series(doc, &currency);
            let latest = series.last().cloned();
            let mode = inflation_mode(doc, &currency);
            let previous = series.iter().rev().nth(1).map(|row| row.yoy_pct);
            let tail = series.len().saturating_sub(INFLATION_CHART_MONTHS);
            InflationCurrency {
                mode,
                chosen: doc.settings.inflation_mode.contains_key(&currency),
                source: if mode == InflationMode::Source {
                    Some(InflationSource::Custom)
                } else {
                    InflationSource::for_currency(&currency)
                },
                source_id: doc.settings.inflation_source.get(&currency).cloned(),
                latest,
                previous,
                manual: flat.get(&currency).copied(),
                effective: inflation_on(doc, &currency, "9999-12"),
                series: series[tail..].to_vec(),
                currency,
            }
        })
        .collect()
}
