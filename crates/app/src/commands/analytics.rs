//! Analytics (plan Ф6): reports, their CSV export and the spending calendar.
//! The reports live in core; this only adds «now», the zone and the user's
//! reference inflation from the app settings.

use tauri::{AppHandle, State};

use trade_core::Amount;
use trade_core::calendar::{Calendar, month_key, now_secs};
use trade_core::model::entity_ref::Pinned;
use trade_core::model::transaction::Transaction;
use trade_core::model::vault::DashPeriod;
use trade_core::service::{
    CalendarMonth, DashboardToday, EntityCard, EntityRef, HealthReport, MonthBalance, MonthPace,
    NetWorthTrend, PinnedView, Report, ReportFilter, ReportKind, ReportOverview, ScenarioStatus,
    SearchEntry, SpendingOverview, report_csv,
};

use crate::commands::app_settings::{current_settings, current_zone};
use crate::commands::planning::with_vault_mut;
use crate::commands::{AppState, err_to_string, require_vault};

fn build_report(
    app: &AppHandle,
    state: &State<'_, AppState>,
    kind: ReportKind,
    filter: &ReportFilter,
) -> Result<Report, String> {
    let zone = current_zone(app);
    let inflation = current_settings(app).inflation;
    let guard = require_vault(state).map_err(err_to_string)?;
    guard
        .as_ref()
        .unwrap()
        .report(kind, filter, now_secs(), &zone, &inflation)
        .map_err(err_to_string)
}

