//! Strategy simulation engine: replay candle streams against a
//! [`Strategy`] and produce a [`SimRun`] with metrics.
//!
//! Data sources:
//! - `Real`: Binance klines (public REST, free, full history).
//! - `Synthetic`: offline GBM generator for instant no-network tests.

use reqwest::Client;

use super::{EquityPoint, Rule, SimRun, SimTrade, Strategy};
use crate::Amount;
use crate::error::{Error, Result};
use crate::model::vault::now_iso;

/// A single OHLC candle.
#[derive(Debug, Clone, PartialEq)]
pub struct Candle {
    pub ts: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
}

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

/// Fetch `limit` candles from Binance public klines.
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
        .map_err(|e| Error::Network(e.to_string()))?;
    if !resp.status().is_success() {
        return Err(Error::Pricing(format!(
            "binance klines http {}",
            resp.status()
        )));
    }
    let rows: Vec<Vec<serde_json::Value>> = resp
        .json()
        .await
        .map_err(|e| Error::Network(e.to_string()))?;
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        if r.len() < 6 {
            continue;
        }
        let ts = iso_from_millis(r[0].as_u64().unwrap_or(0));
        out.push(Candle {
            ts,
            open: r[1].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0),
            high: r[2].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0),
            low: r[3].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0),
            close: r[4].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0),
        });
    }
    if out.is_empty() {
        return Err(Error::Pricing("no candles returned".into()));
    }
    Ok(out)
}

fn iso_from_millis(ms: u64) -> String {
    use time::format_description::well_known::Rfc3339;
    let secs = (ms / 1000) as i64;
    let nanos = ((ms % 1000) * 1_000_000) as u32;
    let dt = time::OffsetDateTime::from_unix_timestamp(secs)
        .unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    let dt = dt.replace_nanosecond(nanos).unwrap_or(dt);
    dt.format(&Rfc3339).unwrap_or_default()
}

