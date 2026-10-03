use reqwest::Client;
use tauri::State;

use trade_core::ipc_contract::{RunStrategyInput, SaveStrategyInput};
use trade_core::sim::engine::{fetch_candles, run_on_candles, synthetic_candles};
use trade_core::sim::{DataSource, SimRun, Strategy};

use crate::commands::{AppState, err_to_string, require_vault};

#[tauri::command]
pub async fn save_strategy(
    input: SaveStrategyInput,
    state: State<'_, AppState>,
) -> Result<Strategy, String> {
    let mut g = require_vault(&state).map_err(err_to_string)?;
    let v = g.as_mut().unwrap();
    let mut strategy = input.strategy;
    strategy.id = uuid::Uuid::new_v4().to_string();
    strategy.created_at = trade_core::model::vault::now_iso();
    // never overwrite by name collision; append
    v.doc.sim.strategies.retain(|s| s.id != strategy.id);
    v.doc.sim.strategies.push(strategy.clone());
    v.persist().map_err(crate::commands::err_to_string)?;
    Ok(strategy)
}

#[tauri::command]
pub async fn list_strategies(state: State<'_, AppState>) -> Result<Vec<Strategy>, String> {
    let g = require_vault(&state).map_err(err_to_string)?;
    Ok(g.as_ref().unwrap().doc.sim.strategies.clone())
}

#[tauri::command]
pub async fn delete_strategy(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut g = require_vault(&state).map_err(err_to_string)?;
    let v = g.as_mut().unwrap();
    let before = v.doc.sim.strategies.len();
    v.doc.sim.strategies.retain(|s| s.id != id);
    v.doc.sim.runs.retain(|r| r.strategy_id != id);
    if v.doc.sim.strategies.len() == before {
        return Err("MISSING: strategy".into());
    }
    v.persist().map_err(crate::commands::err_to_string)?;
    Ok(())
}

#[tauri::command]
pub async fn list_runs(state: State<'_, AppState>) -> Result<Vec<SimRun>, String> {
    let g = require_vault(&state).map_err(err_to_string)?;
    Ok(g.as_ref().unwrap().doc.sim.runs.clone())
}

#[tauri::command]
pub async fn run_strategy(
    input: RunStrategyInput,
    state: State<'_, AppState>,
) -> Result<SimRun, String> {
    let strategy = {
        let g = require_vault(&state).map_err(err_to_string)?;
        g.as_ref()
            .unwrap()
            .doc
            .sim
            .strategies
            .iter()
            .find(|s| s.id == input.strategy_id)
            .cloned()
            .ok_or_else(|| "MISSING: strategy".to_string())?
    };

    let candles = match strategy.source {
        DataSource::Real => {
            let client = Client::builder()
                .timeout(std::time::Duration::from_secs(20))
                .build()
                .map_err(|e| e.to_string())?;
            let symbol = if strategy.symbol.is_empty() {
                "BTC".into()
            } else {
                strategy.symbol.to_uppercase()
            };
            fetch_candles(
                &symbol,
                input.granularity_min.max(1),
                input.candles.clamp(50, 5000),
                &client,
            )
            .await
            .map_err(crate::commands::err_to_string)?
        }
        DataSource::Synthetic => {
            let n = input.candles.max(50) as usize;
            synthetic_candles(&strategy.symbol, n, 100.0, 0.8, 365.0 * 24.0)
        }
    };

    let run = run_on_candles(&strategy, &candles);
    let mut g = require_vault(&state).map_err(err_to_string)?;
    let v = g.as_mut().unwrap();
    v.doc.sim.runs.insert(0, run.clone());
    if v.doc.sim.runs.len() > 200 {
        v.doc.sim.runs.truncate(200);
    }
    v.persist().map_err(crate::commands::err_to_string)?;
    Ok(run)
}
