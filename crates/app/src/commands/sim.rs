use reqwest::Client;
use tauri::State;

use trade_core::ipc_contract::{
    CandlePoint, GbmPathPoint, GbmPathsInput, GbmPathsOutput, GetRunCandlesInput,
    GetRunCandlesOutput, RunStrategyInput, SaveStrategyInput,
};
use trade_core::sim::engine::{fetch_candles, fetch_candles_between, run_on_candles};
use trade_core::sim::{DataSource, SimRun, Strategy};

use crate::commands::{AppState, err_to_string, require_vault};

#[tauri::command]
pub async fn save_strategy(
    input: SaveStrategyInput,
    state: State<'_, AppState>,
) -> Result<Strategy, String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    let vault = guard.as_mut().unwrap();
    let mut strategy = input.strategy;
    strategy.id = uuid::Uuid::new_v4().to_string();
    strategy.created_at = trade_core::model::vault::now_iso();
    // never overwrite by name collision; append
    vault
        .doc
        .sim
        .strategies
        .retain(|candidate| candidate.id != strategy.id);
    vault.doc.sim.strategies.push(strategy.clone());
    vault.persist().map_err(crate::commands::err_to_string)?;
    Ok(strategy)
}

#[tauri::command]
pub async fn list_strategies(state: State<'_, AppState>) -> Result<Vec<Strategy>, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    Ok(guard.as_ref().unwrap().doc.sim.strategies.clone())
}

#[tauri::command]
pub async fn delete_strategy(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    let vault = guard.as_mut().unwrap();
    let before = vault.doc.sim.strategies.len();
    vault
        .doc
        .sim
        .strategies
        .retain(|candidate| candidate.id != id);
    vault.doc.sim.runs.retain(|run| run.strategy_id != id);
    if vault.doc.sim.strategies.len() == before {
        return Err("MISSING: strategy".into());
    }
    vault.persist().map_err(crate::commands::err_to_string)?;
    Ok(())
}

#[tauri::command]
pub async fn list_runs(state: State<'_, AppState>) -> Result<Vec<SimRun>, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    Ok(guard.as_ref().unwrap().doc.sim.runs.clone())
}

#[tauri::command]
pub async fn run_strategy(
    input: RunStrategyInput,
    state: State<'_, AppState>,
) -> Result<SimRun, String> {
    let strategy = {
        let guard = require_vault(&state).map_err(err_to_string)?;
        guard
            .as_ref()
            .unwrap()
            .doc
            .sim
            .strategies
            .iter()
            .find(|candidate| candidate.id == input.strategy_id)
            .cloned()
            .ok_or_else(|| "MISSING: strategy".to_string())?
    };

    let (candles, candle_params, granularity) = match strategy.source {
        DataSource::Real => {
            let client = Client::builder()
                .timeout(std::time::Duration::from_secs(20))
                .build()
                .map_err(|err| err.to_string())?;
            let symbol = if strategy.symbol.is_empty() {
                "BTC".into()
            } else {
                strategy.symbol.to_uppercase()
            };
            let granularity = input.granularity_min.max(1);
            let candles =
                fetch_candles(&symbol, granularity, input.candles.clamp(50, 5000), &client)
                    .await
                    .map_err(crate::commands::err_to_string)?;
            (candles, None, granularity)
        }
        DataSource::Synthetic => {
            let count = input.candles.clamp(50, 20_000);
            let granularity = input.granularity_min.max(1);
            let params = trade_core::sim::CandleParams {
                seed: uuid::Uuid::new_v4().as_u128() as u64,
                count,
                start_price: trade_core::Amount::parse("100").unwrap(),
                annual_vol: trade_core::Amount::parse("0.8").unwrap(),
                candles_per_year: trade_core::Amount::from_f64(
                    365.0 * 24.0 * 60.0 / granularity as f64,
                ),
            };
            let candles = trade_core::sim::engine::synthetic_from_params(&strategy.symbol, &params);
            (candles, Some(params), granularity)
        }
    };

    let mut run = run_on_candles(&strategy, &candles);
    // replay metadata: lets get_run_candles/get_gbm_paths rebuild this exact series
    run.granularity_min = granularity;
    run.candle_params = candle_params;
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    let vault = guard.as_mut().unwrap();
    vault.doc.sim.runs.insert(0, run.clone());
    if vault.doc.sim.runs.len() > 200 {
        vault.doc.sim.runs.truncate(200);
    }
    vault.persist().map_err(crate::commands::err_to_string)?;
    Ok(run)
}

// -- run replay (D2.2) -------------------------------------------------------

fn candle_point(candle: trade_core::sim::engine::Candle) -> CandlePoint {
    CandlePoint {
        ts: candle.ts,
        open: candle.open.to_string(),
        high: candle.high.to_string(),
        low: candle.low.to_string(),
        close: candle.close.to_string(),
        volume: candle.volume.to_string(),
    }
}

