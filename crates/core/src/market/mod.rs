//! Market data: OHLCV candles for charting.
//!
//! Deliberately separate from [`crate::pricing`], which answers "what is one
//! unit worth right now" and caches spot/FX quotes inside the vault. This
//! module answers "what did the candles look like over this window" and keeps
//! nothing: every call is a public, keyless Binance REST request made on
//! demand by the chart the user is currently looking at.
//!
//! These candles once lived in `sim/engine.rs`, back when the Strategy Lab was
//! the only consumer. The simulator is gone, but the charting feature is not,
//! so the fetch path moved here rather than being deleted with it.

use reqwest::Client;
use time::format_description::well_known::Rfc3339;

use crate::error::{Error, Result};

/// A single OHLC candle.
///
/// `f64` is deliberate and confined to this module: candles are third-party
/// market data that is only ever plotted, never summed into a balance, so the
/// `Amount`/Decimal guarantee does not apply. Every value is stringified at the
/// IPC boundary (`GetCandlesOutput`), and the frontend parses it for the chart.
#[derive(Debug, Clone, PartialEq)]
pub struct Candle {
    pub ts: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

/// Binance interval code for a granularity given in minutes.
pub fn interval_str(min: u32) -> &'static str {
    match min {
        1 => "1m",
        3 => "3m",
        5 => "5m",
        15 => "15m",
        30 => "30m",
        60 => "1h",
        240 => "4h",
        1440 => "1d",
        _ => "1h",
    }
}

/// Fetch the most recent `limit` candles from Binance public klines.
pub async fn fetch_candles(
    symbol: &str,
    interval_min: u32,
    limit: u32,
    client: &Client,
) -> Result<Vec<Candle>> {
    let pair = format!("{symbol}USDT");
    let url = "https://api.binance.com/api/v3/klines".to_string();
    let resp = client
        .get(&url)
        .query(&[
            ("symbol", pair.as_str()),
            ("interval", interval_str(interval_min)),
            ("limit", &limit.to_string()),
        ])
        .send()
        .await
        .map_err(|err| Error::Network(err.to_string()))?;
    if !resp.status().is_success() {
        return Err(Error::Pricing(format!(
            "binance klines http {}",
            resp.status()
        )));
    }
    let rows: Vec<Vec<serde_json::Value>> = resp
        .json()
        .await
        .map_err(|err| Error::Network(err.to_string()))?;
    let out = parse_klines(&rows);
    if out.is_empty() {
        return Err(Error::Pricing("no candles returned".into()));
    }
    Ok(out)
}

/// Binance returns klines as positional string arrays; short rows are skipped
/// rather than panicking, so a malformed response degrades to fewer candles.
fn parse_klines(rows: &[Vec<serde_json::Value>]) -> Vec<Candle> {
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        if row.len() < 6 {
            continue;
        }
        let ts = iso_from_millis(row[0].as_u64().unwrap_or(0));
        out.push(Candle {
            ts,
            open: row[1]
                .as_str()
                .and_then(|text| text.parse().ok())
                .unwrap_or(0.0),
            high: row[2]
                .as_str()
                .and_then(|text| text.parse().ok())
                .unwrap_or(0.0),
            low: row[3]
                .as_str()
                .and_then(|text| text.parse().ok())
                .unwrap_or(0.0),
            close: row[4]
                .as_str()
                .and_then(|text| text.parse().ok())
                .unwrap_or(0.0),
            volume: row[5]
                .as_str()
                .and_then(|text| text.parse().ok())
                .unwrap_or(0.0),
        });
    }
    out
}

/// Unix milliseconds to ISO-8601 (RFC3339) UTC.
pub fn iso_from_millis(ms: u64) -> String {
    let secs = (ms / 1000) as i64;
    let nanos = ((ms % 1000) * 1_000_000) as u32;
    let stamp = time::OffsetDateTime::from_unix_timestamp(secs)
        .unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    let stamp = stamp.replace_nanosecond(nanos).unwrap_or(stamp);
    stamp.format(&Rfc3339).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interval_maps_minutes_to_binance_codes() {
        assert_eq!(interval_str(1), "1m");
        assert_eq!(interval_str(60), "1h");
        assert_eq!(interval_str(1440), "1d");
        // unknown granularity must still yield a usable code, never an empty one
        assert_eq!(interval_str(7), "1h");
    }

    #[test]
    fn klines_are_parsed_by_position() {
        let rows = vec![vec![
            serde_json::json!(1_767_225_600_000u64),
            serde_json::json!("100.5"),
            serde_json::json!("110.0"),
            serde_json::json!("95.0"),
            serde_json::json!("105.25"),
            serde_json::json!("1234.5"),
            serde_json::json!("0"),
        ]];
        let out = parse_klines(&rows);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].open, 100.5);
        assert_eq!(out[0].high, 110.0);
        assert_eq!(out[0].low, 95.0);
        assert_eq!(out[0].close, 105.25);
        assert_eq!(out[0].volume, 1234.5);
        assert!(out[0].ts.starts_with("2026-01-01T00:00:00"));
    }

    /// A garbled row must shorten the result, never panic: the chart is a
    /// read-only convenience and must not be able to take down the command.
    #[test]
    fn short_or_unparsable_rows_are_skipped_not_fatal() {
        let rows = vec![
            vec![serde_json::json!(1_767_225_600_000u64)],
            vec![],
            vec![
                serde_json::json!(1_767_225_600_000u64),
                serde_json::json!("not-a-number"),
                serde_json::json!("110"),
                serde_json::json!("95"),
                serde_json::json!("105"),
                serde_json::json!("1234"),
                serde_json::json!("0"),
            ],
        ];
        let out = parse_klines(&rows);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].open, 0.0);
        assert_eq!(out[0].close, 105.0);
    }

    #[test]
    fn millis_round_trip_to_iso() {
        assert_eq!(iso_from_millis(1_767_225_600_000), "2026-01-01T00:00:00Z");
        // sub-second precision is preserved, not truncated away
        assert!(iso_from_millis(1_767_225_600_123).contains(".123"));
    }
}
