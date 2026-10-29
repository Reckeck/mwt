//! Planning (plan §7): automations, budgets, the daily budget, month totals,
//! goals, reminders, price alerts and what is coming up. Every computation
//! lives in core; days are cut in the zone from [`current_zone`].

use tauri::{AppHandle, State};

use trade_core::Amount;
use trade_core::calendar::now_secs;
use trade_core::ipc_contract::{
    AdjustPointsInput, AutomationInput, BudgetInput, ConfirmEstimatedInput,
    CreateLoyaltyProgramInput, GoalInput, MeterReadingInput, PriceAlertInput, ReminderInput,
    SetDepositTermsInput, SetLoyaltyInfoInput, SimulateAutomationsInput, SnoozeReminderInput,
    UpdateAutomationInput, UpdateBudgetInput, UpdateGoalInput, UpdateMeterReadingInput,
    UpdatePriceAlertInput, UpdateReminderInput, UpdateUtilityInput, UtilityInput,
};
use trade_core::model::planning::{
    Automation, AutomationRun, Budget, EmergencyFund, Goal, MeterReading, PriceAlert, Reminder,
    UtilityItem,
};
use trade_core::model::transaction::Transaction;
use trade_core::service::{
    AutomationSimulation, BudgetStatus, CashbackReport, DailyBudget, DepositReport,
    EmergencyStatus, GoalCalendar, GoalDeleted, GoalProgress, InterestTaxReport, MonthSummary,
    PayUtilityBillInput, PropertySummary, UnlockedVault, UpcomingItem, UtilityBill,
    UtilityPaymentDraft, UtilityTrend,
};
use trade_core::workflow::Workflow;

use crate::commands::app_settings::{current_settings, current_zone};
use crate::commands::{AppState, err_to_string, require_vault};

/// Run `read` against the open vault.
pub(crate) fn with_vault<T>(
    state: &State<'_, AppState>,
    read: impl FnOnce(&UnlockedVault) -> trade_core::Result<T>,
) -> Result<T, String> {
    let guard = require_vault(state).map_err(err_to_string)?;
    read(guard.as_ref().unwrap()).map_err(err_to_string)
}