/// Clone a run and its strategy out of the vault — the guard must not live
/// across `.await` (non-`Send` future, see AGENTS.md rule 3).
fn load_run(state: &State<'_, AppState>, run_id: &str) -> Result<(Strategy, SimRun), String> {
    let guard = require_vault(state).map_err(err_to_string)?;
    let doc = &guard.as_ref().unwrap().doc;
    let run = doc
        .sim
        .runs
        .iter()
        .find(|candidate| candidate.id == run_id)
        .cloned()
        .ok_or_else(|| "MISSING: run".to_string())?;
    let strategy = doc
        .sim
        .strategies
        .iter()
        .find(|candidate| candidate.id == run.strategy_id)
        .cloned()
        .ok_or_else(|| "MISSING: strategy".to_string())?;
    Ok((strategy, run))
}

fn replay_client() -> Result<Client, String> {
    Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|err| err.to_string())
}

fn run_range(run: &SimRun) -> Result<(u64, u64), String> {
    let start = trade_core::sim::engine::rfc3339_to_millis(&run.started_at)
        .ok_or_else(|| "UNSUPPORTED: bad run timestamp".to_string())?;
    let end = trade_core::sim::engine::rfc3339_to_millis(&run.finished_at)
        .ok_or_else(|| "UNSUPPORTED: bad run timestamp".to_string())?;
    Ok((start, end.max(start + 1)))
}

fn run_symbol(strategy: &Strategy) -> String {
    if strategy.symbol.trim().is_empty() {
        "BTC".into()
    } else {
        strategy.symbol.trim().to_uppercase()
    }
}

fn granularity_of(run: &SimRun) -> u32 {
    if run.granularity_min == 0 {
        60
    } else {
        run.granularity_min
    }
}

#[tauri::command]
pub async fn get_run_candles(
    input: GetRunCandlesInput,
    state: State<'_, AppState>,
) -> Result<GetRunCandlesOutput, String> {
    let (strategy, run) = load_run(&state, &input.run_id)?;
    let granularity = granularity_of(&run);

    let candles = match (&run.candle_params, run.synthetic) {
        // synthetic: regenerate the exact stored series from its seed
        (Some(params), _) => {
            trade_core::sim::engine::synthetic_from_params(&strategy.symbol, params)
        }
        // real: re-fetch the run's historical range from Binance
        (None, false) => {
            let client = replay_client()?;
            let (start, end) = run_range(&run)?;
            fetch_candles_between(&run_symbol(&strategy), granularity, start, end, &client)
                .await
                .map_err(crate::commands::err_to_string)?
        }
        // pre-v3 synthetic run has no replay key — cannot rebuild candles
        (None, true) => {
            return Err("UNSUPPORTED: run has no candle seed (pre-v3)".into());
        }
    };

    Ok(GetRunCandlesOutput {
        candles: candles.into_iter().map(candle_point).collect(),
    })
}

#[tauri::command]
pub async fn get_gbm_paths(
    input: GbmPathsInput,
    state: State<'_, AppState>,
) -> Result<GbmPathsOutput, String> {
    let (strategy, run) = load_run(&state, &input.run_id)?;
    let granularity = granularity_of(&run);
    let path_count = input.paths.clamp(1, 20) as usize;
    let horizon = input.horizon.clamp(1, 500) as usize;

    let (start, sigma) = match (&run.candle_params, run.synthetic) {
        (Some(params), _) => {
            // synthetic: sigma from the GBM params themselves
            let candles = trade_core::sim::engine::synthetic_from_params(&strategy.symbol, params);
            let start = candles
                .last()
                .map(|candle| candle.close)
                .unwrap_or(params.start_price.to_f64());
            let cpy = params.candles_per_year.to_f64().max(1.0);
            (start, params.annual_vol.to_f64() / cpy.sqrt())
        }
        (None, false) => {
            // real: measure realized volatility on the run's range
            let client = replay_client()?;
            let (from, to) = run_range(&run)?;
            let candles =
                fetch_candles_between(&run_symbol(&strategy), granularity, from, to, &client)
                    .await
                    .map_err(crate::commands::err_to_string)?;
            let sigma = trade_core::sim::engine::realized_sigma(&candles);
            if sigma <= 0.0 {
                return Err("UNSUPPORTED: not enough candles to estimate volatility".into());
            }
            let start = candles
                .last()
                .map(|candle| candle.close)
                .ok_or_else(|| "MISSING: run candles".to_string())?;
            (start, sigma)
        }
        (None, true) => {
            return Err("UNSUPPORTED: run has no candle seed (pre-v3)".into());
        }
    };

    let base_ms = trade_core::sim::engine::rfc3339_to_millis(&run.finished_at)
        .ok_or_else(|| "UNSUPPORTED: bad run timestamp".to_string())?;
    let step_ms = granularity as u64 * 60_000;
    let raw = trade_core::sim::engine::gbm_paths(start, sigma, horizon, path_count);
    let paths = raw
        .into_iter()
        .map(|path| {
            path.into_iter()
                .enumerate()
                .map(|(index, price)| GbmPathPoint {
                    ts: trade_core::sim::engine::iso_from_millis(base_ms + index as u64 * step_ms),
                    price: price.to_string(),
                })
                .collect()
        })
        .collect();
    Ok(GbmPathsOutput { paths })
}
