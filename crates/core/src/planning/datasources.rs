//! The user's data sources (plan I2): saving, where they are used, applying
//! what a background fetch brought. Fetching is in `pricing::fetch_page`,
//! extracting in `crate::datasource`.

use serde::{Deserialize, Serialize};

use crate::Amount;
use crate::datasource::Extracted;
use crate::error::{Error, Result};
use crate::model::datasource::{
    AssetPriceSource, BROKEN_AFTER, DataSource, DepositRateSource, JUMP_FACTOR, MAX_SOURCE_HISTORY,
    NumberFormat, SourceInterval, SourceMethod, SourceOutput, SourceStatus, SourceValue,
};
use crate::model::inflation::InflationMode;
use crate::model::notify::{NoticeCategory, NoticeRecord};
use crate::model::vault::{PricePoint, VaultDoc};
use crate::planning::notifications::{notice_at, notice_title};

/// Where a source's value goes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SourceUse {
    /// The currency's inflation, `InflationMode::Source`.
    Inflation { currency: String },
    /// The price of one unit of an asset, in `currency`.
    AssetPrice { asset_key: String, currency: String },
    /// A deposit's rate: a change is announced, not applied.
    DepositRate { store_id: String },
    /// A workflow node reads it; set in the editor, not here.
    Workflow { workflow_id: String, name: String },
}

/// A source with the places that use it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct DataSourceView {
    pub source: DataSource,
    pub used_by: Vec<SourceUse>,
}

/// What the form sends. `uses` replaces the inflation, price and deposit uses
/// of the source; workflow uses are ignored.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct DataSourceDraft {
    #[serde(default)]
    pub id: Option<String>,
    pub name: String,
    pub url: String,
    pub method: SourceMethod,
    pub output: SourceOutput,
    #[serde(default)]
    pub number: NumberFormat,
    pub interval: SourceInterval,
    #[serde(default)]
    pub uses: Vec<SourceUse>,
}

/// A source is checked in the background this often at most, whatever its
/// interval says.
pub fn is_due(source: &DataSource, now: chrono::DateTime<chrono::Utc>) -> bool {
    let Some(checked) = source
        .checked_at
        .as_deref()
        .and_then(|at| chrono::DateTime::parse_from_rfc3339(at).ok())
    else {
        return true;
    };
    (now - checked.with_timezone(&chrono::Utc)).num_seconds() >= source.interval.seconds()
}

fn invalid(message: impl Into<String>) -> Error {
    Error::InvalidInput(message.into())
}

pub fn check_draft(draft: &DataSourceDraft) -> Result<()> {
    if draft.name.trim().is_empty() {
        return Err(invalid("name: the source needs a name"));
    }
    let url = draft.url.trim().to_lowercase();
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(invalid("url: the address starts with https://"));
    }
    let empty = match &draft.method {
        SourceMethod::Css { selector, .. } => selector.trim().is_empty(),
        SourceMethod::Table {
            table_selector,
            column_header,
            ..
        } => table_selector.trim().is_empty() || column_header.trim().is_empty(),
        SourceMethod::Json { path } => path.trim().is_empty(),
        SourceMethod::Label { label } => label.trim().is_empty(),
        SourceMethod::File { column, .. } => column.trim().is_empty(),
    };
    if empty {
        return Err(invalid("method: say where the value is"));
    }
    if draft.number.multiplier.is_zero() {
        return Err(invalid("multiplier: cannot be zero"));
    }
    for usage in &draft.uses {
        let needs_number = !matches!(
            usage,
            SourceUse::Workflow { .. } | SourceUse::Inflation { .. }
        );
        if needs_number && draft.output != SourceOutput::Number {
            return Err(invalid("uses: a price or a rate needs a number"));
        }
        if matches!(usage, SourceUse::Inflation { .. }) && draft.output == SourceOutput::Text {
            return Err(invalid("uses: inflation needs a number or a series"));
        }
    }
    Ok(())
}

/// The new number is [`JUMP_FACTOR`] times larger or smaller than the last.
fn jumped(before: Amount, after: Amount) -> bool {
    if before.is_zero() || after.is_zero() {
        return false;
    }
    let ratio = (after / before).abs();
    let factor = Amount::from(JUMP_FACTOR);
    ratio > factor || ratio * factor < Amount::from(1u32)
}

