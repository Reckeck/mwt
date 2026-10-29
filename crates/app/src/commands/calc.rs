use tauri::{AppHandle, State};

use trade_core::Amount;
use trade_core::calc::invest::{
    self, InvestCompare, InvestInput, InvestResult, SolveResult, SolveUnknown,
};
use trade_core::calc::{self, CalcInput, CalcResult};
use trade_core::calendar::now_secs;
use trade_core::model::planning::{CalcScenario, ScenarioSource};
use trade_core::service::{CalcScenarioDeleted, CalcScenarioDraft, ScenarioPrefill};

use crate::commands::AppState;
use crate::commands::app_settings::current_zone;
use crate::commands::err_to_string;
use crate::commands::planning::{with_vault, with_vault_mut};

/// Pure calculator preview (converter / position). Needs no vault state —
/// the whole computation happens in `trade_core::calc`.
#[tauri::command]
pub async fn calc_preview(
    input: CalcInput,
    _state: State<'_, AppState>,
) -> Result<CalcResult, String> {
    Ok(calc::preview(&input))
}

// -- investment calculator (plan A) ----------------------------------------------

#[tauri::command]
pub async fn calc_invest_simulate(input: InvestInput) -> Result<InvestResult, String> {
    invest::simulate(&input).map_err(err_to_string)
}

#[tauri::command]
pub async fn calc_invest_solve(
    input: InvestInput,
    target: Amount,
    unknown: SolveUnknown,
) -> Result<SolveResult, String> {
    invest::solve(&input, target, unknown).map_err(err_to_string)
}

/// "What pays off more?": the plan, a higher rate and larger contributions.
#[tauri::command]
pub async fn calc_invest_compare(input: InvestInput) -> Result<InvestCompare, String> {
    invest::compare(&input).map_err(err_to_string)
}

/// The largest withdrawal the plan pays to the end of its term, when the asked one runs out.
#[tauri::command]
pub async fn calc_invest_safe_withdrawal(input: InvestInput) -> Result<Option<Amount>, String> {
    invest::safe_withdrawal(&input).map_err(err_to_string)
}

#[tauri::command]
pub async fn calc_scenarios_list(state: State<'_, AppState>) -> Result<Vec<CalcScenario>, String> {
    with_vault(&state, |vault| Ok(vault.calc_scenarios()))
}

#[tauri::command]
pub async fn calc_scenario_save(
    draft: CalcScenarioDraft,
    state: State<'_, AppState>,
) -> Result<CalcScenario, String> {
    with_vault_mut(&state, |vault| vault.save_calc_scenario(draft))
}

#[tauri::command]
pub async fn calc_scenario_delete(
    id: String,
    state: State<'_, AppState>,
) -> Result<CalcScenarioDeleted, String> {
    with_vault_mut(&state, |vault| vault.delete_calc_scenario(&id))
}

/// Undo of [`calc_scenario_delete`].
#[tauri::command]
pub async fn calc_scenario_restore(
    deleted: CalcScenarioDeleted,
    state: State<'_, AppState>,
) -> Result<CalcScenario, String> {
    with_vault_mut(&state, |vault| vault.restore_calc_scenario(deleted))
}

/// "From your data": start amount, rate and contribution from the user's wallets
/// and income. Saves nothing.
#[tauri::command]
pub async fn calc_scenario_prefill(
    source: ScenarioSource,
    currency: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ScenarioPrefill, String> {
    let zone = current_zone(&app);
    with_vault(&state, |vault| {
        vault.scenario_prefill(&source, &currency, now_secs(), &zone)
    })
}
