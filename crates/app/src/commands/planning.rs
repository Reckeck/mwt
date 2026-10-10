//! Planning (plan §7): automations, budgets, the daily budget, month totals,
//! goals, reminders, price alerts and what is coming up. Every computation
//! lives in core; days are cut in the zone from [`current_zone`].

use tauri::{AppHandle, State};

use trade_core::calendar::now_secs;
use trade_core::ipc_contract::{
    AutomationInput, BudgetInput, GoalInput, PriceAlertInput, ReminderInput, UpdateAutomationInput,
    UpdateBudgetInput, UpdateGoalInput, UpdatePriceAlertInput, UpdateReminderInput,
};
use trade_core::model::planning::{Automation, AutomationRun, Budget, Goal, PriceAlert, Reminder};
use trade_core::model::transaction::Transaction;
use trade_core::service::{
    BudgetStatus, DailyBudget, GoalCalendar, GoalProgress, MonthSummary, UnlockedVault,
    UpcomingItem,
};

use crate::commands::app_settings::current_zone;
use crate::commands::{AppState, err_to_string, require_vault};

/// Run `read` against the open vault.
fn with_vault<T>(
    state: &State<'_, AppState>,
    read: impl FnOnce(&UnlockedVault) -> trade_core::Result<T>,
) -> Result<T, String> {
    let guard = require_vault(state).map_err(err_to_string)?;
    read(guard.as_ref().unwrap()).map_err(err_to_string)
}

/// Run `write` against the open vault.
fn with_vault_mut<T>(
    state: &State<'_, AppState>,
    write: impl FnOnce(&mut UnlockedVault) -> trade_core::Result<T>,
) -> Result<T, String> {
    let mut guard = require_vault(state).map_err(err_to_string)?;
    write(guard.as_mut().unwrap()).map_err(err_to_string)
}

// -- automations -----------------------------------------------------------------

#[tauri::command]
pub async fn list_automations(state: State<'_, AppState>) -> Result<Vec<Automation>, String> {
    with_vault(&state, |vault| Ok(vault.automations()))
}

#[tauri::command]
pub async fn create_automation(
    input: AutomationInput,
    state: State<'_, AppState>,
) -> Result<Automation, String> {
    let draft = input.to_draft().map_err(err_to_string)?;
    with_vault_mut(&state, |vault| vault.create_automation(draft))
}

#[tauri::command]
pub async fn update_automation(
    input: UpdateAutomationInput,
    state: State<'_, AppState>,
) -> Result<Automation, String> {
    let draft = input.automation.to_draft().map_err(err_to_string)?;
    with_vault_mut(&state, |vault| vault.update_automation(&input.id, draft))
}

#[tauri::command]
pub async fn delete_automation(id: String, state: State<'_, AppState>) -> Result<(), String> {
    with_vault_mut(&state, |vault| vault.delete_automation(&id))
}

/// «Создать сейчас»: same funds check as the schedule; a shortfall is
/// `INSUFFICIENT_FUNDS`.
#[tauri::command]
pub async fn run_automation(id: String, state: State<'_, AppState>) -> Result<Transaction, String> {
    with_vault_mut(&state, |vault| vault.run_automation(&id, now_secs()))
}

