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
use crate::model::vault::{PricePoint, now_iso};
use crate::workflow::graph::Graph;

use super::UnlockedVault;

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

fn check_draft(draft: &DataSourceDraft) -> Result<()> {
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

impl UnlockedVault {
    pub fn data_source(&self, id: &str) -> Option<&DataSource> {
        self.doc.data_sources.iter().find(|source| source.id == id)
    }

    fn data_source_mut(&mut self, id: &str) -> Result<&mut DataSource> {
        self.doc
            .data_sources
            .iter_mut()
            .find(|source| source.id == id)
            .ok_or_else(|| Error::NotFound(format!("data source {id}")))
    }

    /// Every place that reads the source.
    pub fn source_uses(&self, id: &str) -> Vec<SourceUse> {
        let mut uses: Vec<SourceUse> = self
            .doc
            .settings
            .inflation_source
            .iter()
            .filter(|(_, source)| *source == id)
            .map(|(currency, _)| SourceUse::Inflation {
                currency: currency.clone(),
            })
            .collect();
        uses.extend(
            self.doc
                .asset_price_sources
                .iter()
                .filter(|link| link.source_id == id)
                .map(|link| SourceUse::AssetPrice {
                    asset_key: link.asset_key.clone(),
                    currency: link.currency.clone(),
                }),
        );
        uses.extend(
            self.doc
                .deposit_rate_sources
                .iter()
                .filter(|link| link.source_id == id)
                .map(|link| SourceUse::DepositRate {
                    store_id: link.store_id.clone(),
                }),
        );
        for workflow in &self.doc.workflows {
            let Ok(graph) = Graph::parse(&workflow.graph) else {
                continue;
            };
            let reads = graph.nodes.iter().any(|node| {
                node.kind.ends_with(".source")
                    && node
                        .params
                        .get("source")
                        .and_then(serde_json::Value::as_str)
                        == Some(id)
            });
            if reads {
                uses.push(SourceUse::Workflow {
                    workflow_id: workflow.id.clone(),
                    name: workflow.name.clone(),
                });
            }
        }
        uses
    }

    pub fn data_sources_list(&self) -> Vec<DataSourceView> {
        let mut out: Vec<DataSourceView> = self
            .doc
            .data_sources
            .iter()
            .map(|source| DataSourceView {
                source: source.clone(),
                used_by: self.source_uses(&source.id),
            })
            .collect();
        out.sort_by_key(|view| view.source.name.to_lowercase());
        out
    }

    /// Create or change a source. A changed address or method starts the
    /// count of failures over; the history stays.
    pub fn save_data_source(&mut self, draft: DataSourceDraft) -> Result<DataSourceView> {
        check_draft(&draft)?;
        let id = match &draft.id {
            Some(id) => {
                let source = self.data_source_mut(id)?;
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
                id.clone()
            }
            None => {
                let id = uuid::Uuid::new_v4().to_string();
                self.doc.data_sources.push(DataSource {
                    id: id.clone(),
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
                    created_at: now_iso(),
                });
                id
            }
        };
        self.set_source_uses(&id, &draft.uses);
        self.persist()?;
        let source = self
            .data_source(&id)
            .cloned()
            .unwrap_or_else(|| unreachable!());
        Ok(DataSourceView {
            used_by: self.source_uses(&id),
            source,
        })
    }

    fn clear_source_uses(&mut self, id: &str) {
        let currencies: Vec<String> = self
            .doc
            .settings
            .inflation_source
            .iter()
            .filter(|(_, source)| *source == id)
            .map(|(currency, _)| currency.clone())
            .collect();
        for currency in currencies {
            self.doc.settings.inflation_source.remove(&currency);
            if self.doc.settings.inflation_mode.get(&currency) == Some(&InflationMode::Source) {
                self.doc.settings.inflation_mode.remove(&currency);
            }
        }
        self.doc
            .asset_price_sources
            .retain(|link| link.source_id != id);
        self.doc
            .deposit_rate_sources
            .retain(|link| link.source_id != id);
    }

    /// One source per currency, asset and deposit: a new use takes the place
    /// over from another source.
    fn set_source_uses(&mut self, id: &str, uses: &[SourceUse]) {
        self.clear_source_uses(id);
        for usage in uses {
            match usage {
                SourceUse::Inflation { currency } => {
                    let currency = currency.trim().to_lowercase();
                    self.doc
                        .settings
                        .inflation_source
                        .insert(currency.clone(), id.to_string());
                    self.doc
                        .settings
                        .inflation_mode
                        .insert(currency, InflationMode::Source);
                }
                SourceUse::AssetPrice {
                    asset_key,
                    currency,
                } => {
                    self.doc
                        .asset_price_sources
                        .retain(|link| link.asset_key != *asset_key);
                    self.doc.asset_price_sources.push(AssetPriceSource {
                        asset_key: asset_key.clone(),
                        source_id: id.to_string(),
                        currency: currency.trim().to_lowercase(),
                    });
                }
                SourceUse::DepositRate { store_id } => {
                    self.doc
                        .deposit_rate_sources
                        .retain(|link| link.store_id != *store_id);
                    self.doc.deposit_rate_sources.push(DepositRateSource {
                        store_id: store_id.clone(),
                        source_id: id.to_string(),
                    });
                }
                SourceUse::Workflow { .. } => {}
            }
        }
    }

    /// Delete a source and every use of it; inflation in source mode goes
    /// back to its default.
    pub fn delete_data_source(&mut self, id: &str) -> Result<()> {
        self.data_source_mut(id)?;
        self.clear_source_uses(id);
        self.doc.data_sources.retain(|source| source.id != id);
        self.persist()
    }

    /// Sources whose interval has passed since the last attempt.
    pub fn due_data_sources(&self, now: chrono::DateTime<chrono::Utc>) -> Vec<DataSource> {
        self.doc
            .data_sources
            .iter()
            .filter(|source| is_due(source, now))
            .cloned()
            .collect()
    }

    /// Record a fetch. An error keeps the value and counts a failure
    /// ([`BROKEN_AFTER`] in a row mark the source broken, with a notice); a
    /// number that jumped [`JUMP_FACTOR`] times waits for confirmation; any
    /// other value becomes the latest and goes to its uses.
    pub fn apply_source_fetch(
        &mut self,
        id: &str,
        fetched: std::result::Result<Extracted, String>,
        ru: bool,
    ) -> Result<Vec<NoticeRecord>> {
        let at = now_iso();
        let source = self.data_source_mut(id)?;
        source.checked_at = Some(at.clone());
        let mut notices = Vec::new();
        match fetched {
            Err(message) => {
                source.failures = source.failures.saturating_add(1);
                source.error = Some(message);
                if source.failures >= BROKEN_AFTER && source.status != SourceStatus::Broken {
                    source.status = SourceStatus::Broken;
                    notices.push(broken_notice(source, &at, ru));
                }
            }
            Ok(extracted) => {
                source.failures = 0;
                source.error = extracted.warning.clone();
                let value = SourceValue {
                    at: at.clone(),
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
                    notices.push(pending_notice(source, before, after, &at, ru));
                } else {
                    push_value(source, value);
                    notices.extend(self.spread_source_value(id, ru));
                }
            }
        }
        self.persist()?;
        Ok(notices)
    }

    /// Take or drop the value waiting after a jump.
    pub fn confirm_source_value(
        &mut self,
        id: &str,
        accept: bool,
        ru: bool,
    ) -> Result<Vec<NoticeRecord>> {
        let source = self.data_source_mut(id)?;
        let Some(value) = source.pending.take() else {
            return Err(invalid("nothing waits for confirmation"));
        };
        source.status = SourceStatus::Ok;
        let notices = if accept {
            push_value(source, value);
            self.spread_source_value(id, ru)
        } else {
            Vec::new()
        };
        self.persist()?;
        Ok(notices)
    }

    /// Send the latest number to the prices it sets and compare it with the
    /// deposits it follows.
    fn spread_source_value(&mut self, id: &str, ru: bool) -> Vec<NoticeRecord> {
        let Some(source) = self.data_source(id).cloned() else {
            return Vec::new();
        };
        let Some(value) = source.last.clone() else {
            return Vec::new();
        };
        let Some(number) = value.number else {
            return Vec::new();
        };
        let points: Vec<PricePoint> = self
            .doc
            .asset_price_sources
            .iter()
            .filter(|link| link.source_id == id)
            .filter_map(|link| {
                let per_usd = self
                    .per_usd(&link.currency)
                    .filter(|rate| !rate.is_zero())?;
                Some(PricePoint {
                    asset_key: link.asset_key.clone(),
                    value_usd: number / per_usd,
                    ts: value.at.clone(),
                    source: format!("custom:{id}"),
                    change_24h_pct: None,
                })
            })
            .collect();
        if !points.is_empty() {
            self.apply_prices(&points);
        }
        let today = value.at.get(..10).unwrap_or_default().to_string();
        let mut notices = Vec::new();
        for link in self
            .doc
            .deposit_rate_sources
            .iter()
            .filter(|link| link.source_id == id)
        {
            let Some(store) = self
                .doc
                .stores
                .iter()
                .find(|store| store.id == link.store_id)
            else {
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
            notices.push(super::notice(
                NoticeCategory::Rates,
                format!("source:{id}:deposit:{}:{}", store.id, pct_text(number)),
                super::notice_title(NoticeCategory::Rates, ru),
                body,
                Some(format!("open:store:{}", store.id)),
                value.at.clone(),
            ));
        }
        notices
    }
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
    super::notice(
        NoticeCategory::Rates,
        format!("source:{}:broken:{at}", source.id),
        super::notice_title(NoticeCategory::Rates, ru),
        body,
        Some("planning-sources".into()),
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
    super::notice(
        NoticeCategory::Rates,
        format!("source:{}:pending:{at}", source.id),
        super::notice_title(NoticeCategory::Rates, ru),
        body,
        Some("planning-sources".into()),
        at.to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::datasource::TableRow;
    use crate::service::test_support::{amount, empty_vault as vault};

    fn draft(uses: Vec<SourceUse>) -> DataSourceDraft {
        DataSourceDraft {
            id: None,
            name: "ЦБ".into(),
            url: "https://cbr.ru/hd_base/infl/".into(),
            method: SourceMethod::Table {
                table_selector: "table.data".into(),
                column_header: "Инфляция".into(),
                row: TableRow::ByDate,
            },
            output: SourceOutput::Number,
            number: NumberFormat::default(),
            interval: SourceInterval::Day,
            uses,
        }
    }

    fn number(value: &str) -> std::result::Result<Extracted, String> {
        Ok(Extracted {
            number: Some(amount(value)),
            ..Extracted::default()
        })
    }

    #[test]
    fn three_failures_in_a_row_break_a_source() {
        let (_dir, mut vault) = vault();
        let id = vault.save_data_source(draft(vec![])).unwrap().source.id;
        for attempt in 1..=2 {
            let notices = vault
                .apply_source_fetch(&id, Err("no table".into()), true)
                .unwrap();
            assert!(notices.is_empty(), "attempt {attempt}");
        }
        let notices = vault
            .apply_source_fetch(&id, Err("no table".into()), true)
            .unwrap();
        assert_eq!(notices.len(), 1);
        assert_eq!(vault.data_source(&id).unwrap().status, SourceStatus::Broken);
        // A fourth failure does not announce it again; a value repairs it.
        assert!(
            vault
                .apply_source_fetch(&id, Err("no table".into()), true)
                .unwrap()
                .is_empty()
        );
        vault.apply_source_fetch(&id, number("6.35"), true).unwrap();
        let source = vault.data_source(&id).unwrap();
        assert_eq!(source.status, SourceStatus::Ok);
        assert_eq!(source.failures, 0);
        assert_eq!(source.last.as_ref().unwrap().number, Some(amount("6.35")));
    }

    #[test]
    fn a_jump_waits_for_confirmation() {
        let (_dir, mut vault) = vault();
        let id = vault.save_data_source(draft(vec![])).unwrap().source.id;
        vault.apply_source_fetch(&id, number("6.35"), true).unwrap();
        let notices = vault.apply_source_fetch(&id, number("635"), true).unwrap();
        assert_eq!(notices.len(), 1);
        let source = vault.data_source(&id).unwrap();
        assert_eq!(source.status, SourceStatus::Pending);
        assert_eq!(source.last.as_ref().unwrap().number, Some(amount("6.35")));
        vault.confirm_source_value(&id, true, true).unwrap();
        let source = vault.data_source(&id).unwrap();
        assert_eq!(source.status, SourceStatus::Ok);
        assert_eq!(source.last.as_ref().unwrap().number, Some(amount("635")));
        assert_eq!(source.history.len(), 1);
        // A small move is taken at once.
        vault.apply_source_fetch(&id, number("640"), true).unwrap();
        assert_eq!(vault.data_source(&id).unwrap().status, SourceStatus::Ok);
    }

    #[test]
    fn inflation_in_source_mode_reads_the_source() {
        let (_dir, mut vault) = vault();
        let view = vault
            .save_data_source(draft(vec![SourceUse::Inflation {
                currency: "THB".into(),
            }]))
            .unwrap();
        let id = view.source.id;
        assert_eq!(
            view.used_by,
            vec![SourceUse::Inflation {
                currency: "thb".into()
            }]
        );
        assert!(vault.inflation_series("thb").is_empty());
        vault
            .apply_source_fetch(
                &id,
                Ok(Extracted {
                    series: vec![
                        crate::model::datasource::SeriesPoint {
                            label: "08.2026".into(),
                            value: amount("1.10"),
                        },
                        crate::model::datasource::SeriesPoint {
                            label: "09.2026".into(),
                            value: amount("1.25"),
                        },
                    ],
                    ..Extracted::default()
                }),
                true,
            )
            .unwrap();
        let series = vault.inflation_series("thb");
        assert_eq!(series.len(), 2);
        assert_eq!(series[1].month, "2026-09");
        assert_eq!(series[1].yoy_pct, amount("1.25"));
        assert_eq!(
            vault.inflation_mode("thb", &Default::default()),
            InflationMode::Source
        );
        // Deleting the source returns the currency to its default mode.
        vault.delete_data_source(&id).unwrap();
        assert!(vault.doc.settings.inflation_source.is_empty());
        assert_ne!(
            vault.inflation_mode("thb", &Default::default()),
            InflationMode::Source
        );
    }

    #[test]
    fn a_price_source_writes_custom_points_in_usd() {
        let (_dir, mut vault) = vault();
        let id = vault
            .save_data_source(draft(vec![SourceUse::AssetPrice {
                asset_key: "asset:gold".into(),
                currency: "usd".into(),
            }]))
            .unwrap()
            .source
            .id;
        vault.apply_source_fetch(&id, number("2400"), true).unwrap();
        let point = vault
            .doc
            .price_cache
            .iter()
            .find(|point| point.asset_key == "asset:gold")
            .unwrap();
        assert_eq!(point.value_usd, amount("2400"));
        assert_eq!(point.source, format!("custom:{id}"));
    }

    #[test]
    fn a_draft_is_checked() {
        let (_dir, mut vault) = vault();
        let mut bad = draft(vec![]);
        bad.url = "ftp://x".into();
        assert!(vault.save_data_source(bad).is_err());
        let mut text = draft(vec![SourceUse::AssetPrice {
            asset_key: "asset:gold".into(),
            currency: "usd".into(),
        }]);
        text.output = SourceOutput::Text;
        assert!(vault.save_data_source(text).is_err());
    }
}
