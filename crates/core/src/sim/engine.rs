//! Strategy simulation engine: replay candle streams against a
//! [`Strategy`] and produce a [`SimRun`] with metrics.
//!
//! Data sources:
//! - `Real`: Binance klines (public REST, free, full history).
//! - `Synthetic`: offline GBM generator for instant no-network tests.

use reqwest::Client;

use super::{EquityPoint, Rule, SimRun, SimSignal, SimTrade, Strategy};
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
    /// Base-asset volume for the interval; synthetic candles use a
    /// plausible random value so the volume pane is not empty.
    pub volume: f64,
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
    let out = parse_klines(&rows);
    if out.is_empty() {
        return Err(Error::Pricing("no candles returned".into()));
    }
    Ok(out)
}

/// Fetch candles opening in `[start_ms, end_ms]` from Binance, paginating
/// with `startTime` (the API caps one page at 1000 rows). Hard-capped at
/// 5000 rows so a huge range cannot turn into an unbounded download.
pub async fn fetch_candles_between(
    symbol: &str,
    interval_min: u32,
    start_ms: u64,
    end_ms: u64,
    client: &Client,
) -> Result<Vec<Candle>> {
    if start_ms >= end_ms {
        return Ok(Vec::new());
    }
    let pair = format!("{symbol}USDT");
    let url = "https://api.binance.com/api/v3/klines";
    let interval = interval_str(interval_min);
    let mut cursor = start_ms;
    let mut out: Vec<Candle> = Vec::new();
    while cursor < end_ms && out.len() < 5000 {
        let resp = client
            .get(url)
            .query(&[
                ("symbol", pair.as_str()),
                ("interval", interval),
                ("startTime", &cursor.to_string()),
                ("endTime", &end_ms.to_string()),
                ("limit", "1000"),
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
        if rows.is_empty() {
            break;
        }
        let last_open = rows
            .last()
            .and_then(|r| r.first())
            .and_then(|v| v.as_u64())
            .unwrap_or(cursor);
        out.extend(parse_klines(&rows));
        if rows.len() < 1000 {
            break;
        }
        cursor = last_open + 1;
    }
    out.dedup_by(|a, b| a.ts == b.ts);
    Ok(out)
}

fn parse_klines(rows: &[Vec<serde_json::Value>]) -> Vec<Candle> {
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
            volume: r[5].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0),
        });
    }
    out
}

/// RFC3339 → unix ms; `None` on unparsable input.
pub fn rfc3339_to_millis(ts: &str) -> Option<u64> {
    use time::format_description::well_known::Rfc3339;
    let dt = time::OffsetDateTime::parse(ts, &Rfc3339).ok()?;
    Some((dt.unix_timestamp_nanos() / 1_000_000) as u64)
}

pub fn iso_from_millis(ms: u64) -> String {
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
    standard_normal_with(&mut rand::thread_rng())
}

