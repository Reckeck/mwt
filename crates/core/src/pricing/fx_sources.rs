//! Public FX sources for a given day, tried in order, with a walk back to
//! earlier days when none of them has the day asked for.
//!
//! | source      | covers                          | history | key |
//! |-------------|---------------------------------|---------|-----|
//! | open.er-api | ~160 fiat, latest only          | no      | no  |
//! | Frankfurter | ECB reference (~30 fiat, no RUB)| yes     | no  |
//! | ЦБ РФ       | RUB and ~40 currencies vs RUB   | yes     | no  |
//!
//! Every source is turned into units per 1 USD, the cache's unit. Parsers are
//! pure functions so they are tested on captured payloads, without a network.

use std::collections::HashMap;

use chrono::{Duration, NaiveDate};
use reqwest::header::{ACCEPT, USER_AGENT};
use rust_decimal::Decimal;

use super::PricingService;
use crate::Amount;
use crate::error::{Error, Result};
use crate::model::vault::FxPoint;

const FRANKFURTER: &str = "https://api.frankfurter.app";
const CBR_DAILY: &str = "https://www.cbr.ru/scripts/XML_daily.asp";

/// How many days back the chain walks before giving up on a currency.
pub const FX_LOOKBACK_DAYS: i64 = 7;

/// Rates of one source for one day: units per 1 USD, keyed by lowercase code.
#[derive(Debug, Clone, PartialEq)]
pub struct FxDay {
    /// The day the source says the rates are for (may precede the one asked).
    pub date: NaiveDate,
    pub source: &'static str,
    pub per_usd: HashMap<String, Amount>,
}

impl FxDay {
    fn points(&self, wanted: &[String]) -> Vec<FxPoint> {
        let ts = format!("{}T00:00:00Z", self.date.format("%Y-%m-%d"));
        wanted
            .iter()
            .filter_map(|code| {
                let rate = self.per_usd.get(code)?;
                rate.is_positive().then(|| FxPoint {
                    currency: code.clone(),
                    per_usd: *rate,
                    ts: ts.clone(),
                    source: self.source.into(),
                })
            })
            .collect()
    }
}

/// Frankfurter `/{date}?from=USD`: `{"base":"USD","date":"…","rates":{"EUR":0.9}}`.
pub fn parse_frankfurter(body: &serde_json::Value) -> Result<FxDay> {
    let date = body["date"]
        .as_str()
        .and_then(|raw| NaiveDate::parse_from_str(raw, "%Y-%m-%d").ok())
        .ok_or_else(|| Error::Pricing("frankfurter: no date".into()))?;
    let rates = body["rates"]
        .as_object()
        .ok_or_else(|| Error::Pricing("frankfurter: no rates".into()))?;
    let mut per_usd: HashMap<String, Amount> = rates
        .iter()
        // The JSON number's own text, not an f64 round trip: 0.9 stays 0.9.
        .filter_map(|(code, rate)| {
            let rate = rate.as_number()?;
            Some((code.to_lowercase(), Amount::parse(&rate.to_string()).ok()?))
        })
        .collect();
    per_usd.insert("usd".into(), Amount::new(Decimal::ONE));
    Ok(FxDay {
        date,
        source: "frankfurter",
        per_usd,
    })
}

/// ЦБ РФ `XML_daily.asp`: `<ValCurs Date="dd.mm.yyyy"><Valute><CharCode>USD</CharCode>
/// <Nominal>1</Nominal><Value>95,1234</Value></Valute>…`, rubles per `Nominal` units.
///
/// The payload is windows-1251, but every field read here is ASCII, so a lossy
/// UTF-8 view is enough and no codec dependency is needed.
pub fn parse_cbr(xml: &str) -> Result<FxDay> {
    let date = attr(xml, "ValCurs", "Date")
        .and_then(|raw| NaiveDate::parse_from_str(&raw, "%d.%m.%Y").ok())
        .ok_or_else(|| Error::Pricing("cbr: no date".into()))?;
    let mut rub_per_unit: HashMap<String, Decimal> = HashMap::new();
    for block in xml.split("<Valute").skip(1) {
        let (Some(code), Some(nominal), Some(value)) = (
            tag(block, "CharCode"),
            tag(block, "Nominal"),
            tag(block, "Value"),
        ) else {
            continue;
        };
        let nominal: Decimal = nominal.trim().parse().unwrap_or(Decimal::ONE);
        let Ok(value) = value.trim().replace(',', ".").parse::<Decimal>() else {
            continue;
        };
        if !nominal.is_zero() {
            rub_per_unit.insert(code.trim().to_lowercase(), value / nominal);
        }
    }
    let rub_per_usd = *rub_per_unit
        .get("usd")
        .ok_or_else(|| Error::Pricing("cbr: no USD".into()))?;
    let mut per_usd: HashMap<String, Amount> = rub_per_unit
        .iter()
        .filter(|(_, rub)| !rub.is_zero())
        .map(|(code, rub)| (code.clone(), Amount::new(rub_per_usd / rub)))
        .collect();
    per_usd.insert("rub".into(), Amount::new(rub_per_usd));
    per_usd.insert("usd".into(), Amount::new(Decimal::ONE));
    Ok(FxDay {
        date,
        source: "cbr",
        per_usd,
    })
}

fn tag(block: &str, name: &str) -> Option<String> {
    let open = format!("<{name}>");
    let start = block.find(&open)? + open.len();
    let end = block[start..].find(&format!("</{name}>"))? + start;
    Some(block[start..end].to_string())
}

fn attr(xml: &str, element: &str, name: &str) -> Option<String> {
    let start = xml.find(&format!("<{element}"))?;
    let head = &xml[start..start + xml[start..].find('>')?];
    let key = format!("{name}=\"");
    let from = head.find(&key)? + key.len();
    let to = head[from..].find('"')? + from;
    Some(head[from..to].to_string())
}