fn pct_text(value: Amount) -> String {
    value.round(2).to_string()
}

/// The source with this id.
pub fn data_source<'doc>(doc: &'doc VaultDoc, id: &str) -> Option<&'doc DataSource> {
    doc.data_sources.iter().find(|source| source.id == id)
}

/// The error of a source id nobody has.
pub fn source_not_found(id: &str) -> Error {
    Error::NotFound(format!("data source {id}"))
}

/// Sources whose interval has passed since the last attempt.
pub fn due_data_sources(doc: &VaultDoc, now: chrono::DateTime<chrono::Utc>) -> Vec<DataSource> {
    doc.data_sources
        .iter()
        .filter(|source| is_due(source, now))
        .cloned()
        .collect()
}

/// The places of a source that the settings and links name; the workflow
/// nodes that read it are added by the scenario (it reads their graphs).
pub fn linked_uses(doc: &VaultDoc, id: &str) -> Vec<SourceUse> {
    let mut uses: Vec<SourceUse> = doc
        .settings
        .inflation_source
        .iter()
        .filter(|(_, source)| *source == id)
        .map(|(currency, _)| SourceUse::Inflation {
            currency: currency.clone(),
        })
        .collect();
    uses.extend(
        doc.asset_price_sources
            .iter()
            .filter(|link| link.source_id == id)
            .map(|link| SourceUse::AssetPrice {
                asset_key: link.asset_key.clone(),
                currency: link.currency.clone(),
            }),
    );
    uses.extend(
        doc.deposit_rate_sources
            .iter()
            .filter(|link| link.source_id == id)
            .map(|link| SourceUse::DepositRate {
                store_id: link.store_id.clone(),
            }),
    );
    uses
}

/// A new source of a checked draft.
pub fn new_source(draft: &DataSourceDraft, id: String, created_at: String) -> DataSource {
    DataSource {
        id,
        name: draft.name.trim().to_string(),
        url: draft.url.trim().to_string(),
        method: draft.method.clone(),
        output: draft.output,
        number: draft.number.clone(),
        interval: draft.interval,
        last: None,
        history: Vec::new(),
        pending: None,
        failures: 0,
        status: SourceStatus::Ok,
        error: None,
        checked_at: None,
        created_at,
    }
}

/// Change a source to a checked draft. A changed address or method starts
/// the count of failures over; the history stays.
pub fn edit_source(source: &mut DataSource, draft: &DataSourceDraft) {
    let moved = source.url != draft.url.trim() || source.method != draft.method;
    source.name = draft.name.trim().to_string();
    source.url = draft.url.trim().to_string();
    source.method = draft.method.clone();
    source.output = draft.output;
    source.number = draft.number.clone();
    source.interval = draft.interval;
    if moved {
        source.failures = 0;
        source.status = SourceStatus::Ok;
        source.error = None;
        source.pending = None;
        source.checked_at = None;
    }
}

fn drop_source_inflation(doc: &mut VaultDoc, currencies: Vec<String>) {
    for currency in currencies {
        doc.settings.inflation_source.remove(&currency);
        if doc.settings.inflation_mode.get(&currency) == Some(&InflationMode::Source) {
            doc.settings.inflation_mode.remove(&currency);
        }
    }
}

/// Drop the inflation choices that point at a source that no longer
/// exists (deleted on another device while this one chose it). Callers
/// persist.
pub fn prune_inflation_sources(doc: &mut VaultDoc) {
    let dangling: Vec<String> = doc
        .settings
        .inflation_source
        .iter()
        .filter(|(_, source)| data_source(doc, source).is_none())
        .map(|(currency, _)| currency.clone())
        .collect();
    drop_source_inflation(doc, dangling);
}

