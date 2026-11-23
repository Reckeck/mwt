//! Planning (plan §7): automations, budgets, the daily budget, month totals,
//! goals, reminders, price alerts and what is coming up. Every computation
//! lives in core; days are cut in the zone from [`current_zone`].

use tauri::{AppHandle, State};

use trade_core::Amount;
use trade_core::calendar::now_secs;
use trade_core::household::loyalty::{CashbackReport, cashback_report};
use trade_core::household::utilities as utility_reads;
use trade_core::household::utilities::{
    PayUtilityBillInput, PropertySummary, UtilityBill, UtilityPaymentDraft, UtilityTrend,
};
use trade_core::ipc_contract::{
    AdjustPointsInput, AutomationInput, BudgetInput, ConfirmEstimatedInput,
    CreateLoyaltyProgramInput, GoalInput, MeterReadingInput, PriceAlertInput, ReminderInput,
    SetDepositTermsInput, SetLoyaltyInfoInput, SimulateAutomationsInput, SnoozeReminderInput,
    UpdateAutomationInput, UpdateBudgetInput, UpdateGoalInput, UpdateMeterReadingInput,
    UpdatePriceAlertInput, UpdateReminderInput, UpdateUtilityInput, UtilityInput,
};
use trade_core::model::ids::StoreId;
use trade_core::model::planning::{
    Automation, AutomationRun, Budget, EmergencyFund, Goal, MeterReading, PriceAlert, Reminder,
    UtilityItem,
};
use trade_core::model::store::Store;
use trade_core::model::transaction::Transaction;
use trade_core::money::interest as interest_reads;
use trade_core::money::interest::{DepositReport, InterestTaxReport};
use trade_core::planning::agenda::UpcomingItem;
use trade_core::planning::automations::AutomationSimulation;
use trade_core::planning::budget::{BudgetStatus, DailyBudget, MonthSummary};
use trade_core::planning::emergency::EmergencyStatus;
use trade_core::planning::goals::{GoalCalendar, GoalDeleted, GoalProgress};
use trade_core::planning::{
    agenda as agenda_reads, budget as budget_reads, daily_budget as daily_reads,
    goals as goal_reads, upcoming as upcoming_reads,
};
use trade_core::service::UnlockedVault;
use trade_core::usecases::household::{loyalty, utilities};
use trade_core::usecases::money::interest;
use trade_core::usecases::money::rebase;
use trade_core::usecases::planning::{
    agenda, auto_reminders, automation_card, automation_rules, automations, budget, emergency,
    goals, my_automations, scenarios,
};
use trade_core::workflow::Workflow;

use crate::commands::app_settings::current_zone;
use crate::commands::{AppState, with_vault, with_vault_mut};
use crate::ipc::IpcError;
use crate::ipc::commands::ipc_commands;
use crate::ipc::ipc;

// -- automations -----------------------------------------------------------------

#[tauri::command]
pub async fn list_automations(state: State<'_, AppState>) -> Result<Vec<Automation>, IpcError> {
    with_vault(&state, |vault| Ok(automations::automations(vault)))
}

#[tauri::command]
pub async fn create_automation(
    input: AutomationInput,
    state: State<'_, AppState>,
) -> Result<Automation, IpcError> {
    let draft = input.to_draft()?;
    with_vault_mut(&state, |vault| automations::create_automation(vault, draft))
}

#[tauri::command]
pub async fn update_automation(
    input: UpdateAutomationInput,
    state: State<'_, AppState>,
) -> Result<Automation, IpcError> {
    with_vault_mut(&state, |vault| {
        let input = rebase::rebase_automation_input(vault, input)?;
        let draft = input.automation.to_draft()?;
        automations::update_automation(vault, &input.id, draft)
    })
}

#[tauri::command]
pub async fn delete_automation(id: String, state: State<'_, AppState>) -> Result<(), IpcError> {
    with_vault_mut(&state, |vault| automations::delete_automation(vault, &id))
}

/// "Create now": same funds check as the schedule; a shortfall is
/// `INSUFFICIENT_FUNDS`.
#[tauri::command]
pub async fn run_automation(
    id: String,
    state: State<'_, AppState>,
) -> Result<Transaction, IpcError> {
    with_vault_mut(&state, |vault| {
        automations::run_automation(vault, &id, now_secs())
    })
}