#[tauri::command]
pub async fn get_report(
    kind: ReportKind,
    filter: ReportFilter,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Report, String> {
    build_report(&app, &state, kind, &filter)
}

/// The analytics start screen: one card per question, the filter's last
/// month against the one before.
#[tauri::command]
pub async fn get_report_overview(
    filter: ReportFilter,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ReportOverview, String> {
    let zone = current_zone(&app);
    let inflation = current_settings(&app).inflation;
    let guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_ref()
        .unwrap()
        .report_overview(&filter, now_secs(), &zone, &inflation)
        .map_err(err_to_string)
}

/// The capital checkup (plan C4): the light, the fund, loans and DTI, the
/// savings rate, passive income and the freedom index.
#[tauri::command]
pub async fn capital_health(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<HealthReport, String> {
    let zone = current_zone(&app);
    let inflation = current_settings(&app).inflation;
    let guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_ref()
        .unwrap()
        .capital_health(now_secs(), &zone, &inflation)
        .map_err(err_to_string)
}

/// The yearly return the freedom potential assumes; `None` = the key rate.
#[tauri::command]
pub async fn health_settings_set(
    expected_return: Option<Amount>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    with_vault_mut(&state, |vault| vault.set_freedom_rate(expected_return))
}

/// Save a report as CSV through the native save dialog. `None` when the
/// user closed the dialog. The webview cannot download files itself.
#[tauri::command]
pub async fn export_report_csv(
    kind: ReportKind,
    filter: ReportFilter,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<String>, String> {
    let report = build_report(&app, &state, kind, &filter)?;
    let csv = report_csv(&report);
    let name = format!(
        "{}.csv",
        serde_json::to_value(kind)
            .ok()
            .and_then(|value| value.as_str().map(str::to_string))
            .unwrap_or_else(|| "report".into())
    );
    crate::platform_fs::save_bytes_dialog(&app, &name, "CSV", &["csv"], csv.as_bytes())
        .map_err(err_to_string)
}

/// What each month of the filter's period left and what carried over.
#[tauri::command]
pub async fn get_month_balance(
    filter: ReportFilter,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<MonthBalance, String> {
    let zone = current_zone(&app);
    let inflation = current_settings(&app).inflation;
    let guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_ref()
        .unwrap()
        .month_balance(&filter, now_secs(), &zone, &inflation)
        .map_err(err_to_string)
}

/// Carry the unspent money over from `month` (`YYYY-MM`); `None` stops it.
#[tauri::command]
pub async fn set_carry_over(
    month: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    with_vault_mut(&state, |vault| vault.set_carry_over(month.as_deref()))
}

/// Spending by day of a `YYYY-MM` month; the current month when omitted.
#[tauri::command]
pub async fn get_calendar(
    month: Option<String>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<CalendarMonth, String> {
    let zone = current_zone(&app);
    let month = month.unwrap_or_else(|| month_key(Calendar::new(&zone).date_of(now_secs())));
    let guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_ref()
        .unwrap()
        .calendar_month(&month, now_secs(), &zone)
        .map_err(err_to_string)
}

/// The dashboard «Spending» widget (plan H2): the period against the previous
/// one, its series and the last seven days; the dashboard period when omitted.
#[tauri::command]
pub async fn get_spending_overview(
    period: Option<DashPeriod>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<SpendingOverview, String> {
    let zone = current_zone(&app);
    let guard = require_vault(&state).map_err(err_to_string)?;
    let vault = guard.as_ref().unwrap();
    let period = period.unwrap_or(vault.doc.settings.dashboard_period);
    Ok(vault.spending_overview(period, now_secs(), &zone))
}

/// «Бюджет дня» and «Итоги месяца» (plan H3): the month so far against the
/// previous month to the same date, the plan to date and the growing categories.
#[tauri::command]
pub async fn get_month_pace(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<MonthPace, String> {
    let zone = current_zone(&app);
    let guard = require_vault(&state).map_err(err_to_string)?;
    Ok(guard.as_ref().unwrap().month_pace(now_secs(), &zone))
}

/// «Чистый капитал» (plan H3): the holdings now against the period's start, and
/// their series; the dashboard period when omitted.
#[tauri::command]
pub async fn get_net_worth_trend(
    period: Option<DashPeriod>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<NetWorthTrend, String> {
    let zone = current_zone(&app);
    let guard = require_vault(&state).map_err(err_to_string)?;
    let vault = guard.as_ref().unwrap();
    let period = period.unwrap_or(vault.doc.settings.dashboard_period);
    Ok(vault.net_worth_trend(period, now_secs(), &zone))
}

/// The «Сегодня» strip (plan H5): capital and its change, the light, what
/// may be spent today and up to three things to do, in one request.
#[tauri::command]
pub async fn get_dashboard_today(
    period: Option<DashPeriod>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DashboardToday, String> {
    let zone = current_zone(&app);
    let inflation = current_settings(&app).inflation;
    let guard = require_vault(&state).map_err(err_to_string)?;
    let vault = guard.as_ref().unwrap();
    let period = period.unwrap_or(vault.doc.settings.dashboard_period);
    Ok(vault.dashboard_today(period, now_secs(), &zone, &inflation))
}

/// The «Сценарий» widget (plan H4): a calculator scenario's plan to date
/// against the money its wallets hold.
#[tauri::command]
pub async fn get_scenario_status(
    id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ScenarioStatus, String> {
    let zone = current_zone(&app);
    let guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_ref()
        .unwrap()
        .scenario_status(&id, now_secs(), &zone)
        .map_err(err_to_string)
}

/// The card of any entity (plan J1): its head and what links to it.
#[tauri::command]
pub async fn get_entity_card(
    target: EntityRef,
    state: State<'_, AppState>,
) -> Result<EntityCard, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    Ok(guard.as_ref().unwrap().entity_card(&target))
}

/// Everything the palette can find by name (plan J2); wallets come from the tree.
#[tauri::command]
pub async fn get_search_index(state: State<'_, AppState>) -> Result<Vec<SearchEntry>, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    Ok(guard.as_ref().unwrap().search_index())
}

/// Operations matching a text or an amount, newest first. `categories` are
/// the built-in category keys whose translated names matched on the frontend.
#[tauri::command]
pub async fn search_transactions(
    query: String,
    categories: Vec<String>,
    limit: usize,
    state: State<'_, AppState>,
) -> Result<Vec<Transaction>, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    Ok(guard
        .as_ref()
        .unwrap()
        .search_transactions(&query, &categories, limit))
}

/// The sidebar's pinned entities and saved filters with their short values.
#[tauri::command]
pub async fn get_pinned(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<PinnedView>, String> {
    let zone = current_zone(&app);
    let guard = require_vault(&state).map_err(err_to_string)?;
    Ok(guard.as_ref().unwrap().pinned(now_secs(), &zone))
}

/// Replaces the pinned list (order kept) and returns it valued.
#[tauri::command]
pub async fn set_pinned(
    pins: Vec<Pinned>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<PinnedView>, String> {
    let zone = current_zone(&app);
    with_vault_mut(&state, |vault| {
        vault.set_pinned(pins)?;
        Ok(vault.pinned(now_secs(), &zone))
    })
}