/// Forget every use of a source: the inflation choices, the price and the
/// deposit-rate links.
pub fn clear_source_uses(doc: &mut VaultDoc, id: &str) {
    let currencies: Vec<String> = doc
        .settings
        .inflation_source
        .iter()
        .filter(|(_, source)| *source == id)
        .map(|(currency, _)| currency.clone())
        .collect();
    drop_source_inflation(doc, currencies);
    doc.asset_price_sources.retain(|link| link.source_id != id);
    doc.deposit_rate_sources.retain(|link| link.source_id != id);
}

/// One source per currency, asset and deposit: a new use takes the place
/// over from another source.
pub fn set_source_uses(doc: &mut VaultDoc, id: &str, uses: &[SourceUse]) {
    clear_source_uses(doc, id);
    for usage in uses {
        match usage {
            SourceUse::Inflation { currency } => {
                let currency = currency.trim().to_lowercase();
                doc.settings
                    .inflation_source
                    .insert(currency.clone(), id.to_string());
                doc.settings
                    .inflation_mode
                    .insert(currency, InflationMode::Source);
            }
            SourceUse::AssetPrice {
                asset_key,
                currency,
            } => {
                doc.asset_price_sources
                    .retain(|link| link.asset_key != *asset_key);
                doc.asset_price_sources.push(AssetPriceSource {
                    asset_key: asset_key.clone(),
                    source_id: id.to_string(),
                    currency: currency.trim().to_lowercase(),
                });
            }
            SourceUse::DepositRate { store_id } => {
                doc.deposit_rate_sources
                    .retain(|link| link.store_id != *store_id);
                doc.deposit_rate_sources.push(DepositRateSource {
                    store_id: store_id.clone(),
                    source_id: id.to_string(),
                });
            }
            SourceUse::Workflow { .. } => {}
        }
    }
}

/// What a fetch did to a source besides changing it: the notices it
/// raised, and whether the new value goes on to its uses.
pub struct FetchOutcome {
    pub notices: Vec<NoticeRecord>,
    pub spread: bool,
}

/// Record a fetch made at `at`. An error keeps the value and counts a
/// failure ([`BROKEN_AFTER`] in a row mark the source broken, with a
/// notice); a number that jumped [`JUMP_FACTOR`] times waits for
/// confirmation; any other value becomes the latest and is to be spread.
pub fn record_fetch(
    source: &mut DataSource,
    fetched: std::result::Result<Extracted, String>,
    at: &str,
    ru: bool,
) -> FetchOutcome {
    source.checked_at = Some(at.to_string());
    let mut outcome = FetchOutcome {
        notices: Vec::new(),
        spread: false,
    };
    match fetched {
        Err(message) => {
            source.failures = source.failures.saturating_add(1);
            source.error = Some(message);
            if source.failures >= BROKEN_AFTER && source.status != SourceStatus::Broken {
                source.status = SourceStatus::Broken;
                outcome.notices.push(broken_notice(source, at, ru));
            }
        }
        Ok(extracted) => {
            source.failures = 0;
            source.error = extracted.warning.clone();
            let value = SourceValue {
                at: at.to_string(),
                number: extracted.number,
                text: extracted.text,
                series: extracted.series,
            };
            let before = source.last.as_ref().and_then(|last| last.number);
            if let (Some(before), Some(after)) = (before, value.number)
                && jumped(before, after)
            {
                source.status = SourceStatus::Pending;
                source.pending = Some(value);
                outcome
                    .notices
                    .push(pending_notice(source, before, after, at, ru));
            } else {
                push_value(source, value);
                outcome.spread = true;
            }
        }
    }
    outcome
}

/// Take or drop the value waiting after a jump; `true` when it was taken and
/// is to be spread.
pub fn settle_pending(source: &mut DataSource, accept: bool) -> Result<bool> {
    let Some(value) = source.pending.take() else {
        return Err(invalid("nothing waits for confirmation"));
    };
    source.status = SourceStatus::Ok;
    if accept {
        push_value(source, value);
    }
    Ok(accept)
}

/// Where the latest number of a source goes: prices for the assets it sets
/// (the ones `per_usd` can convert) and notices comparing it with the
/// deposits it follows.
#[derive(Default)]
pub struct Spread {
    pub points: Vec<PricePoint>,
    pub notices: Vec<NoticeRecord>,
}