/// What the automations would do over the next days, worked out on a sandbox
/// copy of the vault: nothing is written.
#[tauri::command]
pub async fn simulate_automations(
    input: SimulateAutomationsInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<AutomationSimulation, IpcError> {
    let zone = current_zone(&app);
    with_vault(&state, |vault| {
        automations::simulate_automations(
            vault,
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
) -> Result<Vec<AutomationRun>, IpcError> {
    with_vault(&state, |vault| {
        Ok(automation_rules::automation_runs(
            &vault.doc,
            automation_id.as_deref(),
        ))
    })
}

#[tauri::command]
pub async fn retry_automation_run(
    run_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Transaction, IpcError> {
    let zone = current_zone(&app);
    with_vault_mut(&state, |vault| {
        automations::retry_automation_run(vault, &run_id, now_secs(), &zone)
    })
}

/// The real amount of an operation an automation booked approximately (a
/// salary that differs every time); it makes the next estimate.
#[tauri::command]
pub async fn confirm_estimated_amount(
    input: ConfirmEstimatedInput,
    state: State<'_, AppState>,
) -> Result<Transaction, IpcError> {
    let amount = Amount::parse(&input.amount)?;
    with_vault_mut(&state, |vault| {
        automations::confirm_estimated_amount(vault, &input.tx_id, amount)
    })
}

/// "Make an ordinary workflow": the automation's workflow is edited in the
/// editor from now on.
#[tauri::command]
pub async fn detach_automation(
    id: String,
    state: State<'_, AppState>,
) -> Result<Workflow, IpcError> {
    with_vault_mut(&state, |vault| automations::detach_automation(vault, &id))
}

#[tauri::command]
pub async fn skip_automation_run(
    run_id: String,
    state: State<'_, AppState>,
) -> Result<AutomationRun, IpcError> {
    with_vault_mut(&state, |vault| {
        automations::skip_automation_run(vault, &run_id)
    })
}

// -- budgets ---------------------------------------------------------------------

#[tauri::command]
pub async fn list_budgets(state: State<'_, AppState>) -> Result<Vec<Budget>, IpcError> {
    with_vault(&state, |vault| Ok(budget_reads::budgets(&vault.doc)))
}

#[tauri::command]
pub async fn create_budget(
    input: BudgetInput,
    state: State<'_, AppState>,
) -> Result<Budget, IpcError> {
    with_vault_mut(&state, |vault| budget::create_budget(vault, input.into()))
}

#[tauri::command]
pub async fn update_budget(
    input: UpdateBudgetInput,
    state: State<'_, AppState>,
) -> Result<Budget, IpcError> {
    with_vault_mut(&state, |vault| {
        let input = rebase::rebase_budget_input(&vault.doc, input)?;
        budget::update_budget(vault, &input.id, input.budget.into())
    })
}

#[tauri::command]
pub async fn delete_budget(id: String, state: State<'_, AppState>) -> Result<(), IpcError> {
    with_vault_mut(&state, |vault| budget::delete_budget(vault, &id))
}

#[tauri::command]
pub async fn get_budget_status(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<BudgetStatus>, IpcError> {
    let zone = current_zone(&app);
    with_vault(&state, |vault| {
        Ok(budget::budget_status(&vault.books(), now_secs(), &zone))
    })
}

#[tauri::command]
pub async fn get_daily_budget(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DailyBudget, IpcError> {
    let zone = current_zone(&app);
    with_vault(&state, |vault| {
        Ok(daily_reads::daily_budget(&vault.books(), now_secs(), &zone))
    })
}

/// Totals of a `YYYY-MM` month; the current month when omitted.
#[tauri::command]
pub async fn get_month_summary(
    month: Option<String>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<MonthSummary, IpcError> {
    let zone = current_zone(&app);
    let month = month.unwrap_or_else(|| {
        let date = trade_core::calendar::Calendar::new(&zone).date_of(now_secs());
        trade_core::calendar::month_key(date)
    });
    with_vault(&state, |vault| {
        budget::month_summary(&vault.books(), &month, &zone)
    })
}

// -- goals -----------------------------------------------------------------------

#[tauri::command]
pub async fn list_goals(state: State<'_, AppState>) -> Result<Vec<Goal>, IpcError> {
    with_vault(&state, |vault| Ok(goal_reads::goals(&vault.doc)))
}

#[tauri::command]
pub async fn create_goal(input: GoalInput, state: State<'_, AppState>) -> Result<Goal, IpcError> {
    with_vault_mut(&state, |vault| goals::create_goal(vault, input.into()))
}

#[tauri::command]
pub async fn update_goal(
    input: UpdateGoalInput,
    state: State<'_, AppState>,
) -> Result<Goal, IpcError> {
    with_vault_mut(&state, |vault| {
        let input = rebase::rebase_goal_input(&vault.doc, input)?;
        goals::update_goal(vault, &input.id, input.goal.into())
    })
}

#[tauri::command]
pub async fn delete_goal(id: String, state: State<'_, AppState>) -> Result<GoalDeleted, IpcError> {
    with_vault_mut(&state, |vault| goals::delete_goal(vault, &id, now_secs()))
}

/// Undo of [`delete_goal`].
#[tauri::command]
pub async fn restore_goal(
    deleted: GoalDeleted,
    state: State<'_, AppState>,
) -> Result<Goal, IpcError> {
    with_vault_mut(&state, |vault| {
        goals::restore_goal(vault, deleted, now_secs())
    })
}

#[tauri::command]
pub async fn get_goal_progress(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<GoalProgress>, IpcError> {
    let zone = current_zone(&app);
    with_vault(&state, |vault| {
        Ok(goals::goal_progress(&vault.books(), now_secs(), &zone))
    })
}

/// A goal draft that reaches what a calculator scenario ends with; the goal
/// form opens with it.
#[tauri::command]
pub async fn goal_from_scenario(
    id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<GoalInput, IpcError> {
    let zone = current_zone(&app);
    with_vault(&state, |vault| {
        scenarios::goal_from_scenario(&vault.doc, &id, now_secs(), &zone)
    })
}

#[tauri::command]
pub async fn get_goal_calendar(
    year: i32,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<GoalCalendar, IpcError> {
    let zone = current_zone(&app);
    with_vault(&state, |vault| {
        goals::goal_calendar(&vault.books(), year, now_secs(), &zone)
    })
}

// -- emergency fund --------------------------------------------------------------

/// The emergency fund with how many months of mandatory spending it covers.
#[tauri::command]
pub async fn emergency_fund_get(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<EmergencyStatus, IpcError> {
    let zone = current_zone(&app);
    with_vault(&state, |vault| {
        Ok(trade_core::planning::emergency::emergency_fund_status(
            &vault.books(),
            now_secs(),
            &zone,
        ))
    })
}

#[tauri::command]
pub async fn emergency_fund_set(
    fund: EmergencyFund,
    state: State<'_, AppState>,
) -> Result<EmergencyFund, IpcError> {
    with_vault_mut(&state, |vault| emergency::set_emergency_fund(vault, fund))
}

/// Add a savings account to the fund or take it out.
#[tauri::command]
pub async fn emergency_fund_toggle(
    store_id: String,
    on: bool,
    state: State<'_, AppState>,
) -> Result<EmergencyFund, IpcError> {
    with_vault_mut(&state, |vault| {
        emergency::emergency_fund_toggle(vault, &store_id, on)
    })
}

// -- reminders -------------------------------------------------------------------

/// Every reminder; the automatic ones are brought in step with the data first.
#[tauri::command]
pub async fn list_reminders(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<Reminder>, IpcError> {
    let zone = current_zone(&app);
    let ru = crate::scheduler::russian(&app);
    with_vault_mut(&state, |vault| {
        auto_reminders::sync_auto_reminders(vault, now_secs(), &zone, ru)?;
        Ok(auto_reminders::reminders_in(vault, now_secs(), &zone, ru))
    })
}

#[tauri::command]
pub async fn create_reminder(
    input: ReminderInput,
    state: State<'_, AppState>,
) -> Result<Reminder, IpcError> {
    with_vault_mut(&state, |vault| {
        agenda::create_reminder_paying(vault, &input.text, &input.due_at, input.repeat, input.pay)
    })
}

#[tauri::command]
pub async fn update_reminder(
    input: UpdateReminderInput,
    state: State<'_, AppState>,
) -> Result<Reminder, IpcError> {
    with_vault_mut(&state, |vault| {
        let input = rebase::rebase_reminder_input(&vault.doc, input)?;
        agenda::update_reminder_paying(
            vault,
            &input.id,
            &input.reminder.text,
            &input.reminder.due_at,
            input.reminder.repeat,
            input.reminder.pay,
        )
    })
}

#[tauri::command]
pub async fn delete_reminder(id: String, state: State<'_, AppState>) -> Result<(), IpcError> {
    with_vault_mut(&state, |vault| agenda::delete_reminder(vault, &id))
}

#[tauri::command]
pub async fn set_reminder_done(
    id: String,
    done: bool,
    state: State<'_, AppState>,
) -> Result<Reminder, IpcError> {
    with_vault_mut(&state, |vault| agenda::set_reminder_done(vault, &id, done))
}

/// Stop a repeating reminder for good.
#[tauri::command]
pub async fn finish_reminder_series(
    id: String,
    state: State<'_, AppState>,
) -> Result<Reminder, IpcError> {
    with_vault_mut(&state, |vault| agenda::finish_reminder_series(vault, &id))
}

/// Put a reminder off until a moment.
#[tauri::command]
pub async fn snooze_reminder(
    input: SnoozeReminderInput,
    state: State<'_, AppState>,
) -> Result<Reminder, IpcError> {
    with_vault_mut(&state, |vault| {
        agenda::snooze_reminder(vault, &input.id, &input.until)
    })
}

// -- price alerts ----------------------------------------------------------------

#[tauri::command]
pub async fn list_price_alerts(state: State<'_, AppState>) -> Result<Vec<PriceAlert>, IpcError> {
    with_vault(&state, |vault| Ok(agenda_reads::price_alerts(&vault.doc)))
}

#[tauri::command]
pub async fn create_price_alert(
    input: PriceAlertInput,
    state: State<'_, AppState>,
) -> Result<PriceAlert, IpcError> {
    with_vault_mut(&state, |vault| {
        agenda::create_price_alert(vault, input.into())
    })
}

#[tauri::command]
pub async fn update_price_alert(
    input: UpdatePriceAlertInput,
    state: State<'_, AppState>,
) -> Result<PriceAlert, IpcError> {
    with_vault_mut(&state, |vault| {
        let input = rebase::rebase_price_alert_input(&vault.doc, input)?;
        agenda::update_price_alert(vault, &input.id, input.alert.into())
    })
}

#[tauri::command]
pub async fn delete_price_alert(id: String, state: State<'_, AppState>) -> Result<(), IpcError> {
    with_vault_mut(&state, |vault| agenda::delete_price_alert(vault, &id))
}

// -- agenda ----------------------------------------------------------------------

/// What is coming up in the next `days` days (default 30).
#[tauri::command]
pub async fn get_upcoming(
    days: Option<u32>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<UpcomingItem>, IpcError> {
    let zone = current_zone(&app);
    with_vault(&state, |vault| {
        Ok(upcoming_reads::upcoming(
            &vault.books(),
            days.unwrap_or(30).min(366),
            now_secs(),
            &zone,
        ))
    })
}

/// "Mine": everything that fires on its own, with when and what it does next.
#[tauri::command]
pub async fn list_my_automations(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<trade_core::planning::my_automations::MineItem>, IpcError> {
    let zone = current_zone(&app);
    with_vault(&state, |vault| {
        Ok(my_automations::my_automations(vault, now_secs(), &zone))
    })
}

/// The "On" switch of a "Mine" row, whatever kind it is.
#[tauri::command]
pub async fn set_my_automation_active(
    input: trade_core::ipc_contract::SetMyAutomationActiveInput,
    state: State<'_, AppState>,
) -> Result<(), IpcError> {
    with_vault_mut(&state, |vault| {
        my_automations::set_my_automation_active(
            vault,
            input.kind,
            &input.id,
            input.active,
            now_secs(),
        )
    })
}

/// The card of a scheme or a repeat: when, if, what, asks, next time, runs.
#[tauri::command]
pub async fn get_automation_card(
    id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<trade_core::planning::automation_card::AutomationCard, IpcError> {
    let zone = current_zone(&app);
    with_vault(&state, |vault| {
        automation_card::automation_card(vault, &id, now_secs(), &zone)
    })
}

/// The next three dates a repeat would fire on: the form shows them as it is filled.
#[tauri::command]
pub async fn preview_automation_dates(
    input: trade_core::ipc_contract::AutomationDatesInput,
    app: AppHandle,
) -> Result<Vec<String>, IpcError> {
    Ok(input.upcoming(now_secs(), &current_zone(&app)))
}

/// The journal: every run and every reminder or alert shown, newest first.
#[tauri::command]
pub async fn list_automation_journal(
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<trade_core::planning::automation_card::JournalEntry>, IpcError> {
    with_vault(&state, |vault| {
        Ok(automation_card::automation_journal(
            vault,
            limit.unwrap_or(100).min(500),
        ))
    })
}

// -- savings, deposits, ПДС ---------------------------------------------------------

#[tauri::command]
pub async fn set_deposit_terms(
    input: SetDepositTermsInput,
    state: State<'_, AppState>,
) -> Result<trade_core::model::store::Store, IpcError> {
    with_vault_mut(&state, |vault| {
        interest::set_deposit_terms(vault, &StoreId::from(input.store_id.as_str()), input.terms)
    })
}

/// Rate, interest earned and ahead, ПДС co-financing of one account (today in the user's zone).
#[tauri::command]
pub async fn get_deposit_report(
    store_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DepositReport, IpcError> {
    let today = trade_core::calendar::Calendar::new(&current_zone(&app)).date_of(now_secs());
    with_vault(&state, |vault| {
        interest_reads::deposit_report(&vault.books(), &StoreId::from(store_id.as_str()), today)
    })
}

/// НДФЛ on deposit interest for a year (this one by default).
#[tauri::command]
pub async fn get_interest_tax(
    year: Option<i32>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<InterestTaxReport, IpcError> {
    use chrono::Datelike;
    let today = trade_core::calendar::Calendar::new(&current_zone(&app)).date_of(now_secs());
    with_vault(&state, |vault| {
        Ok(interest_reads::interest_tax(
            &vault.books(),
            year.unwrap_or(today.year()),
        ))
    })
}

// -- bonuses and cashback -----------------------------------------------------------

#[ipc(write)]
pub fn set_loyalty_info(
    vault: &mut UnlockedVault,
    input: SetLoyaltyInfoInput,
) -> trade_core::Result<Store> {
    loyalty::set_loyalty_info(vault, &StoreId::from(input.store_id), input.info)
}

#[ipc(write)]
pub fn create_loyalty_program(
    vault: &mut UnlockedVault,
    input: CreateLoyaltyProgramInput,
) -> trade_core::Result<Store> {
    loyalty::create_loyalty_program(
        vault,
        &input.name,
        &input.color,
        input.contact_id,
        input.info,
    )
}

#[ipc(write)]
pub fn adjust_points(
    vault: &mut UnlockedVault,
    input: AdjustPointsInput,
) -> trade_core::Result<Transaction> {
    loyalty::adjust_points(
        vault,
        &StoreId::from(input.store_id),
        input.delta,
        &input.comment,
        input.source_store_id.map(StoreId::from),
        input.timestamp,
    )
}

/// Cashback and bonus points of this month (`period = "month"`) or this year.
#[ipc(read)]
pub fn get_cashback_report(
    vault: &UnlockedVault,
    app: &AppHandle,
    period: Option<String>,
) -> trade_core::Result<CashbackReport> {
    use chrono::Datelike;
    let today = trade_core::calendar::Calendar::new(&current_zone(app)).date_of(now_secs());
    let from = if period.as_deref() == Some("year") {
        format!("{}-01-01T00:00:00Z", today.year())
    } else {
        format!("{}-01T00:00:00Z", today.format("%Y-%m"))
    };
    Ok(cashback_report(
        &vault.books(),
        &from,
        "9999-12-31T23:59:59Z",
    ))
}

// -- utilities (plan §7.5) ---------------------------------------------------------

#[tauri::command]
pub async fn list_utility_items(state: State<'_, AppState>) -> Result<Vec<UtilityItem>, IpcError> {
    with_vault(&state, |vault| Ok(utility_reads::utility_items(&vault.doc)))
}

#[tauri::command]
pub async fn create_utility_item(
    input: UtilityInput,
    state: State<'_, AppState>,
) -> Result<UtilityItem, IpcError> {
    with_vault_mut(&state, |vault| {
        utilities::create_utility_item(vault, input.into())
    })
}

#[tauri::command]
pub async fn update_utility_item(
    input: UpdateUtilityInput,
    state: State<'_, AppState>,
) -> Result<UtilityItem, IpcError> {
    with_vault_mut(&state, |vault| {
        let input = rebase::rebase_utility_input(&vault.doc, input)?;
        utilities::update_utility_item(vault, &input.id, input.item.into())
    })
}

#[tauri::command]
pub async fn delete_utility_item(id: String, state: State<'_, AppState>) -> Result<(), IpcError> {
    with_vault_mut(&state, |vault| utilities::delete_utility_item(vault, &id))
}

#[tauri::command]
pub async fn add_meter_reading(
    input: MeterReadingInput,
    state: State<'_, AppState>,
) -> Result<MeterReading, IpcError> {
    with_vault_mut(&state, |vault| {
        utilities::add_meter_reading(
            vault,
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
) -> Result<MeterReading, IpcError> {
    with_vault_mut(&state, |vault| {
        utilities::update_meter_reading(vault, &input.id, input.value, &input.period, input.day)
    })
}

/// Every active utility of a property for one month (`YYYY-MM`, default: this month).
#[tauri::command]
pub async fn get_utility_bill(
    property_id: String,
    period: Option<String>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<UtilityBill, IpcError> {
    let zone = current_zone(&app);
    let period = period.unwrap_or_else(|| {
        trade_core::calendar::month_key(
            trade_core::calendar::Calendar::new(&zone).date_of(now_secs()),
        )
    });
    with_vault(&state, |vault| {
        trade_core::household::utilities::utility_bill(
            &vault.books(),
            &property_id,
            &period,
            &zone,
            now_secs(),
        )
    })
}

/// Pay a month's bill at once: readings plus one payment per line, all or nothing.
#[tauri::command]
pub async fn pay_utility_bill(
    input: PayUtilityBillInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<Transaction>, IpcError> {
    let zone = current_zone(&app);
    with_vault_mut(&state, |vault| {
        utilities::pay_utility_bill(vault, input, &zone)
    })
}

#[tauri::command]
pub async fn delete_meter_reading(id: String, state: State<'_, AppState>) -> Result<(), IpcError> {
    with_vault_mut(&state, |vault| utilities::delete_meter_reading(vault, &id))
}

#[tauri::command]
pub async fn get_meter_trend(
    item_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<UtilityTrend, IpcError> {
    let zone = current_zone(&app);
    with_vault(&state, |vault| {
        utility_reads::meter_trend(&vault.books(), &item_id, &zone)
    })
}

/// A property's value, debt and monthly costs.
#[tauri::command]
pub async fn get_property_summary(
    store_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<PropertySummary, IpcError> {
    let zone = current_zone(&app);
    with_vault(&state, |vault| {
        utilities::property_summary(vault, &store_id, &zone)
    })
}

/// What a meter's bill would be by its formula (or tariff); needs no vault.
#[tauri::command]
pub async fn preview_utility_formula(
    input: trade_core::household::utilities::UtilityFormulaInput,
) -> Result<trade_core::household::utilities::UtilityFormulaPreview, IpcError> {
    Ok(trade_core::household::utilities::preview_utility_formula(
        &input,
    ))
}

/// The "Pay" for one month (`YYYY-MM`, default: this month).
#[tauri::command]
pub async fn get_utility_payment_draft(
    item_id: String,
    period: Option<String>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<UtilityPaymentDraft, IpcError> {
    let zone = current_zone(&app);
    let period = period.unwrap_or_else(|| {
        trade_core::calendar::month_key(
            trade_core::calendar::Calendar::new(&zone).date_of(now_secs()),
        )
    });
    with_vault(&state, |vault| {
        utilities::utility_payment_draft(vault, &item_id, &period, &zone)
    })
}

ipc_commands!(planning_commands, PLANNING_NAMES, commands::planning: [list_automations, create_automation, update_automation, delete_automation, run_automation, simulate_automations, list_automation_runs, retry_automation_run, skip_automation_run, confirm_estimated_amount, detach_automation, list_budgets, create_budget, update_budget, delete_budget, get_budget_status, get_daily_budget, get_month_summary, list_goals, create_goal, update_goal, delete_goal, restore_goal, get_goal_progress, get_goal_calendar, goal_from_scenario, emergency_fund_get, emergency_fund_set, emergency_fund_toggle, list_reminders, create_reminder, update_reminder, delete_reminder, set_reminder_done, finish_reminder_series, snooze_reminder, list_price_alerts, create_price_alert, update_price_alert, delete_price_alert, get_upcoming, list_my_automations, set_my_automation_active, get_automation_card, preview_automation_dates, list_automation_journal, set_loyalty_info, create_loyalty_program, adjust_points, get_cashback_report, set_deposit_terms, get_deposit_report, get_interest_tax, list_utility_items, create_utility_item, update_utility_item, delete_utility_item, add_meter_reading, delete_meter_reading, update_meter_reading, get_utility_bill, pay_utility_bill, preview_utility_formula, get_meter_trend, get_property_summary, get_utility_payment_draft]);
