use tauri::{AppHandle, State};

use trade_core::Amount;
use trade_core::calc::invest::{
    self, InvestCompare, InvestInput, InvestResult, SolveResult, SolveUnknown,
};
use trade_core::calc::{self, CalcInput, CalcResult};
use trade_core::calendar::now_secs;
use trade_core::model::planning::{CalcScenario, ScenarioSource};
use trade_core::planning::scenarios as scenario_reads;
use trade_core::service::{CalcScenarioDeleted, CalcScenarioDraft, ScenarioPrefill};
use trade_core::usecases::planning::scenarios;

use crate::commands::AppState;
use crate::commands::app_settings::current_zone;
use crate::commands::{with_vault, with_vault_mut};
use crate::ipc::IpcError;
use crate::ipc::commands::ipc_commands;

/// Pure calculator preview (converter / position). Needs no vault state —
/// the whole computation happens in `trade_core::calc`.
#[tauri::command]
pub async fn calc_preview(
    input: CalcInput,
    _state: State<'_, AppState>,
) -> Result<CalcResult, IpcError> {
    Ok(calc::preview(&input))
}

// -- investment calculator (plan A) ----------------------------------------------

#[tauri::command]
pub async fn calc_invest_simulate(input: InvestInput) -> Result<InvestResult, IpcError> {
    invest::simulate(&input).map_err(IpcError::from)
}

#[tauri::command]
pub async fn calc_invest_solve(
    input: InvestInput,
    target: Amount,
    unknown: SolveUnknown,
) -> Result<SolveResult, IpcError> {
    invest::solve(&input, target, unknown).map_err(IpcError::from)
}

/// "What pays off more?": the plan, a higher rate and larger contributions.
#[tauri::command]
pub async fn calc_invest_compare(input: InvestInput) -> Result<InvestCompare, IpcError> {
    invest::compare(&input).map_err(IpcError::from)
}

/// The largest withdrawal the plan pays to the end of its term, when the asked one runs out.
#[tauri::command]
pub async fn calc_invest_safe_withdrawal(input: InvestInput) -> Result<Option<Amount>, IpcError> {
    invest::safe_withdrawal(&input).map_err(IpcError::from)
}

#[tauri::command]
pub async fn calc_scenarios_list(
    state: State<'_, AppState>,
) -> Result<Vec<CalcScenario>, IpcError> {
    with_vault(&state, |vault| {
        Ok(scenario_reads::calc_scenarios(&vault.doc))
    })
}

#[tauri::command]
pub async fn calc_scenario_save(
    draft: CalcScenarioDraft,
    state: State<'_, AppState>,
) -> Result<CalcScenario, IpcError> {
    with_vault_mut(&state, |vault| scenarios::save_calc_scenario(vault, draft))
}

#[tauri::command]
pub async fn calc_scenario_delete(
    id: String,
    state: State<'_, AppState>,
) -> Result<CalcScenarioDeleted, IpcError> {
    with_vault_mut(&state, |vault| scenarios::delete_calc_scenario(vault, &id))
}

/// Undo of [`calc_scenario_delete`].
#[tauri::command]
pub async fn calc_scenario_restore(
    deleted: CalcScenarioDeleted,
    state: State<'_, AppState>,
) -> Result<CalcScenario, IpcError> {
    with_vault_mut(&state, |vault| {
        scenarios::restore_calc_scenario(vault, deleted)
    })
}

/// "From your data": start amount, rate and contribution from the user's wallets
/// and income. Saves nothing.
#[tauri::command]
pub async fn calc_scenario_prefill(
    source: ScenarioSource,
    currency: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ScenarioPrefill, IpcError> {
    let zone = current_zone(&app);
    with_vault(&state, |vault| {
        scenario_reads::scenario_prefill(&vault.books(), &source, &currency, now_secs(), &zone)
    })
}

ipc_commands!(calc_commands, CALC_NAMES, commands::calc: [calc_preview, calc_invest_simulate, calc_invest_solve, calc_invest_compare, calc_invest_safe_withdrawal, calc_scenarios_list, calc_scenario_save, calc_scenario_delete, calc_scenario_restore, calc_scenario_prefill]);
