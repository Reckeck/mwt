//! Monthly inflation from the statistics offices, all public and keyless.
//!
//! - RUB: Bank of Russia `hd_base/infl` — an HTML table "Date | Key
//!   rate | Inflation, % y/y | Target", newest month first, decimal comma.
//! - USD: BLS CPI-U all items (`CUUR0000SA0`), JSON; the index level, so the
//!   annual rate is the index against the same month a year before.
//! - EUR: Eurostat HICP annual rate of change for the euro area
//!   (`prc_hicp_minr`, `RCH_A`), JSON-stat.
//!
//! The parsers are pure and tested on saved answers in `fixtures/`.

use chrono::{Datelike, NaiveDate};
use reqwest::header::{ACCEPT, USER_AGENT};

use super::PricingService;
use crate::Amount;
use crate::error::{Error, Result};
use crate::model::inflation::{InflationReading, InflationSource};

const CBR_INFLATION: &str = "https://www.cbr.ru/hd_base/infl/";
const BLS_CPI: &str = "https://api.bls.gov/publicAPI/v1/timeseries/data/CUUR0000SA0";
const EUROSTAT_HICP: &str =
    "https://ec.europa.eu/eurostat/api/dissemination/statistics/1.0/data/prc_hicp_minr";

fn reading(source: InflationSource, month: String, yoy_pct: Amount) -> InflationReading {
    InflationReading {
        currency: source.currency().into(),
        month,
        yoy_pct,
        source,
    }
}

fn sorted(mut rows: Vec<InflationReading>, source: &str) -> Result<Vec<InflationReading>> {
    if rows.is_empty() {
        return Err(Error::Pricing(format!("{source}: no inflation rows")));
    }
    rows.sort_by(|left, right| left.month.cmp(&right.month));
    rows.dedup_by(|later, earlier| later.month == earlier.month);
    Ok(rows)
}

/// `MM.YYYY` → `YYYY-MM`.
fn cbr_month(raw: &str) -> Option<String> {
    let (month, year) = raw.trim().split_once('.')?;
    let month: u32 = month.parse().ok()?;
    let year: i32 = year.parse().ok()?;
    ((1..=12).contains(&month) && (1900..=9999).contains(&year))
        .then(|| format!("{year:04}-{month:02}"))
}

/// The text inside every `<tag …>…</tag>` of a fragment, tags stripped.
fn cells<'text>(row: &'text str, tag: &str) -> Vec<&'text str> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    row.split(&open)
        .skip(1)
        .filter_map(|cell| {
            let body = &cell[cell.find('>')? + 1..];
            Some(body[..body.find(&close).unwrap_or(body.len())].trim())
        })
        .collect()
}

/// The Bank of Russia table → readings, oldest first. The inflation column
/// is found by its header, so a reordered table still reads right.
pub fn parse_inflation_cbr(html: &str) -> Result<Vec<InflationReading>> {
    let table = html
        .find("<table class=\"data\"")
        .map(|start| &html[start..])
        .ok_or_else(|| Error::Pricing("cbr: no data table".into()))?;
    let table = &table[..table.find("</table>").unwrap_or(table.len())];
    let mut rows = table.split("<tr").skip(1);
    let header = rows
        .next()
        .ok_or_else(|| Error::Pricing("cbr: empty table".into()))?;
    let column = cells(header, "th")
        .iter()
        .position(|title| title.starts_with("Инфляция"))
        .ok_or_else(|| Error::Pricing("cbr: no inflation column".into()))?;
    let readings = rows
        .filter_map(|row| {
            let values = cells(row, "td");
            let month = cbr_month(values.first()?)?;
            let value = Amount::parse(&values.get(column)?.replace(',', ".")).ok()?;
            Some(reading(InflationSource::Cbr, month, value))
        })
        .collect();
    sorted(readings, "cbr")
}

