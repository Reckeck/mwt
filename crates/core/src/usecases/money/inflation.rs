//! Scenarios of the inflation area: the fetch merge, the mode and source a
//! currency reads its inflation from, and the notices a fetch calls for.
//! Each decides in `money::inflation`, then ends in one `vault.transact`
//! (the merges that the refresh persists once at its end write nothing).

use std::collections::BTreeMap;

use crate::Amount;
use crate::error::Result;
use crate::model::deposit::KeyRatePoint;
use crate::model::inflation::{InflationMode, InflationReading};
use crate::model::notify::{NoticeCategory, NoticeRecord};
use crate::money::inflation::{
    FetchedRates, InflationNews, flat_rates_to_adopt, merge_readings, plan_inflation_mode,
    plan_inflation_source,
};
use crate::money::interest::merge_key_rates;
use crate::service::{UnlockedVault, notice, notice_title};

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
    notice(
        NoticeCategory::Rates,
        format!("rates:inflation:{}:{}", news.currency, news.month),
        notice_title(NoticeCategory::Rates, ru),
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
    notice(
        NoticeCategory::Rates,
        format!("rates:key:{date}"),
        notice_title(NoticeCategory::Rates, ru),
        body,
        None,
        crate::model::vault::now_iso(),
    )
}

/// The notices of the decisions in `table` that came after `known`.
fn key_rate_notices(known: &KeyRatePoint, table: &[KeyRatePoint], ru: bool) -> Vec<NoticeRecord> {
    let mut before = known.rate;
    let mut notices = Vec::new();
    for point in table.iter().filter(|point| point.date > known.date) {
        if point.rate != before {
            notices.push(key_rate_notice(&point.date, before, point.rate, ru));
        }
        before = point.rate;
    }
    notices
}

/// Copy the rates this device had in its app config into the vault, once
/// (`Settings.inflation_adopted`): only values that differ from the shipped
/// table, and never over a rate the group already has. Returns whether
/// anything was written. Two devices that adopt different values of one
/// currency settle on the later write when they sync.
pub fn adopt_flat_inflation(
    vault: &mut UnlockedVault,
    local: &BTreeMap<String, Amount>,
) -> Result<bool> {
    if vault.doc.settings.inflation_adopted {
        return Ok(false);
    }
    let adopted = flat_rates_to_adopt(&vault.doc, crate::appconfig::normalize_inflation(local));
    vault.transact(|tx| {
        let settings = &mut tx.doc_mut().settings;
        settings.inflation_manual.extend(adopted);
        settings.inflation_adopted = true;
        Ok(true)
    })
}

/// Merge fetched readings: a revised month replaces the old value. Returns
/// the months newer than what each currency had (for the notice). Does
/// not persist.
pub fn apply_inflation(
    vault: &mut UnlockedVault,
    fresh: &[InflationReading],
) -> Vec<InflationNews> {
    let known = std::mem::take(&mut vault.doc.inflation_points);
    let (merged, news) = merge_readings(known, fresh);
    vault.doc.inflation_points = merged;
    news
}

/// Merge a background fetch, persist, and return the notices it calls
/// for (not yet logged): new inflation months and key-rate decisions
/// that were not known before. The first fetch fills history silently.
pub fn apply_rates(
    vault: &mut UnlockedVault,
    fetched: &FetchedRates,
    ru: bool,
) -> Result<Vec<NoticeRecord>> {
    vault.transact(|tx| {
        let doc = tx.doc_mut();
        let mut notices = Vec::new();
        if let Some(points) = &fetched.key_rates {
            let known = doc.key_rates.last().cloned();
            doc.key_rates = merge_key_rates(&doc.key_rates, points);
            if let Some(known) = known {
                notices.extend(key_rate_notices(&known, &doc.key_rates, ru));
            }
        }
        let (merged, news) = merge_readings(
            std::mem::take(&mut doc.inflation_points),
            &fetched.inflation,
        );
        doc.inflation_points = merged;
        notices.extend(news.iter().map(|news| inflation_notice(news, ru)));
        Ok(notices)
    })
}

/// Choose where a currency's inflation comes from; `None` returns it to
/// the inferred default.
pub fn set_inflation_mode(
    vault: &mut UnlockedVault,
    currency: &str,
    mode: Option<InflationMode>,
) -> Result<()> {
    let currency = plan_inflation_mode(&vault.doc, currency, mode)?;
    vault.transact(|tx| {
        let modes = &mut tx.doc_mut().settings.inflation_mode;
        match mode {
            Some(mode) => modes.insert(currency, mode),
            None => modes.remove(&currency),
        };
        Ok(())
    })
}

/// Read a currency's inflation from one of the user's data sources.
pub fn set_inflation_source(
    vault: &mut UnlockedVault,
    currency: &str,
    source_id: &str,
) -> Result<()> {
    let currency = plan_inflation_source(&vault.books(), currency, source_id)?;
    vault.transact(|tx| {
        let settings = &mut tx.doc_mut().settings;
        settings
            .inflation_source
            .insert(currency.clone(), source_id.to_string());
        settings
            .inflation_mode
            .insert(currency, InflationMode::Source);
        Ok(())
    })
}
