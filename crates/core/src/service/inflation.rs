//! Inflation as the app uses it (plan I1): the fetched monthly series of a
//! currency, or the user's flat rate.
//!
//! The flat table (`flat_inflation`: the shipped rates under
//! `Settings.inflation_manual`), the series and each currency's mode all live
//! in the vault, so every synced device reads the same numbers.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};

use crate::Amount;
use crate::calendar;
use crate::error::{Error, Result};
use crate::model::deposit::KeyRatePoint;
use crate::model::inflation::{InflationMode, InflationReading, InflationSource};
use crate::model::notify::{NoticeCategory, NoticeRecord};

use super::UnlockedVault;

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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
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

/// What a background fetch brought, for [`UnlockedVault::apply_rates`].
#[derive(Debug, Clone, Default)]
pub struct FetchedRates {
    /// `None` when the key rate could not be fetched.
    pub key_rates: Option<Vec<KeyRatePoint>>,
    /// Readings of every source that answered.
    pub inflation: Vec<InflationReading>,
}

const MONTHS_RU: [&str; 12] = [
    "январе",
    "феврале",
    "марте",
    "апреле",
    "мае",
    "июне",
    "июле",
    "августе",
    "сентябре",
    "октябре",
    "ноябре",
    "декабре",
];
const MONTHS_EN: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// `YYYY-MM` → "in September" (Russian prepositional case) / "September".
fn month_name(month: &str, ru: bool) -> String {
    let index = month
        .get(5..7)
        .and_then(|raw| raw.parse::<usize>().ok())
        .filter(|number| (1..=12).contains(number));
    match index {
        Some(number) if ru => MONTHS_RU[number - 1].to_string(),
        Some(number) => MONTHS_EN[number - 1].to_string(),
        None => month.to_string(),
    }
}

/// «6.35» without trailing zeros.
fn pct_text(value: Amount) -> String {
    value.round(2).to_string()
}

fn signed(value: Amount) -> String {
    if value.is_negative() {
        pct_text(value)
    } else {
        format!("+{}", pct_text(value))
    }
}

/// The notice of a new inflation month.
pub fn inflation_notice(news: &InflationNews, ru: bool) -> NoticeRecord {
    let month = month_name(&news.month, ru);
    let currency = news.currency.to_uppercase();
    let change = news
        .previous
        .map(|previous| format!(" ({})", signed(news.yoy_pct - previous)))
        .unwrap_or_default();
    let body = if ru {
        format!(
            "Инфляция ({currency}) в {month} {}%{change}",
            pct_text(news.yoy_pct)
        )
    } else {
        format!(
            "Inflation ({currency}) in {month}: {}%{change}",
            pct_text(news.yoy_pct)
        )
    };
    super::notice(
        NoticeCategory::Rates,
        format!("rates:inflation:{}:{}", news.currency, news.month),
        super::notice_title(NoticeCategory::Rates, ru),
        body,
        Some(crate::routes::SETTINGS_INFLATION.into()),
        crate::model::vault::now_iso(),
    )
}

/// The notice of a key-rate decision taking effect on `date`.
pub fn key_rate_notice(date: &str, before: Amount, after: Amount, ru: bool) -> NoticeRecord {
    let body = if ru {
        format!(
            "ЦБ изменил ставку: {}% -> {}% с {date}",
            pct_text(before),
            pct_text(after)
        )
    } else {
        format!(
            "The Bank of Russia changed the key rate: {}% -> {}% from {date}",
            pct_text(before),
            pct_text(after)
        )
    };
    super::notice(
        NoticeCategory::Rates,
        format!("rates:key:{date}"),
        super::notice_title(NoticeCategory::Rates, ru),
        body,
        None,
        crate::model::vault::now_iso(),
    )
}

fn code(currency: &str) -> String {
    currency.trim().to_lowercase()
}