#[tauri::command]
pub async fn list_automation_runs(
    automation_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<AutomationRun>, String> {
    with_vault(&state, |vault| {
        Ok(vault.automation_runs(automation_id.as_deref()))
    })
}

#[tauri::command]
pub async fn retry_automation_run(
    run_id: String,
    state: State<'_, AppState>,
) -> Result<Transaction, String> {
    with_vault_mut(&state, |vault| {
        vault.retry_automation_run(&run_id, now_secs())
    })
}

#[tauri::command]
pub async fn skip_automation_run(
    run_id: String,
    state: State<'_, AppState>,
) -> Result<AutomationRun, String> {
    with_vault_mut(&state, |vault| vault.skip_automation_run(&run_id))
}

// -- budgets ---------------------------------------------------------------------

#[tauri::command]
pub async fn list_budgets(state: State<'_, AppState>) -> Result<Vec<Budget>, String> {
    with_vault(&state, |vault| Ok(vault.budgets()))
}

#[tauri::command]
pub async fn create_budget(
    input: BudgetInput,
    state: State<'_, AppState>,
) -> Result<Budget, String> {
    with_vault_mut(&state, |vault| vault.create_budget(input.into()))
}

#[tauri::command]
pub async fn update_budget(
    input: UpdateBudgetInput,
    state: State<'_, AppState>,
) -> Result<Budget, String> {
    with_vault_mut(&state, |vault| {
        vault.update_budget(&input.id, input.budget.into())
    })
}

#[tauri::command]
pub async fn delete_budget(id: String, state: State<'_, AppState>) -> Result<(), String> {
    with_vault_mut(&state, |vault| vault.delete_budget(&id))
}

#[tauri::command]
pub async fn get_budget_status(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<BudgetStatus>, String> {
    let zone = current_zone(&app);
    with_vault(&state, |vault| Ok(vault.budget_status(now_secs(), &zone)))
}

#[tauri::command]
pub async fn get_daily_budget(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DailyBudget, String> {
    let zone = current_zone(&app);
    with_vault(&state, |vault| Ok(vault.daily_budget(now_secs(), &zone)))
}

/// Totals of a `YYYY-MM` month; the current month when omitted.
#[tauri::command]
pub async fn get_month_summary(
    month: Option<String>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<MonthSummary, String> {
    let zone = current_zone(&app);
    let month = month.unwrap_or_else(|| {
        let date = trade_core::calendar::Calendar::new(&zone).date_of(now_secs());
        trade_core::calendar::month_key(date)
    });
    with_vault(&state, |vault| vault.month_summary(&month, &zone))
}

// -- goals -----------------------------------------------------------------------

#[tauri::command]
pub async fn list_goals(state: State<'_, AppState>) -> Result<Vec<Goal>, String> {
    with_vault(&state, |vault| Ok(vault.goals()))
}

#[tauri::command]
pub async fn create_goal(input: GoalInput, state: State<'_, AppState>) -> Result<Goal, String> {
    with_vault_mut(&state, |vault| vault.create_goal(input.into()))
}

#[tauri::command]
pub async fn update_goal(
    input: UpdateGoalInput,
    state: State<'_, AppState>,
) -> Result<Goal, String> {
    with_vault_mut(&state, |vault| {
        vault.update_goal(&input.id, input.goal.into())
    })
}

#[tauri::command]
pub async fn delete_goal(id: String, state: State<'_, AppState>) -> Result<(), String> {
    with_vault_mut(&state, |vault| vault.delete_goal(&id))
}

#[tauri::command]
pub async fn get_goal_progress(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<GoalProgress>, String> {
    let zone = current_zone(&app);
    with_vault(&state, |vault| Ok(vault.goal_progress(now_secs(), &zone)))
}

#[tauri::command]
pub async fn get_goal_calendar(
    year: i32,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<GoalCalendar, String> {
    let zone = current_zone(&app);
    with_vault(&state, |vault| vault.goal_calendar(year, now_secs(), &zone))
}

// -- reminders -------------------------------------------------------------------

#[tauri::command]
pub async fn list_reminders(state: State<'_, AppState>) -> Result<Vec<Reminder>, String> {
    with_vault(&state, |vault| Ok(vault.reminders()))
}

#[tauri::command]
pub async fn create_reminder(
    input: ReminderInput,
    state: State<'_, AppState>,
) -> Result<Reminder, String> {
    with_vault_mut(&state, |vault| {
        vault.create_reminder(&input.text, &input.due_at)
    })
}

#[tauri::command]
pub async fn update_reminder(
    input: UpdateReminderInput,
    state: State<'_, AppState>,
) -> Result<Reminder, String> {
    with_vault_mut(&state, |vault| {
        vault.update_reminder(&input.id, &input.reminder.text, &input.reminder.due_at)
    })
}

#[tauri::command]
pub async fn delete_reminder(id: String, state: State<'_, AppState>) -> Result<(), String> {
    with_vault_mut(&state, |vault| vault.delete_reminder(&id))
}

#[tauri::command]
pub async fn set_reminder_done(
    id: String,
    done: bool,
    state: State<'_, AppState>,
) -> Result<Reminder, String> {
    with_vault_mut(&state, |vault| vault.set_reminder_done(&id, done))
}

// -- price alerts ----------------------------------------------------------------

#[tauri::command]
pub async fn list_price_alerts(state: State<'_, AppState>) -> Result<Vec<PriceAlert>, String> {
    with_vault(&state, |vault| Ok(vault.price_alerts()))
}

#[tauri::command]
pub async fn create_price_alert(
    input: PriceAlertInput,
    state: State<'_, AppState>,
) -> Result<PriceAlert, String> {
    with_vault_mut(&state, |vault| vault.create_price_alert(input.into()))
}

#[tauri::command]
pub async fn update_price_alert(
    input: UpdatePriceAlertInput,
    state: State<'_, AppState>,
) -> Result<PriceAlert, String> {
    with_vault_mut(&state, |vault| {
        vault.update_price_alert(&input.id, input.alert.into())
    })
}

#[tauri::command]
pub async fn delete_price_alert(id: String, state: State<'_, AppState>) -> Result<(), String> {
    with_vault_mut(&state, |vault| vault.delete_price_alert(&id))
}

// -- agenda ----------------------------------------------------------------------

/// What is coming up in the next `days` days (default 30).
#[tauri::command]
pub async fn get_upcoming(
    days: Option<u32>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<UpcomingItem>, String> {
    let zone = current_zone(&app);
    with_vault(&state, |vault| {
        Ok(vault.upcoming(days.unwrap_or(30).min(366), now_secs(), &zone))
    })
}