impl PricingService {
    async fn frankfurter_on(&self, date: NaiveDate) -> Result<FxDay> {
        let url = format!("{FRANKFURTER}/{}", date.format("%Y-%m-%d"));
        let resp = self
            .client
            .get(url)
            .query(&[("from", "USD")])
            .header(USER_AGENT, "multi-wallet-tracker")
            .header(ACCEPT, "application/json")
            .send()
            .await
            .map_err(|err| Error::Network(err.to_string()))?;
        if !resp.status().is_success() {
            return Err(Error::Pricing(format!(
                "frankfurter http {:?}",
                resp.status()
            )));
        }
        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|err| Error::Network(err.to_string()))?;
        parse_frankfurter(&body)
    }

    async fn cbr_on(&self, date: NaiveDate) -> Result<FxDay> {
        let resp = self
            .client
            .get(CBR_DAILY)
            .query(&[("date_req", date.format("%d/%m/%Y").to_string())])
            .header(USER_AGENT, "multi-wallet-tracker")
            .send()
            .await
            .map_err(|err| Error::Network(err.to_string()))?;
        if !resp.status().is_success() {
            return Err(Error::Pricing(format!("cbr http {:?}", resp.status())));
        }
        let bytes = resp
            .bytes()
            .await
            .map_err(|err| Error::Network(err.to_string()))?;
        parse_cbr(&String::from_utf8_lossy(&bytes))
    }

    /// Latest table from open.er-api, stamped with today's date.
    async fn er_api_today(&self, today: NaiveDate) -> Result<FxDay> {
        let mut per_usd = self.fx_rates().await?;
        per_usd.insert("usd".into(), Amount::new(Decimal::ONE));
        Ok(FxDay {
            date: today,
            source: "open.er-api",
            per_usd,
        })
    }

    /// Rates of `wanted` (lowercase codes) for `date`, from the first source
    /// that has them. A currency no source has for that day is looked up on
    /// the days before, up to [`FX_LOOKBACK_DAYS`]; one still missing then is
    /// simply absent from the result (the caller falls back to its cache).
    ///
    /// Network-only: the caller merges the points with `apply_fx`.
    pub async fn fetch_fx_on(
        &self,
        wanted: &[String],
        date: NaiveDate,
        today: NaiveDate,
    ) -> Vec<FxPoint> {
        let mut missing: Vec<String> = wanted
            .iter()
            .map(|code| code.trim().to_lowercase())
            .filter(|code| !code.is_empty() && code != "usd")
            .collect();
        missing.sort();
        missing.dedup();
        let mut found = Vec::new();
        for back in 0..=FX_LOOKBACK_DAYS {
            if missing.is_empty() {
                break;
            }
            let day = date - Duration::days(back);
            let mut sources = Vec::new();
            if day >= today
                && let Ok(table) = self.er_api_today(today).await
            {
                sources.push(table);
            }
            if let Ok(table) = self.frankfurter_on(day).await {
                sources.push(table);
            }
            if let Ok(table) = self.cbr_on(day).await {
                sources.push(table);
            }
            for table in &sources {
                let points = table.points(&missing);
                missing.retain(|code| !points.iter().any(|point| &point.currency == code));
                found.extend(points);
            }
        }
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn amount(raw: &str) -> Amount {
        Amount::parse(raw).unwrap()
    }

    #[test]
    fn frankfurter_payload_becomes_per_usd() {
        let body = serde_json::json!({"amount": 1.0, "base": "USD", "date": "2026-10-09", "rates": {"EUR": 0.9, "GBP": 0.75}});
        let day = parse_frankfurter(&body).unwrap();
        assert_eq!(day.date, NaiveDate::from_ymd_opt(2026, 10, 9).unwrap());
        assert_eq!(day.per_usd["eur"], amount("0.9"));
        assert_eq!(day.per_usd["usd"], amount("1"));
    }

    #[test]
    fn cbr_payload_is_converted_through_the_dollar() {
        let xml = r#"<?xml version="1.0" encoding="windows-1251"?>
<ValCurs Date="10.10.2026" name="Foreign Currency Market">
<Valute ID="R01235"><NumCode>840</NumCode><CharCode>USD</CharCode><Nominal>1</Nominal><Name>x</Name><Value>80,0000</Value></Valute>
<Valute ID="R01335"><NumCode>398</NumCode><CharCode>KZT</CharCode><Nominal>100</Nominal><Name>x</Name><Value>16,0000</Value></Valute>
</ValCurs>"#;
        let day = parse_cbr(xml).unwrap();
        assert_eq!(day.date, NaiveDate::from_ymd_opt(2026, 10, 10).unwrap());
        assert_eq!(day.per_usd["rub"], amount("80"));
        // 100 KZT = 16 RUB → 1 KZT = 0.16 RUB → 80 / 0.16 = 500 KZT per USD.
        assert_eq!(day.per_usd["kzt"], amount("500"));
    }

    #[test]
    fn broken_payloads_are_errors_not_panics() {
        assert!(parse_cbr("<html>maintenance</html>").is_err());
        assert!(parse_frankfurter(&serde_json::json!({"message": "not found"})).is_err());
    }

    #[test]
    fn points_cover_only_wanted_codes_on_the_source_day() {
        let day = FxDay {
            date: NaiveDate::from_ymd_opt(2026, 10, 9).unwrap(),
            source: "cbr",
            per_usd: HashMap::from([("rub".into(), amount("80")), ("eur".into(), amount("0"))]),
        };
        let points = day.points(&["rub".into(), "eur".into(), "gbp".into()]);
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].ts, "2026-10-09T00:00:00Z");
    }
}