impl UnlockedVault {
    /// Readings of a currency, oldest first: the fetched ones, or in source
    /// mode the ones read from the user's data source.
    pub fn inflation_series(&self, currency: &str) -> Vec<InflationReading> {
        let currency = code(currency);
        if self.doc.settings.inflation_mode.get(&currency) == Some(&InflationMode::Source) {
            return self.source_inflation(&currency);
        }
        let mut rows: Vec<InflationReading> = self
            .doc
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
    fn source_inflation(&self, currency: &str) -> Vec<InflationReading> {
        let Some(source) = self
            .doc
            .settings
            .inflation_source
            .get(currency)
            .and_then(|id| self.doc.data_sources.iter().find(|source| source.id == *id))
        else {
            return Vec::new();
        };
        let mut by_month: BTreeMap<String, Amount> = BTreeMap::new();
        let values = source.history.iter().chain(source.last.iter());
        for value in values {
            for point in &value.series {
                if let Some(date) = crate::datasource::parse_date_label(&point.label) {
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
    pub fn flat_inflation(&self) -> BTreeMap<String, Amount> {
        let mut table = crate::appconfig::default_inflation();
        table.extend(self.doc.settings.inflation_manual.clone());
        table
    }

    /// Copy the rates this device had in its app config into the vault, once
    /// (`Settings.inflation_adopted`): only values that differ from the shipped
    /// table, and never over a rate the group already has. Returns whether
    /// anything was written. Two devices that adopt different values of one
    /// currency settle on the later write when they sync.
    pub fn adopt_flat_inflation(&mut self, local: &BTreeMap<String, Amount>) -> Result<bool> {
        if self.doc.settings.inflation_adopted {
            return Ok(false);
        }
        let shipped = crate::appconfig::default_inflation();
        for (currency, rate) in crate::appconfig::normalize_inflation(local) {
            if shipped.get(&currency) != Some(&rate) {
                self.doc
                    .settings
                    .inflation_manual
                    .entry(currency)
                    .or_insert(rate);
            }
        }
        self.doc.settings.inflation_adopted = true;
        self.persist()?;
        Ok(true)
    }

    /// The mode the user chose, else: no built-in source → manual; a flat rate
    /// the user changed from the shipped default → manual (they meant it);
    /// otherwise automatic.
    pub fn inflation_mode(&self, currency: &str) -> InflationMode {
        let currency = code(currency);
        if let Some(mode) = self.doc.settings.inflation_mode.get(&currency) {
            return *mode;
        }
        if InflationSource::for_currency(&currency).is_none() {
            return InflationMode::Manual;
        }
        let shipped = crate::appconfig::default_inflation();
        match self.flat_inflation().get(&currency) {
            Some(rate) if shipped.get(&currency) != Some(rate) => InflationMode::Manual,
            _ => InflationMode::Auto,
        }
    }

    /// Inflation of a currency in a month (`YYYY-MM`), percent: the reading of
    /// that month or the latest one before it in auto mode, else the flat
    /// rate. A month before the first reading takes the first one.
    pub fn inflation_on(&self, currency: &str, month: &str) -> Option<Amount> {
        let currency = code(currency);
        let flat = self.flat_inflation().get(&currency).copied();
        if self.inflation_mode(&currency) == InflationMode::Manual {
            return flat;
        }
        let series = self.inflation_series(&currency);
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
    pub fn effective_inflation(&self) -> BTreeMap<String, Amount> {
        self.inflation_currencies()
            .into_iter()
            .filter_map(|currency| {
                let rate = self.inflation_on(&currency, "9999-12")?;
                Some((currency, rate))
            })
            .collect()
    }

    fn inflation_currencies(&self) -> BTreeSet<String> {
        self.flat_inflation()
            .into_keys()
            .chain(self.doc.settings.inflation_source.keys().cloned())
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
    pub(crate) fn real_coefficients(
        &self,
        currency: &str,
        from: NaiveDate,
        today: NaiveDate,
    ) -> Option<BTreeMap<NaiveDate, Amount>> {
        if self.inflation_mode(currency) == InflationMode::Manual {
            return None;
        }
        let series = self.inflation_series(currency);
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

    /// Merge fetched readings: a revised month replaces the old value. Returns
    /// the months newer than what each currency had (for the notice). Does
    /// not persist.
    pub fn apply_inflation(&mut self, fresh: &[InflationReading]) -> Vec<InflationNews> {
        let mut by_key: BTreeMap<(String, String), InflationReading> = self
            .doc
            .inflation_points
            .drain(..)
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
        self.doc.inflation_points = by_key.into_values().collect();
        news
    }

    /// Merge a background fetch, persist, and return the notices it calls
    /// for (not yet logged): new inflation months and key-rate decisions
    /// that were not known before. The first fetch fills history silently.
    pub fn apply_rates(&mut self, fetched: &FetchedRates, ru: bool) -> Result<Vec<NoticeRecord>> {
        let mut notices = Vec::new();
        if let Some(points) = &fetched.key_rates {
            let known = self.doc.key_rates.last().cloned();
            self.apply_key_rates(points);
            if let Some(known) = known {
                let mut before = known.rate;
                for point in self
                    .doc
                    .key_rates
                    .iter()
                    .filter(|point| point.date > known.date)
                {
                    if point.rate != before {
                        notices.push(key_rate_notice(&point.date, before, point.rate, ru));
                    }
                    before = point.rate;
                }
            }
        }
        for news in self.apply_inflation(&fetched.inflation) {
            notices.push(inflation_notice(&news, ru));
        }
        self.persist()?;
        Ok(notices)
    }

    /// Choose where a currency's inflation comes from; `None` returns it to
    /// the inferred default.
    pub fn set_inflation_mode(
        &mut self,
        currency: &str,
        mode: Option<InflationMode>,
    ) -> Result<()> {
        let currency = code(currency);
        if currency.is_empty() {
            return Err(Error::InvalidInput("currency is empty".into()));
        }
        if mode == Some(InflationMode::Auto) && InflationSource::for_currency(&currency).is_none() {
            return Err(Error::InvalidInput(format!(
                "{currency}: no built-in inflation source"
            )));
        }
        if mode == Some(InflationMode::Source)
            && !self.doc.settings.inflation_source.contains_key(&currency)
        {
            return Err(Error::InvalidInput(format!(
                "{currency}: no data source chosen for inflation"
            )));
        }
        match mode {
            Some(mode) => self.doc.settings.inflation_mode.insert(currency, mode),
            None => self.doc.settings.inflation_mode.remove(&currency),
        };
        self.persist()
    }

    /// Read a currency's inflation from one of the user's data sources.
    pub fn set_inflation_source(&mut self, currency: &str, source_id: &str) -> Result<()> {
        let currency = code(currency);
        if currency.is_empty() {
            return Err(Error::InvalidInput("currency is empty".into()));
        }
        if self.data_source(source_id).is_none() {
            return Err(Error::NotFound(format!("data source {source_id}")));
        }
        self.doc
            .settings
            .inflation_source
            .insert(currency.clone(), source_id.to_string());
        self.doc
            .settings
            .inflation_mode
            .insert(currency, InflationMode::Source);
        self.persist()
    }

    /// Every currency with a flat rate or a built-in source, for the settings
    /// screen.
    pub fn inflation_overview(&self) -> Vec<InflationCurrency> {
        let flat = self.flat_inflation();
        self.inflation_currencies()
            .into_iter()
            .map(|currency| {
                let series = self.inflation_series(&currency);
                let latest = series.last().cloned();
                let mode = self.inflation_mode(&currency);
                let previous = series.iter().rev().nth(1).map(|row| row.yoy_pct);
                let tail = series.len().saturating_sub(INFLATION_CHART_MONTHS);
                InflationCurrency {
                    mode,
                    chosen: self.doc.settings.inflation_mode.contains_key(&currency),
                    source: if mode == InflationMode::Source {
                        Some(InflationSource::Custom)
                    } else {
                        InflationSource::for_currency(&currency)
                    },
                    source_id: self.doc.settings.inflation_source.get(&currency).cloned(),
                    latest,
                    previous,
                    manual: flat.get(&currency).copied(),
                    effective: self.inflation_on(&currency, "9999-12"),
                    series: series[tail..].to_vec(),
                    currency,
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::test_support::{amount, empty_vault};

    fn reading(currency: &str, month: &str, yoy: &str) -> InflationReading {
        InflationReading {
            currency: currency.into(),
            month: month.into(),
            yoy_pct: amount(yoy),
            source: InflationSource::for_currency(currency).unwrap_or(InflationSource::Cbr),
        }
    }

    fn day(raw: &str) -> NaiveDate {
        NaiveDate::parse_from_str(raw, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn mode_is_inferred_until_chosen() {
        let (_dir, mut vault) = empty_vault();
        assert_eq!(vault.inflation_mode("RUB"), InflationMode::Auto);
        // No built-in source for THB.
        assert_eq!(vault.inflation_mode("thb"), InflationMode::Manual);
        set_rates(&mut vault, &[("rub", "9.5")]);
        assert_eq!(vault.inflation_mode("rub"), InflationMode::Manual);
        vault
            .set_inflation_mode("rub", Some(InflationMode::Auto))
            .unwrap();
        assert_eq!(vault.inflation_mode("rub"), InflationMode::Auto);
        vault.set_inflation_mode("rub", None).unwrap();
        assert_eq!(vault.inflation_mode("rub"), InflationMode::Manual);
        assert!(
            vault
                .set_inflation_mode("thb", Some(InflationMode::Auto))
                .is_err()
        );
    }

    #[test]
    fn auto_reads_the_series_and_falls_back_to_the_flat_rate() {
        let (_dir, mut vault) = empty_vault();
        assert_eq!(vault.inflation_on("rub", "2026-09"), Some(amount("8")));
        vault.apply_inflation(&[
            reading("rub", "2026-07", "5.98"),
            reading("rub", "2026-09", "6.35"),
        ]);
        assert_eq!(vault.inflation_on("rub", "2026-09"), Some(amount("6.35")));
        // A month without a reading takes the one before it…
        assert_eq!(vault.inflation_on("rub", "2026-08"), Some(amount("5.98")));
        // …and a month before the series its first reading.
        assert_eq!(vault.inflation_on("rub", "2020-01"), Some(amount("5.98")));
        assert_eq!(
            vault.effective_inflation().get("rub"),
            Some(&amount("6.35"))
        );
        vault
            .set_inflation_mode("rub", Some(InflationMode::Manual))
            .unwrap();
        assert_eq!(vault.inflation_on("rub", "2026-09"), Some(amount("8")));
        assert_eq!(vault.effective_inflation().get("thb"), Some(&amount("1")));
    }

    #[test]
    fn apply_reports_only_new_months() {
        let (_dir, mut vault) = empty_vault();
        // The first fetch fills history: nothing to announce.
        let news = vault.apply_inflation(&[
            reading("rub", "2026-07", "5.98"),
            reading("rub", "2026-08", "6.33"),
        ]);
        assert!(news.is_empty());
        // A revision of a known month replaces it silently.
        let news = vault.apply_inflation(&[reading("rub", "2026-08", "6.30")]);
        assert!(news.is_empty());
        assert_eq!(vault.inflation_series("rub").len(), 2);
        assert_eq!(vault.inflation_series("rub")[1].yoy_pct, amount("6.30"));
        let news = vault.apply_inflation(&[reading("RUB", "2026-09", "6.35")]);
        assert_eq!(news.len(), 1);
        assert_eq!(news[0].month, "2026-09");
        assert_eq!(news[0].previous, Some(amount("6.30")));
        assert_eq!(vault.inflation_series("rub").len(), 3);
    }

    #[test]
    fn rates_notices_only_for_news() {
        let (_dir, mut vault) = empty_vault();
        let key = |date: &str, rate: &str| KeyRatePoint {
            date: date.into(),
            rate: amount(rate),
        };
        let first = FetchedRates {
            key_rates: Some(vec![key("2026-07-28", "14")]),
            inflation: vec![reading("rub", "2026-08", "6.33")],
        };
        assert!(vault.apply_rates(&first, true).unwrap().is_empty());
        let second = FetchedRates {
            key_rates: Some(vec![key("2026-07-28", "14"), key("2026-10-24", "13")]),
            inflation: vec![
                reading("rub", "2026-08", "6.33"),
                reading("rub", "2026-09", "6.35"),
            ],
        };
        let notices = vault.apply_rates(&second, true).unwrap();
        let bodies: Vec<&str> = notices.iter().map(|notice| notice.body.as_str()).collect();
        assert_eq!(
            bodies,
            [
                "ЦБ изменил ставку: 14% -> 13% с 2026-10-24",
                "Инфляция (RUB) в сентябре 6.35% (+0.02)",
            ]
        );
        assert!(
            notices
                .iter()
                .all(|notice| notice.category == NoticeCategory::Rates)
        );
        // Fetching the same again says nothing.
        assert!(vault.apply_rates(&second, false).unwrap().is_empty());
    }

    #[test]
    fn a_constant_series_matches_the_flat_rate() {
        let (_dir, mut vault) = empty_vault();
        let today = day("2026-10-21");
        let rows: Vec<InflationReading> = (1..=12)
            .map(|month| reading("rub", &format!("2025-{month:02}"), "8"))
            .collect();
        vault.apply_inflation(&rows);
        let table = vault
            .real_coefficients("rub", day("2024-10-01"), today)
            .unwrap();
        assert_eq!(table[&day("2026-10-01")], Amount::from(1));
        for (month, coefficient) in &table {
            let elapsed =
                (today.year() - month.year()) * 12 + today.month() as i32 - month.month() as i32;
            let flat = Amount::from_f64(1.08_f64.powf(f64::from(elapsed) / 12.0));
            assert!(
                (*coefficient - flat).abs() < amount("0.01"),
                "{month}: {coefficient} vs {flat}"
            );
        }
        assert!((table[&day("2025-10-01")] - amount("1.08")).abs() < amount("0.0001"));
    }

    #[test]
    fn coefficients_follow_the_months() {
        let (_dir, mut vault) = empty_vault();
        vault.apply_inflation(&[
            reading("rub", "2026-08", "12"),
            reading("rub", "2026-09", "0"),
        ]);
        let table = vault
            .real_coefficients("rub", day("2026-08-01"), day("2026-10-05"))
            .unwrap();
        // September at 0 % and October carrying it: September's money kept its value.
        assert_eq!(table[&day("2026-09-01")], Amount::from(1));
        assert_eq!(table[&day("2026-08-01")], Amount::from(1));
        let table = vault
            .real_coefficients("rub", day("2026-07-01"), day("2026-09-05"))
            .unwrap();
        // August at 12 %: one month of it on July's money.
        let month_of_twelve = amount("1.12").root_n(12);
        assert_eq!(table[&day("2026-07-01")], month_of_twelve);
        assert!(
            vault
                .real_coefficients("thb", day("2026-07-01"), day("2026-09-05"))
                .is_none()
        );
    }

    fn table(pairs: &[(&str, &str)]) -> BTreeMap<String, Amount> {
        pairs
            .iter()
            .map(|(code, rate)| ((*code).to_string(), amount(rate)))
            .collect()
    }

    fn set_rates(vault: &mut UnlockedVault, pairs: &[(&str, &str)]) {
        vault
            .update_settings(crate::ipc_contract::UpdateSettingsInput {
                inflation_manual: Some(table(pairs)),
                ..Default::default()
            })
            .unwrap();
    }

    #[test]
    fn the_flat_table_is_the_shipped_one_under_the_vaults_own_rates() {
        let (_dir, mut vault) = empty_vault();
        assert_eq!(
            vault.flat_inflation(),
            crate::appconfig::default_inflation()
        );
        set_rates(&mut vault, &[("rub", "9.5"), ("gbp", "4")]);
        let flat = vault.flat_inflation();
        assert_eq!(flat.get("rub"), Some(&amount("9.5")));
        assert_eq!(flat.get("gbp"), Some(&amount("4")));
        assert_eq!(flat.get("usd"), Some(&amount("3")));
        assert_eq!(flat.get("thb"), Some(&amount("1")));
    }

    #[test]
    fn a_saved_table_keeps_only_what_differs_or_was_already_set() {
        let (_dir, mut vault) = empty_vault();
        // The form sends the whole table; the shipped values in it are not data.
        set_rates(&mut vault, &[("rub", "8"), ("usd", "3.5"), ("GBP", "4")]);
        assert_eq!(
            vault.doc.settings.inflation_manual,
            table(&[("usd", "3.5"), ("gbp", "4")])
        );
        // A value set once stays even when it is put back to the shipped one.
        set_rates(&mut vault, &[("usd", "3"), ("gbp", "4")]);
        assert_eq!(
            vault.doc.settings.inflation_manual,
            table(&[("usd", "3"), ("gbp", "4")])
        );
        // A rate that is not a number of percent is dropped.
        set_rates(&mut vault, &[("gbp", "4"), ("chf", "5000")]);
        assert_eq!(vault.doc.settings.inflation_manual, table(&[("gbp", "4")]));
    }

    #[test]
    fn old_device_rates_are_adopted_once_and_never_overwrite() {
        let (_dir, mut vault) = empty_vault();
        set_rates(&mut vault, &[("gbp", "4")]);
        let local = table(&[("rub", "8"), ("usd", "5"), ("gbp", "9"), ("thb", "1")]);
        assert!(vault.adopt_flat_inflation(&local).unwrap());
        assert!(vault.doc.settings.inflation_adopted);
        // Only the differing, not yet set values moved over.
        assert_eq!(
            vault.doc.settings.inflation_manual,
            table(&[("usd", "5"), ("gbp", "4")])
        );
        // Once: a later change of the old table is not picked up again.
        let again = table(&[("usd", "6"), ("eur", "7")]);
        assert!(!vault.adopt_flat_inflation(&again).unwrap());
        assert_eq!(
            vault.doc.settings.inflation_manual,
            table(&[("usd", "5"), ("gbp", "4")])
        );
    }

    #[test]
    fn adoption_survives_a_reopen_and_does_not_write_twice() {
        let (_dir, mut vault) = empty_vault();
        let local = table(&[("rub", "10")]);
        assert!(vault.adopt_flat_inflation(&local).unwrap());
        let reopened = UnlockedVault::open(vault.path.clone(), "pw").unwrap();
        assert!(reopened.doc.settings.inflation_adopted);
        assert_eq!(reopened.flat_inflation().get("rub"), Some(&amount("10")));
    }
}