/// Run `write` against the open vault.
pub(crate) fn with_vault_mut<T>(
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

/// "Create now": same funds check as the schedule; a shortfall is
/// `INSUFFICIENT_FUNDS`.
#[tauri::command]
pub async fn run_automation(id: String, state: State<'_, AppState>) -> Result<Transaction, String> {
    with_vault_mut(&state, |vault| vault.run_automation(&id, now_secs()))
}

/// What the automations would do over the next days, worked out on a sandbox
/// copy of the vault: nothing is written.
#[tauri::command]
pub async fn simulate_automations(
    input: SimulateAutomationsInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<AutomationSimulation, String> {
    let zone = current_zone(&app);
    with_vault(&state, |vault| {
        vault.simulate_automations(
            input.automation_id.as_deref(),
            input.days,
            now_secs(),
            &zone,
        )
    })
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
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Transaction, String> {
    let zone = current_zone(&app);
    with_vault_mut(&state, |vault| {
        vault.retry_automation_run(&run_id, now_secs(), &zone)
    })
}

/// The real amount of an operation an automation booked approximately (a
/// salary that differs every time); it makes the next estimate.
#[tauri::command]
pub async fn confirm_estimated_amount(
    input: ConfirmEstimatedInput,
    state: State<'_, AppState>,
) -> Result<Transaction, String> {
    let amount = Amount::parse(&input.amount).map_err(err_to_string)?;
    with_vault_mut(&state, |vault| {
        vault.confirm_estimated_amount(&input.tx_id, amount)
    })
}

/// "Make an ordinary workflow": the automation's workflow is edited in the
/// editor from now on.
#[tauri::command]
pub async fn detach_automation(id: String, state: State<'_, AppState>) -> Result<Workflow, String> {
    with_vault_mut(&state, |vault| vault.detach_automation(&id))
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
pub async fn delete_goal(id: String, state: State<'_, AppState>) -> Result<GoalDeleted, String> {
    with_vault_mut(&state, |vault| vault.delete_goal(&id, now_secs()))
}

/// Undo of [`delete_goal`].
#[tauri::command]
pub async fn restore_goal(
    deleted: GoalDeleted,
    state: State<'_, AppState>,
) -> Result<Goal, String> {
    with_vault_mut(&state, |vault| vault.restore_goal(deleted, now_secs()))
}

#[tauri::command]
pub async fn get_goal_progress(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<GoalProgress>, String> {
    let zone = current_zone(&app);
    with_vault(&state, |vault| Ok(vault.goal_progress(now_secs(), &zone)))
}

/// A goal draft that reaches what a calculator scenario ends with; the goal
/// form opens with it.
#[tauri::command]
pub async fn goal_from_scenario(
    id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<GoalInput, String> {
    let zone = current_zone(&app);
    with_vault(&state, |vault| {
        vault.goal_from_scenario(&id, now_secs(), &zone)
    })
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

// -- emergency fund --------------------------------------------------------------

/// The emergency fund with how many months of mandatory spending it covers.
#[tauri::command]
pub async fn emergency_fund_get(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<EmergencyStatus, String> {
    let zone = current_zone(&app);
    let inflation = current_settings(&app).inflation;
    with_vault(&state, |vault| {
        Ok(vault.emergency_fund_status(now_secs(), &zone, &inflation))
    })
}

#[tauri::command]
pub async fn emergency_fund_set(
    fund: EmergencyFund,
    state: State<'_, AppState>,
) -> Result<EmergencyFund, String> {
    with_vault_mut(&state, |vault| vault.set_emergency_fund(fund))
}

/// Add a savings account to the fund or take it out.
#[tauri::command]
pub async fn emergency_fund_toggle(
    store_id: String,
    on: bool,
    state: State<'_, AppState>,
) -> Result<EmergencyFund, String> {
    with_vault_mut(&state, |vault| vault.emergency_fund_toggle(&store_id, on))
}

// -- reminders -------------------------------------------------------------------

/// Every reminder; the automatic ones are brought in step with the data first.
#[tauri::command]
pub async fn list_reminders(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<Reminder>, String> {
    let zone = current_zone(&app);
    let ru = crate::scheduler::russian(&app);
    with_vault_mut(&state, |vault| {
        vault.sync_auto_reminders(now_secs(), &zone, ru)?;
        Ok(vault.reminders())
    })
}

#[tauri::command]
pub async fn create_reminder(
    input: ReminderInput,
    state: State<'_, AppState>,
) -> Result<Reminder, String> {
    with_vault_mut(&state, |vault| {
        vault.create_reminder_paying(&input.text, &input.due_at, input.repeat, input.pay)
    })
}

#[tauri::command]
pub async fn update_reminder(
    input: UpdateReminderInput,
    state: State<'_, AppState>,
) -> Result<Reminder, String> {
    with_vault_mut(&state, |vault| {
        vault.update_reminder_paying(
            &input.id,
            &input.reminder.text,
            &input.reminder.due_at,
            input.reminder.repeat,
            input.reminder.pay,
        )
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

/// Stop a repeating reminder for good.
#[tauri::command]
pub async fn finish_reminder_series(
    id: String,
    state: State<'_, AppState>,
) -> Result<Reminder, String> {
    with_vault_mut(&state, |vault| vault.finish_reminder_series(&id))
}

/// Put a reminder off until a moment.
#[tauri::command]
pub async fn snooze_reminder(
    input: SnoozeReminderInput,
    state: State<'_, AppState>,
) -> Result<Reminder, String> {
    with_vault_mut(&state, |vault| {
        vault.snooze_reminder(&input.id, &input.until)
    })
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

/// "Mine": everything that fires on its own, with when and what it does next.
#[tauri::command]
pub async fn list_my_automations(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<trade_core::service::MineItem>, String> {
    let zone = current_zone(&app);
    with_vault(&state, |vault| Ok(vault.my_automations(now_secs(), &zone)))
}

/// The "On" switch of a "Mine" row, whatever kind it is.
#[tauri::command]
pub async fn set_my_automation_active(
    input: trade_core::ipc_contract::SetMyAutomationActiveInput,
    state: State<'_, AppState>,
) -> Result<(), String> {
    with_vault_mut(&state, |vault| {
        vault.set_my_automation_active(input.kind, &input.id, input.active, now_secs())
    })
}

/// The card of a scheme or a repeat: when, if, what, asks, next time, runs.
#[tauri::command]
pub async fn get_automation_card(
    id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<trade_core::service::AutomationCard, String> {
    let zone = current_zone(&app);
    with_vault(&state, |vault| {
        vault.automation_card(&id, now_secs(), &zone)
    })
}

/// The next three dates a repeat would fire on: the form shows them as it is filled.
#[tauri::command]
pub async fn preview_automation_dates(
    input: trade_core::ipc_contract::AutomationDatesInput,
    app: AppHandle,
) -> Result<Vec<String>, String> {
    Ok(input.upcoming(now_secs(), &current_zone(&app)))
}

/// The journal: every run and every reminder or alert shown, newest first.
#[tauri::command]
pub async fn list_automation_journal(
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<trade_core::service::JournalEntry>, String> {
    with_vault(&state, |vault| {
        Ok(vault.automation_journal(limit.unwrap_or(100).min(500)))
    })
}

// -- savings, deposits, ПДС ---------------------------------------------------------

#[tauri::command]
pub async fn set_deposit_terms(
    input: SetDepositTermsInput,
    state: State<'_, AppState>,
) -> Result<trade_core::model::store::Store, String> {
    with_vault_mut(&state, |vault| {
        vault.set_deposit_terms(&input.store_id, input.terms)
    })
}

/// Rate, interest earned and ahead, ПДС co-financing of one account (today in the user's zone).
#[tauri::command]
pub async fn get_deposit_report(
    store_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DepositReport, String> {
    let today = trade_core::calendar::Calendar::new(&current_zone(&app)).date_of(now_secs());
    with_vault(&state, |vault| vault.deposit_report(&store_id, today))
}

/// НДФЛ on deposit interest for a year (this one by default).
#[tauri::command]
pub async fn get_interest_tax(
    year: Option<i32>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<InterestTaxReport, String> {
    use chrono::Datelike;
    let today = trade_core::calendar::Calendar::new(&current_zone(&app)).date_of(now_secs());
    with_vault(&state, |vault| {
        Ok(vault.interest_tax(year.unwrap_or(today.year())))
    })
}

// -- bonuses and cashback -----------------------------------------------------------

#[tauri::command]
pub async fn set_loyalty_info(
    input: SetLoyaltyInfoInput,
    state: State<'_, AppState>,
) -> Result<trade_core::model::store::Store, String> {
    with_vault_mut(&state, |vault| {
        vault.set_loyalty_info(&input.store_id, input.info)
    })
}

#[tauri::command]
pub async fn create_loyalty_program(
    input: CreateLoyaltyProgramInput,
    state: State<'_, AppState>,
) -> Result<trade_core::model::store::Store, String> {
    with_vault_mut(&state, |vault| {
        vault.create_loyalty_program(&input.name, &input.color, input.contact_id, input.info)
    })
}

#[tauri::command]
pub async fn adjust_points(
    input: AdjustPointsInput,
    state: State<'_, AppState>,
) -> Result<Transaction, String> {
    with_vault_mut(&state, |vault| {
        vault.adjust_points(
            &input.store_id,
            input.delta,
            &input.comment,
            input.source_store_id,
            input.timestamp,
        )
    })
}

/// Cashback and bonus points of this month (`period = "month"`) or this year.
#[tauri::command]
pub async fn get_cashback_report(
    period: Option<String>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<CashbackReport, String> {
    use chrono::Datelike;
    let today = trade_core::calendar::Calendar::new(&current_zone(&app)).date_of(now_secs());
    let from = if period.as_deref() == Some("year") {
        format!("{}-01-01T00:00:00Z", today.year())
    } else {
        format!("{}-01T00:00:00Z", today.format("%Y-%m"))
    };
    with_vault(&state, |vault| {
        Ok(vault.cashback_report(&from, "9999-12-31T23:59:59Z"))
    })
}

// -- utilities (plan §7.5) ---------------------------------------------------------

#[tauri::command]
pub async fn list_utility_items(state: State<'_, AppState>) -> Result<Vec<UtilityItem>, String> {
    with_vault(&state, |vault| Ok(vault.utility_items()))
}

#[tauri::command]
pub async fn create_utility_item(
    input: UtilityInput,
    state: State<'_, AppState>,
) -> Result<UtilityItem, String> {
    with_vault_mut(&state, |vault| vault.create_utility_item(input.into()))
}

#[tauri::command]
pub async fn update_utility_item(
    input: UpdateUtilityInput,
    state: State<'_, AppState>,
) -> Result<UtilityItem, String> {
    with_vault_mut(&state, |vault| {
        vault.update_utility_item(&input.id, input.item.into())
    })
}

#[tauri::command]
pub async fn delete_utility_item(id: String, state: State<'_, AppState>) -> Result<(), String> {
    with_vault_mut(&state, |vault| vault.delete_utility_item(&id))
}

#[tauri::command]
pub async fn add_meter_reading(
    input: MeterReadingInput,
    state: State<'_, AppState>,
) -> Result<MeterReading, String> {
    with_vault_mut(&state, |vault| {
        vault.add_meter_reading(
            &input.item_id,
            input.value,
            &input.period,
            input.day,
            input.zone.as_deref(),
        )
    })
}

#[tauri::command]
pub async fn update_meter_reading(
    input: UpdateMeterReadingInput,
    state: State<'_, AppState>,
) -> Result<MeterReading, String> {
    with_vault_mut(&state, |vault| {
        vault.update_meter_reading(&input.id, input.value, &input.period, input.day)
    })
}

/// Every active utility of a property for one month (`YYYY-MM`, default: this month).
#[tauri::command]
pub async fn get_utility_bill(
    property_id: String,
    period: Option<String>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<UtilityBill, String> {
    let zone = current_zone(&app);
    let period = period.unwrap_or_else(|| {
        trade_core::calendar::month_key(
            trade_core::calendar::Calendar::new(&zone).date_of(now_secs()),
        )
    });
    with_vault(&state, |vault| {
        vault.utility_bill(&property_id, &period, &zone)
    })
}

/// Pay a month's bill at once: readings plus one payment per line, all or nothing.
#[tauri::command]
pub async fn pay_utility_bill(
    input: PayUtilityBillInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<Transaction>, String> {
    let zone = current_zone(&app);
    with_vault_mut(&state, |vault| vault.pay_utility_bill(input, &zone))
}

#[tauri::command]
pub async fn delete_meter_reading(id: String, state: State<'_, AppState>) -> Result<(), String> {
    with_vault_mut(&state, |vault| vault.delete_meter_reading(&id))
}

#[tauri::command]
pub async fn get_meter_trend(
    item_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<UtilityTrend, String> {
    let zone = current_zone(&app);
    with_vault(&state, |vault| vault.meter_trend(&item_id, &zone))
}

/// A property's value, debt and monthly costs.
#[tauri::command]
pub async fn get_property_summary(
    store_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<PropertySummary, String> {
    let zone = current_zone(&app);
    with_vault(&state, |vault| vault.property_summary(&store_id, &zone))
}

/// What a meter's bill would be by its formula (or tariff); needs no vault.
#[tauri::command]
pub async fn preview_utility_formula(
    input: trade_core::service::UtilityFormulaInput,
) -> Result<trade_core::service::UtilityFormulaPreview, String> {
    Ok(trade_core::service::preview_utility_formula(&input))
}

/// The "Pay" for one month (`YYYY-MM`, default: this month).
#[tauri::command]
pub async fn get_utility_payment_draft(
    item_id: String,
    period: Option<String>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<UtilityPaymentDraft, String> {
    let zone = current_zone(&app);
    let period = period.unwrap_or_else(|| {
        trade_core::calendar::month_key(
            trade_core::calendar::Calendar::new(&zone).date_of(now_secs()),
        )
    });
    with_vault(&state, |vault| {
        vault.utility_payment_draft(&item_id, &period, &zone)
    })
}