/// BLS CPI-U index levels → annual rates, oldest first. A month whose index
/// or year-before index is missing (`"-"`) gets no reading.
pub fn parse_inflation_bls(json: &str) -> Result<Vec<InflationReading>> {
    let body: serde_json::Value =
        serde_json::from_str(json).map_err(|err| Error::Pricing(format!("bls: {err}")))?;
    let data = body
        .pointer("/Results/series/0/data")
        .and_then(|data| data.as_array())
        .ok_or_else(|| Error::Pricing("bls: no series".into()))?;
    let mut index = std::collections::BTreeMap::new();
    for point in data {
        let year = point.get("year").and_then(|year| year.as_str());
        let period = point.get("period").and_then(|period| period.as_str());
        let value = point.get("value").and_then(|value| value.as_str());
        let (Some(year), Some(period), Some(value)) = (year, period, value) else {
            continue;
        };
        let (Ok(year), Some(Ok(month))) = (
            year.parse::<i32>(),
            period.strip_prefix('M').map(str::parse::<u32>),
        ) else {
            continue;
        };
        // M13 is the annual average, not a month.
        if !(1..=12).contains(&month) {
            continue;
        }
        if let Ok(level) = Amount::parse(value)
            && level.is_positive()
        {
            index.insert((year, month), level);
        }
    }
    let hundred = Amount::from(100);
    let readings = index
        .iter()
        .filter_map(|(&(year, month), &level)| {
            let before = index.get(&(year - 1, month))?;
            let yoy = ((level / *before - Amount::from(1)) * hundred).round(2);
            Some(reading(
                InflationSource::Bls,
                format!("{year:04}-{month:02}"),
                yoy,
            ))
        })
        .collect();
    sorted(readings, "bls")
}

/// Eurostat JSON-stat → readings, oldest first. The query fixes every
/// dimension but time to one value, so a value's flat index steps by the
/// time dimension's stride.
pub fn parse_inflation_eurostat(json: &str) -> Result<Vec<InflationReading>> {
    let body: serde_json::Value =
        serde_json::from_str(json).map_err(|err| Error::Pricing(format!("eurostat: {err}")))?;
    let dims: Vec<&str> = body
        .get("id")
        .and_then(|ids| ids.as_array())
        .map(|ids| ids.iter().filter_map(|id| id.as_str()).collect())
        .unwrap_or_default();
    let sizes: Vec<u64> = body
        .get("size")
        .and_then(|sizes| sizes.as_array())
        .map(|sizes| sizes.iter().filter_map(|size| size.as_u64()).collect())
        .unwrap_or_default();
    let time_at = dims
        .iter()
        .position(|dim| *dim == "time")
        .filter(|_| dims.len() == sizes.len())
        .ok_or_else(|| Error::Pricing("eurostat: no time dimension".into()))?;
    let stride: u64 = sizes[time_at + 1..].iter().product();
    let months = body
        .pointer("/dimension/time/category/index")
        .and_then(|index| index.as_object())
        .ok_or_else(|| Error::Pricing("eurostat: no time index".into()))?;
    let values = body.get("value");
    let readings = months
        .iter()
        .filter_map(|(month, position)| {
            let flat = position.as_u64()? * stride;
            let value = values?.get(flat.to_string())?.as_f64()?;
            let month = month.get(..7)?;
            NaiveDate::parse_from_str(&format!("{month}-01"), "%Y-%m-%d").ok()?;
            Some(reading(
                InflationSource::Eurostat,
                month.into(),
                Amount::from_f64(value).round(2),
            ))
        })
        .collect();
    sorted(readings, "eurostat")
}