/// Box-Muller draw from an explicit RNG — the seeded path for replays.
fn standard_normal_with(rng: &mut impl rand::Rng) -> f64 {
    loop {
        let u1: f64 = rng.sample(rand::distributions::Standard);
        let u2: f64 = rng.sample(rand::distributions::Standard);
        if u1 > 1e-12 {
            return (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
        }
    }
}

fn unit_with(rng: &mut impl rand::Rng) -> f64 {
    rng.sample(rand::distributions::Standard)
}

/// Fixed epoch for synthetic series: 2026-01-01T00:00:00Z. A deterministic
/// base keeps timestamps reproducible, so replayed candles line up with the
/// trades/signals stored in the run.
const SYNTHETIC_BASE_MS: u64 = 1_767_225_600_000;

/// Synthesize `n` candles with seeded GBM (offline, reproducible).
#[allow(clippy::too_many_arguments)]
pub fn synthetic_candles(
    symbol: &str,
    n: usize,
    start_price: f64,
    annual_vol: f64,
    candles_per_year: f64,
    step_min: u32,
    seed: u64,
) -> Vec<Candle> {
    use rand::SeedableRng;
    let dt = 1.0 / candles_per_year;
    let drift = 0.06;
    let mut price = start_price;
    let mut out = Vec::with_capacity(n);
    let step_min = step_min.max(1) as u64;
    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
    let _ = symbol;
    for i in 0..n {
        let z = standard_normal_with(&mut rng);
        price *= ((drift - 0.5 * annual_vol * annual_vol) * dt + annual_vol * z * dt.sqrt()).exp();
        let open = price;
        let intraday = standard_normal_with(&mut rng);
        let close = (price * (1.0 + intraday * 0.004 * dt.sqrt())).max(open * 0.5);
        let high = open.max(close) + unit_with(&mut rng) * 0.002 * price;
        let low = (open.min(close) - unit_with(&mut rng) * 0.002 * price).max(price * 0.01);
        let ts = iso_from_millis(SYNTHETIC_BASE_MS + i as u64 * step_min * 60_000);
        out.push(Candle {
            ts,
            open,
            high,
            low,
            close,
            volume: 100.0 + unit_with(&mut rng) * 900.0,
        });
    }
    out
}

/// Regenerate the exact synthetic series of a run from its stored params.
pub fn synthetic_from_params(symbol: &str, p: &crate::sim::CandleParams) -> Vec<Candle> {
    let cpy = p.candles_per_year.to_f64().max(1.0);
    let step_min = ((365.0 * 24.0 * 60.0) / cpy).round().max(1.0) as u32;
    synthetic_candles(
        symbol,
        p.n as usize,
        p.start_price.to_f64(),
        p.annual_vol.to_f64(),
        cpy,
        step_min,
        p.seed,
    )
}

/// Per-candle sigma from realized log returns (sample stdev, no drift).
/// `0.0` when the series is too short to measure anything.
pub fn realized_sigma(candles: &[Candle]) -> f64 {
    if candles.len() < 3 {
        return 0.0;
    }
    let rets: Vec<f64> = candles
        .windows(2)
        .map(|w| (w[1].close / w[0].close).ln())
        .filter(|r| r.is_finite())
        .collect();
    if rets.len() < 2 {
        return 0.0;
    }
    let mean = rets.iter().sum::<f64>() / rets.len() as f64;
    let var = rets.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / (rets.len() - 1) as f64;
    var.sqrt()
}

/// `count` driftless GBM forward paths of `steps` candles from `start`.
/// Each inner vec starts with `start` and has `steps + 1` points.
pub fn gbm_paths(start: f64, sigma: f64, steps: usize, count: usize) -> Vec<Vec<f64>> {
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let mut price = start;
        let mut path = Vec::with_capacity(steps + 1);
        path.push(start);
        for _ in 0..steps {
            let z = standard_normal();
            price *= (-0.5 * sigma * sigma + sigma * z).exp();
            path.push(price);
        }
        out.push(path);
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

fn describe_rule(r: &Rule, series: &[f64], i: usize) -> String {
    let s = series_for(&r.indicator, series);
    match s.get(i).copied().flatten() {
        Some(v) => format!("{}({v:.4}) {} {}", r.indicator, r.op, r.value),
        None => format!("{} {} {}", r.indicator, r.op, r.value),
    }
}

/// All rules matched at bar `i` → joined descriptions; otherwise `None`.
/// Unknown indicators degrade to `series_for`'s fallback (close series) —
/// a typo must never panic (see the sim-strategy skill).
fn rules_detail(rules: &[Rule], series: &[f64], i: usize) -> Option<String> {
    if rules.is_empty() {
        return None;
    }
    let next = i.saturating_sub(1);
    let mut details = Vec::with_capacity(rules.len());
    for r in rules {
        let s = series_for(&r.indicator, series);
        let cur = s.get(i).copied().flatten()?;
        let prev = s.get(next).copied().flatten()?;
        if !eval_rule(r, cur, prev) {
            return None;
        }
        details.push(describe_rule(r, series, i));
    }
    Some(details.join(" & "))
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
    let mut signals: Vec<SimSignal> = Vec::new();

    for (i, c) in candles.iter().enumerate() {
        let equity = cash_f + qty * c.close;
        max_equity = max_equity.max(equity);
        if max_equity > 0.0 {
            max_dd = max_dd.max((max_equity - equity) / max_equity);
        }
        equity_curve.push(EquityPoint::new(c.ts.clone(), Amount::from_f64(equity)));

        let in_position = qty > 1e-12;

        if in_position {
            let exit_detail = rules_detail(&strategy.exit_rules, &closes, i);
            let sl_tp: Option<(&str, String)> = open_price.and_then(|entry| {
                let pct = (c.close - entry) / entry;
                let sl = strategy
                    .stop_loss_pct
                    .map(|s| pct <= -(s.to_f64() / 100.0))
                    .unwrap_or(false);
                let tp = strategy
                    .take_profit_pct
                    .map(|s| pct >= s.to_f64() / 100.0)
                    .unwrap_or(false);
                let detail = format!(
                    "entry {entry:.4} → close {:.4} ({:.2}%)",
                    c.close,
                    pct * 100.0
                );
                if sl {
                    Some(("stop_loss", detail))
                } else if tp {
                    Some(("take_profit", detail))
                } else {
                    None
                }
            });
            // SL/TP are checked first: a candle can trip both conditions.
            if sl_tp.is_some() || exit_detail.is_some() {
                let (reason, detail) = match sl_tp {
                    Some((reason, detail)) => (reason, detail),
                    None => ("rules", exit_detail.unwrap_or_default()),
                };
                signals.push(SimSignal {
                    ts: c.ts.clone(),
                    side: "exit".into(),
                    reason: reason.into(),
                    detail,
                    price: Amount::from_f64(c.close),
                });
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

        if !in_position && let Some(detail) = rules_detail(&strategy.entry_rules, &closes, i) {
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
                signals.push(SimSignal {
                    ts: c.ts.clone(),
                    side: "entry".into(),
                    reason: "rules".into(),
                    detail,
                    price: Amount::from_f64(c.close),
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
        signals.push(SimSignal {
            ts: c.ts.clone(),
            side: "exit".into(),
            reason: "liquidation".into(),
            detail: format!("end of run, close {:.4}", c.close),
            price: Amount::from_f64(c.close),
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
        signals,
        candle_params: None,
        granularity_min: 60,
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
                volume: 1000.0,
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
        let cs = synthetic_candles("ETH", 100, 3000.0, 0.7, 365.0 * 24.0, 60, 42);
        assert_eq!(cs.len(), 100);
        assert!(cs.iter().all(|c| c.close > 0.0));
    }

    #[test]
    fn synthetic_is_reproducible_for_the_same_seed() {
        let a = synthetic_candles("ETH", 50, 3000.0, 0.7, 365.0 * 24.0, 60, 7);
        let b = synthetic_candles("ETH", 50, 3000.0, 0.7, 365.0 * 24.0, 60, 7);
        let c = synthetic_candles("ETH", 50, 3000.0, 0.7, 365.0 * 24.0, 60, 8);
        assert_eq!(a, b);
        assert!(a.iter().zip(&c).any(|(x, y)| x.close != y.close));
        // deterministic epoch, not `now_utc` — signals must line up on replay
        assert!(a[0].ts.starts_with("2026-01-01T00:00:00"));
    }

    #[test]
    fn synthetic_from_params_round_trips() {
        let params = crate::sim::CandleParams {
            seed: 42,
            n: 30,
            start_price: Amount::parse("100").unwrap(),
            annual_vol: Amount::parse("0.8").unwrap(),
            candles_per_year: Amount::parse("8760").unwrap(),
        };
        let a = synthetic_from_params("BTC", &params);
        let b = synthetic_from_params("BTC", &params);
        assert_eq!(a, b);
        assert_eq!(a.len(), 30);
        // 8760 candles/year → 60-minute step
        assert!(a[1].ts.contains("01:00:00"));
    }

    #[test]
    fn run_records_signals_with_reasons() {
        let s = Strategy {
            name: "trend".into(),
            initial_capital: Amount::parse("1000").unwrap(),
            position_size_pct: Amount::parse("1").unwrap(),
            take_profit_pct: Some(Amount::parse("5").unwrap()),
            entry_rules: vec![Rule {
                indicator: "close".into(),
                field: "close".into(),
                op: "gt".into(),
                value: Amount::parse("0").unwrap(),
            }],
            ..Default::default()
        };
        // rising series → entry then take-profit exit
        let closes: Vec<f64> = (0..60).map(|i| 10.0 + i as f64).collect();
        let run = run_on_candles(&s, &candles_from(&closes));
        assert!(run.trades.iter().any(|t| t.side == "buy"));
        assert!(
            run.signals
                .iter()
                .any(|sig| sig.side == "entry" && sig.reason == "rules")
        );
        assert!(
            run.signals
                .iter()
                .any(|sig| sig.side == "exit" && sig.reason == "take_profit")
        );
        let entry = run.signals.iter().find(|sig| sig.side == "entry").unwrap();
        assert!(entry.detail.contains("close("));
        assert!(entry.detail.contains(" gt "));
    }

    #[test]
    fn realized_sigma_and_gbm_paths() {
        let cs = synthetic_candles("ETH", 200, 3000.0, 0.7, 365.0 * 24.0, 60, 42);
        let sigma = realized_sigma(&cs);
        assert!(sigma > 0.0 && sigma < 0.2);
        assert_eq!(realized_sigma(&cs[..2]), 0.0);
        let paths = gbm_paths(100.0, sigma, 10, 5);
        assert_eq!(paths.len(), 5);
        assert!(
            paths
                .iter()
                .all(|p| p.len() == 11 && (p[0] - 100.0).abs() < 1e-9)
        );
    }
}
