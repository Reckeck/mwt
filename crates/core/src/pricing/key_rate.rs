//! The Bank of Russia key rate, for the tax-free limit on deposit interest.
//!
//! Source: the Bank's own `DailyInfo` SOAP service (`KeyRate`), public and
//! keyless. The answer has one row per day the rate was in force.

use chrono::NaiveDate;
use reqwest::header::{CONTENT_TYPE, USER_AGENT};

use super::PricingService;
use crate::Amount;
use crate::error::{Error, Result};
use crate::model::deposit::KeyRatePoint;

const CBR_DAILY_INFO: &str = "https://www.cbr.ru/DailyInfoWebServ/DailyInfo.asmx";

/// `<KR><DT>2026-03-20T00:00:00+03:00</DT><Rate>21.00</Rate></KR>` rows →
/// points, one per day the rate changed (consecutive equal days collapse).
pub fn parse_key_rates(xml: &str) -> Result<Vec<KeyRatePoint>> {
    let mut rows: Vec<KeyRatePoint> = Vec::new();
    for block in xml.split("<KR").skip(1) {
        let (Some(day), Some(rate)) = (inner(block, "DT"), inner(block, "Rate")) else {
            continue;
        };
        let Some(date) = day
            .get(..10)
            .and_then(|raw| NaiveDate::parse_from_str(raw, "%Y-%m-%d").ok())
        else {
            continue;
        };
        let Ok(rate) = Amount::parse(rate.trim()) else {
            continue;
        };
        rows.push(KeyRatePoint {
            date: date.format("%Y-%m-%d").to_string(),
            rate,
        });
    }
    if rows.is_empty() {
        return Err(Error::Pricing("cbr: no key rate rows".into()));
    }
    rows.sort_by(|left, right| left.date.cmp(&right.date));
    rows.dedup_by(|later, earlier| later.rate == earlier.rate);
    Ok(rows)
}

fn inner<'text>(block: &'text str, name: &str) -> Option<&'text str> {
    let open = format!("<{name}>");
    let start = block.find(&open)? + open.len();
    let end = block[start..].find(&format!("</{name}>"))? + start;
    Some(&block[start..end])
}

impl PricingService {
    /// Key rate changes between two days (network only; the caller caches).
    pub async fn fetch_key_rates(
        &self,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<KeyRatePoint>> {
        let body = format!(
            r#"<?xml version="1.0" encoding="utf-8"?><soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/"><soap:Body><KeyRate xmlns="http://web.cbr.ru/"><fromDate>{}</fromDate><ToDate>{}</ToDate></KeyRate></soap:Body></soap:Envelope>"#,
            from.format("%Y-%m-%d"),
            to.format("%Y-%m-%d"),
        );
        let resp = self
            .client
            .post(CBR_DAILY_INFO)
            .header(CONTENT_TYPE, "text/xml; charset=utf-8")
            .header("SOAPAction", "\"http://web.cbr.ru/KeyRate\"")
            .header(USER_AGENT, "trade-app")
            .body(body)
            .send()
            .await
            .map_err(|err| Error::Network(err.to_string()))?;
        if !resp.status().is_success() {
            return Err(Error::Pricing(format!(
                "cbr key rate http {:?}",
                resp.status()
            )));
        }
        let text = resp
            .text()
            .await
            .map_err(|err| Error::Network(err.to_string()))?;
        parse_key_rates(&text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_collapse_to_changes() {
        let xml = r#"<KeyRate xmlns=""><KR diffgr:id="KR1"><DT>2026-03-21T00:00:00+03:00</DT><Rate>21.00</Rate></KR>
<KR diffgr:id="KR2"><DT>2026-03-20T00:00:00+03:00</DT><Rate>21.00</Rate></KR>
<KR diffgr:id="KR3"><DT>2026-03-19T00:00:00+03:00</DT><Rate>16.00</Rate></KR></KeyRate>"#;
        let points = parse_key_rates(xml).unwrap();
        assert_eq!(points.len(), 2);
        assert_eq!(points[0].date, "2026-03-19");
        assert_eq!(points[1].date, "2026-03-20");
        assert_eq!(points[1].rate, Amount::parse("21").unwrap());
    }

    #[test]
    fn an_error_page_is_an_error() {
        assert!(parse_key_rates("<html>503</html>").is_err());
    }
}