/// Standard normal via Box-Muller (no extra dependency).
fn standard_normal() -> f64 {
    loop {
        let u1 = rand::random::<f64>();
        let u2 = rand::random::<f64>();
        if u1 > 1e-12 {
            return (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
        }
    }
}

/// Synthesize `n` candles with GBM (for offline tests).
pub fn synthetic_candles(
    symbol: &str,
    n: usize,
    start_price: f64,
    annual_vol: f64,
    candles_per_year: f64,
) -> Vec<Candle> {
    let dt = 1.0 / candles_per_year;
    let drift = 0.06;
    let mut price = start_price;
    let mut out = Vec::with_capacity(n);
    let base = time::OffsetDateTime::now_utc() - time::Duration::minutes(n as i64 * 60);
    let _ = symbol;
    for i in 0..n {
        let z = standard_normal();
        price *= ((drift - 0.5 * annual_vol * annual_vol) * dt + annual_vol * z * dt.sqrt()).exp();
        let open = price;
        let intraday = standard_normal();
        let close = (price * (1.0 + intraday * 0.004 * dt.sqrt())).max(open * 0.5);
        let high = open.max(close) + rand::random::<f64>() * 0.002 * price;
        let low = (open.min(close) - rand::random::<f64>() * 0.002 * price).max(price * 0.01);
        let ts = (base + time::Duration::minutes(i as i64 * 60)).to_string();
        out.push(Candle {
            ts,
            open,
            high,
            low,
            close,
        });
    }
    out
}

// -- indicators --------------------------------------------------------------

pub fn sma(values: &[f64], period: usize) -> Vec<Option<f64>> {
    let mut out = vec![None; values.len()];
    if period == 0 {
        return out;
    }
    let mut acc = 0.0;
    for (i, v) in values.iter().enumerate() {
        acc += v;
        if i >= period {
            acc -= values[i - period];
        }
        if i + 1 >= period {
            out[i] = Some(acc / period as f64);
        }
    }
    out
}

pub fn ema(values: &[f64], period: usize) -> Vec<Option<f64>> {
    let mut out = vec![None; values.len()];
    if period == 0 {
        return out;
    }
    let k = 2.0 / (period as f64 + 1.0);
    let mut prev = None;
    for (i, v) in values.iter().enumerate() {
        prev = Some(match prev {
            None => *v,
            Some(p) => v * k + p * (1.0 - k),
        });
        if i + 1 >= period {
            out[i] = prev;
        }
    }
    out
}

pub fn rsi(values: &[f64], period: usize) -> Vec<Option<f64>> {
    let mut out = vec![None; values.len()];
    let n = values.len();
    if period == 0 || n <= period {
        return out;
    }
    let mut gains = 0.0;
    let mut losses = 0.0;
    for i in 1..=period {
        let d = values[i] - values[i - 1];
        if d >= 0.0 {
            gains += d;
        } else {
            losses -= d;
        }
    }
    let mut avg_gain = gains / period as f64;
    let mut avg_loss = losses / period as f64;
    out[period] = Some(idx(avg_gain, avg_loss));
    for i in (period + 1)..n {
        let d = values[i] - values[i - 1];
        let g = if d > 0.0 { d } else { 0.0 };
        let l = if d < 0.0 { -d } else { 0.0 };
        avg_gain = (avg_gain * (period as f64 - 1.0) + g) / period as f64;
        avg_loss = (avg_loss * (period as f64 - 1.0) + l) / period as f64;
        out[i] = Some(idx(avg_gain, avg_loss));
    }
    out
}

fn idx(avg_gain: f64, avg_loss: f64) -> f64 {
    if avg_loss == 0.0 {
        100.0
    } else {
        100.0 - 100.0 / (1.0 + avg_gain / avg_loss)
    }
}

fn series_for(name: &str, values: &[f64]) -> Vec<Option<f64>> {
    match name.to_lowercase().as_str() {
        "close" => values.iter().map(|v| Some(*v)).collect(),
        "sma20" => sma(values, 20),
        "sma50" => sma(values, 50),
        "ema12" => ema(values, 12),
        "ema26" => ema(values, 26),
        "rsi14" => rsi(values, 14),
        _ => values.iter().map(|v| Some(*v)).collect(),
    }
}

fn eval_rule(r: &Rule, cur: f64, prev: f64) -> bool {
    let threshold = r.value.to_f64();
    match r.op.as_str() {
        "gt" | "over" => cur > threshold,
        "lt" | "under" => cur < threshold,
        "cross_up" => prev <= threshold && cur > threshold,
        "cross_down" => prev >= threshold && cur < threshold,
        _ => false,
    }
}

fn all_rules(rules: &[Rule], series: &[f64], i: usize) -> bool {
    if rules.is_empty() {
        return false;
    }
    rules.iter().all(|r| {
        let s = series_for(&r.indicator, series);
        let next = if i > 0 { i - 1 } else { 0 };
        match (s.get(i), s.get(next)) {
            (Some(Some(cur)), Some(Some(prev))) => eval_rule(r, *cur, *prev),
            (Some(Some(cur)), None) => eval_rule(r, *cur, *cur),
            _ => false,
        }
    })
}

// -- runner -------------------------------------------------------------------

/// Run a strategy against candles; returns a full [`SimRun`].
pub fn run_on_candles(strategy: &Strategy, candles: &[Candle]) -> SimRun {
    let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
    let mut cash_f: f64 = strategy.initial_capital.to_f64();
    let mut qty: f64 = 0.0;
    let mut equity_curve = Vec::with_capacity(candles.len());
    let mut trades: Vec<SimTrade> = Vec::new();
    let mut wins = 0u32;
    let mut losses = 0u32;
    let mut max_equity = f64::MIN;
    let mut max_dd = 0.0f64;
    let mut open_price: Option<f64> = None;

    for (i, c) in candles.iter().enumerate() {
        let equity = cash_f + qty * c.close;
        max_equity = max_equity.max(equity);
        if max_equity > 0.0 {
            max_dd = max_dd.max((max_equity - equity) / max_equity);
        }
        equity_curve.push(EquityPoint::new(c.ts.clone(), Amount::from_f64(equity)));

        let in_position = qty > 1e-12;

        if in_position {
            let should_exit = all_rules(&strategy.exit_rules, &closes, i);
            let sl_tp = open_price
                .map(|entry| {
                    let pct = (c.close - entry) / entry;
                    let sl = strategy
                        .stop_loss_pct
                        .map(|s| pct <= -(s.to_f64() / 100.0))
                        .unwrap_or(false);
                    let tp = strategy
                        .take_profit_pct
                        .map(|s| pct >= s.to_f64() / 100.0)
                        .unwrap_or(false);
                    sl || tp
                })
                .unwrap_or(false);
            if should_exit || sl_tp {
                sell_at(
                    c,
                    &mut cash_f,
                    &mut qty,
                    &mut trades,
                    &mut wins,
                    &mut losses,
                    &mut open_price,
                );
                continue;
            }
        }

        if !in_position && all_rules(&strategy.entry_rules, &closes, i) {
            let invest_pct = strategy.position_size_pct.to_f64().clamp(0.0, 1.0);
            let invest = cash_f * invest_pct;
            let fee = invest * 0.001;
            let q = (invest - fee) / c.close;
            if q > 1e-9 && invest > 0.0 {
                cash_f -= invest;
                qty += q;
                open_price = Some(c.close);
                trades.push(SimTrade {
                    id: uuid::Uuid::new_v4().to_string(),
                    ts: c.ts.clone(),
                    side: "buy".into(),
                    price: Amount::from_f64(c.close),
                    qty: Amount::from_f64(q),
                    value: Amount::from_f64(invest),
                    fee: Amount::from_f64(fee),
                });
            }
        }
    }

    // liquidate the remainder at the last close for clean metrics
    if qty > 1e-12
        && let Some(c) = candles.last()
    {
        let value = qty * c.close;
        cash_f += value;
        trades.push(SimTrade {
            id: uuid::Uuid::new_v4().to_string(),
            ts: c.ts.clone(),
            side: "sell".into(),
            price: Amount::from_f64(c.close),
            qty: Amount::from_f64(qty),
            value: Amount::from_f64(value),
            fee: Amount::ZERO,
        });
    }

    let initial = strategy.initial_capital.to_f64();
    let final_equity = Amount::from_f64(cash_f);
    let total_return_pct = if initial != 0.0 {
        (cash_f - initial) / initial * 100.0
    } else {
        0.0
    };
    let closed = (wins + losses) as f64;
    let win_rate_pct = if closed > 0.0 {
        wins as f64 / closed * 100.0
    } else {
        0.0
    };

    equity_curve.push(EquityPoint::new(now_iso(), final_equity));

    SimRun {
        id: uuid::Uuid::new_v4().to_string(),
        strategy_id: strategy.id.clone(),
        started_at: candles
            .first()
            .map(|c| c.ts.clone())
            .unwrap_or_else(now_iso),
        finished_at: candles.last().map(|c| c.ts.clone()).unwrap_or_else(now_iso),
        initial_capital: strategy.initial_capital,
        final_equity: final_equity.round(2),
        total_return_pct: Amount::from_f64(total_return_pct).round(2),
        max_drawdown_pct: Amount::from_f64(max_dd * 100.0).round(2),
        win_rate_pct: Amount::from_f64(win_rate_pct).round(2),
        total_trades: trades.iter().filter(|t| t.side == "buy").count() as u32,
        equity_curve,
        trades,
        synthetic: strategy.source != crate::sim::DataSource::Real,
    }
}

fn sell_at(
    c: &Candle,
    cash: &mut f64,
    qty: &mut f64,
    trades: &mut Vec<SimTrade>,
    wins: &mut u32,
    losses: &mut u32,
    open_price: &mut Option<f64>,
) {
    let value = *qty * c.close;
    let fee = value * 0.001;
    *cash += value - fee;
    trades.push(SimTrade {
        id: uuid::Uuid::new_v4().to_string(),
        ts: c.ts.clone(),
        side: "sell".into(),
        price: Amount::from_f64(c.close),
        qty: Amount::from_f64(*qty),
        value: Amount::from_f64(value),
        fee: Amount::from_f64(fee),
    });
    if let Some(entry) = open_price {
        if c.close >= *entry {
            *wins += 1;
        } else {
            *losses += 1;
        }
    }
    *qty = 0.0;
    *open_price = None;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candles_from(closes: &[f64]) -> Vec<Candle> {
        closes
            .iter()
            .enumerate()
            .map(|(i, c)| Candle {
                ts: format!("2026-01-{:02}T00:00:00Z", i % 28 + 1),
                open: *c,
                high: *c * 1.01,
                low: *c * 0.99,
                close: *c,
            })
            .collect()
    }

    #[test]
    fn sma_ema_rsi_basic() {
        let v: Vec<f64> = (1..=30).map(|x| x as f64).collect();
        assert!((sma(&v, 3)[4].unwrap() - 4.0).abs() < 1e-9);
        assert!(ema(&v, 3)[2].is_some());
        assert!(rsi(&v, 14)[20].is_some());
    }

    #[test]
    fn uptrend_yields_positive_return() {
        let s = Strategy {
            name: "hold".into(),
            initial_capital: Amount::parse("1000").unwrap(),
            position_size_pct: Amount::parse("1").unwrap(),
            entry_rules: vec![Rule {
                indicator: "close".into(),
                field: "close".into(),
                op: "gt".into(),
                value: Amount::parse("0").unwrap(),
            }],
            ..Default::default()
        };
        let closes: Vec<f64> = (0..100).map(|i| 10.0 + i as f64 * 0.1).collect();
        let run = run_on_candles(&s, &candles_from(&closes));
        assert!(run.final_equity.to_f64() > 1000.0);
        assert!(run.equity_curve.len() >= 100);
    }

    #[test]
    fn synthetic_generates_candles() {
        let cs = synthetic_candles("ETH", 100, 3000.0, 0.7, 365.0 * 24.0);
        assert_eq!(cs.len(), 100);
        assert!(cs.iter().all(|c| c.close > 0.0));
    }
}