impl PricingService {
    /// Readings of one source from `from`'s month to `to` (network only;
    /// the caller applies them).
    pub async fn fetch_inflation(
        &self,
        source: InflationSource,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<InflationReading>> {
        let request = match source {
            InflationSource::Cbr => self.client.get(CBR_INFLATION).query(&[
                ("UniDbQuery.Posted", "True".to_string()),
                (
                    "UniDbQuery.From",
                    format!("01.{:02}.{}", from.month(), from.year()),
                ),
                ("UniDbQuery.To", to.format("%d.%m.%Y").to_string()),
            ]),
            // v1 answers the last three years without a key.
            InflationSource::Bls => self.client.get(BLS_CPI),
            InflationSource::Eurostat => self.client.get(EUROSTAT_HICP).query(&[
                ("format", "JSON".to_string()),
                ("lang", "EN".to_string()),
                ("geo", "EA".to_string()),
                ("unit", "RCH_A".to_string()),
                ("coicop18", "TOTAL".to_string()),
                ("sinceTimePeriod", from.format("%Y-%m").to_string()),
            ]),
            InflationSource::Custom => {
                return Err(Error::Unsupported);
            }
        };
        let resp = request
            .header(USER_AGENT, "multi-wallet-tracker")
            .header(ACCEPT, "*/*")
            .send()
            .await
            .map_err(|err| Error::Network(err.to_string()))?;
        if !resp.status().is_success() {
            return Err(Error::Pricing(format!(
                "{source:?} inflation http {:?}",
                resp.status()
            )));
        }
        let text = resp
            .text()
            .await
            .map_err(|err| Error::Network(err.to_string()))?;
        match source {
            InflationSource::Cbr => parse_inflation_cbr(&text),
            InflationSource::Bls => parse_inflation_bls(&text),
            InflationSource::Eurostat | InflationSource::Custom => parse_inflation_eurostat(&text),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pct(raw: &str) -> Amount {
        Amount::parse(raw).unwrap()
    }

    fn at<'rows>(rows: &'rows [InflationReading], month: &str) -> Option<&'rows InflationReading> {
        rows.iter().find(|row| row.month == month)
    }

    #[test]
    fn cbr_table_reads_by_header() {
        let rows = parse_inflation_cbr(include_str!("fixtures/cbr_inflation.html")).unwrap();
        assert_eq!(rows.len(), 21);
        assert_eq!(rows[0].month, "2025-01");
        let last = rows.last().unwrap();
        assert_eq!(last.month, "2026-09");
        assert_eq!(last.yoy_pct, pct("6.35"));
        assert_eq!(last.currency, "rub");
        assert_eq!(last.source, InflationSource::Cbr);
        assert_eq!(at(&rows, "2026-08").unwrap().yoy_pct, pct("6.33"));
    }

    #[test]
    fn cbr_columns_may_move() {
        let html = r#"<table class="data"><tr><th>Дата</th><th>Инфляция, % г/г</th><th>Ключевая ставка</th></tr>
<tr><td>02.2026</td><td>5,10</td><td>15,00</td></tr><tr><td>bad</td><td>1</td></tr></table>"#;
        let rows = parse_inflation_cbr(html).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].yoy_pct, pct("5.10"));
    }

    #[test]
    fn bls_rate_is_the_index_a_year_on() {
        let rows = parse_inflation_bls(include_str!("fixtures/bls_cpi.json")).unwrap();
        assert_eq!(rows[0].month, "2025-01");
        assert_eq!(rows[0].yoy_pct, pct("3.00"));
        assert_eq!(at(&rows, "2026-09").unwrap().yoy_pct, pct("3.74"));
        // October 2025 was never published, so neither it nor a year on reads.
        assert!(at(&rows, "2025-10").is_none());
        assert!(rows.iter().all(|row| row.month.as_str() < "2026-10"));
    }

    #[test]
    fn eurostat_json_stat() {
        let rows = parse_inflation_eurostat(include_str!("fixtures/eurostat_hicp.json")).unwrap();
        assert_eq!(rows.len(), 21);
        assert_eq!(rows[0].month, "2025-01");
        assert_eq!(rows[0].yoy_pct, pct("2.5"));
        assert_eq!(rows.last().unwrap().month, "2026-09");
        assert_eq!(rows.last().unwrap().yoy_pct, pct("3.8"));
        assert_eq!(rows[0].currency, "eur");
    }

    #[test]
    fn error_pages_are_errors() {
        assert!(parse_inflation_cbr("<html>503</html>").is_err());
        assert!(parse_inflation_bls(r#"{"status":"REQUEST_NOT_PROCESSED"}"#).is_err());
        assert!(parse_inflation_bls("<html>").is_err());
        assert!(parse_inflation_eurostat(r#"{"error":[{"status":400}]}"#).is_err());
    }
}