/// Send the latest number to the prices it sets and compare it with the
/// deposits it follows. `per_usd` gives the units of a currency per USD.
pub fn spread_value(
    doc: &VaultDoc,
    source: &DataSource,
    per_usd: impl Fn(&str) -> Option<Amount>,
    ru: bool,
    delivered_at: &str,
) -> Spread {
    let id = source.id.as_str();
    let Some(value) = source.last.clone() else {
        return Spread::default();
    };
    let Some(number) = value.number else {
        return Spread::default();
    };
    let points: Vec<PricePoint> = doc
        .asset_price_sources
        .iter()
        .filter(|link| link.source_id == id)
        .filter_map(|link| {
            let rate = per_usd(&link.currency).filter(|rate| !rate.is_zero())?;
            Some(PricePoint {
                asset_key: link.asset_key.clone(),
                value_usd: number / rate,
                ts: value.at.clone(),
                source: format!("custom:{id}"),
                change_24h_pct: None,
            })
        })
        .collect();
    let today = value.at.get(..10).unwrap_or_default().to_string();
    let mut notices = Vec::new();
    for link in doc
        .deposit_rate_sources
        .iter()
        .filter(|link| link.source_id == id)
    {
        let Some(store) = doc.stores.iter().find(|store| store.id == link.store_id) else {
            continue;
        };
        let current = store
            .deposit
            .as_ref()
            .and_then(|terms| terms.rate_on(&today));
        if current == Some(number) {
            continue;
        }
        let was = current.map_or_else(|| "—".to_string(), pct_text);
        let body = if ru {
            format!(
                "Ставка по «{}» на сайте: {}%, в счёте {was}%. Обновите условия вклада, если она уже действует.",
                store.name,
                pct_text(number)
            )
        } else {
            format!(
                "The rate of «{}» on the site: {}%, in the account {was}%. Update the deposit terms if it is in force.",
                store.name,
                pct_text(number)
            )
        };
        notices.push(notice_at(
            NoticeCategory::Rates,
            format!("source:{id}:deposit:{}:{}", store.id, pct_text(number)),
            notice_title(NoticeCategory::Rates, ru),
            body,
            Some(format!("open:store:{}", store.id)),
            value.at.clone(),
            delivered_at.to_string(),
        ));
    }
    Spread { points, notices }
}

fn push_value(source: &mut DataSource, value: SourceValue) {
    if let Some(last) = source.last.take() {
        source.history.push(last);
        let extra = source.history.len().saturating_sub(MAX_SOURCE_HISTORY);
        source.history.drain(..extra);
    }
    source.last = Some(value);
    source.status = SourceStatus::Ok;
    source.pending = None;
}

fn broken_notice(source: &DataSource, at: &str, ru: bool) -> NoticeRecord {
    let body = if ru {
        format!(
            "Источник «{}» не обновляется: {} попытки подряд без значения. Откройте его и нажмите «Починить».",
            source.name, source.failures
        )
    } else {
        format!(
            "Source «{}» stopped updating: {} attempts in a row brought nothing. Open it and press «Fix».",
            source.name, source.failures
        )
    };
    notice_at(
        NoticeCategory::Rates,
        format!("source:{}:broken:{at}", source.id),
        notice_title(NoticeCategory::Rates, ru),
        body,
        Some(crate::routes::AUTOMATIONS_SOURCES.into()),
        at.to_string(),
        at.to_string(),
    )
}

fn pending_notice(
    source: &DataSource,
    before: Amount,
    after: Amount,
    at: &str,
    ru: bool,
) -> NoticeRecord {
    let body = if ru {
        format!(
            "Источник «{}»: значение изменилось с {before} на {after}. Проверьте и подтвердите — до этого используется прежнее.",
            source.name
        )
    } else {
        format!(
            "Source «{}»: the value went from {before} to {after}. Check and confirm it; until then the old one is used.",
            source.name
        )
    };
    notice_at(
        NoticeCategory::Rates,
        format!("source:{}:pending:{at}", source.id),
        notice_title(NoticeCategory::Rates, ru),
        body,
        Some(crate::routes::AUTOMATIONS_SOURCES.into()),
        at.to_string(),
        at.to_string(),
    )
}
