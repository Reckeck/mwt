//! Analytics (plan F6): reports, their CSV export and the spending calendar.
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

use trade_core::money::search;
use trade_core::reports::{dashboard, dashboard_tiles, dashboard_today, insights, overview};
use trade_core::usecases::household::{month_balance, relations, search as search_scenarios};
use trade_core::usecases::planning::health;

use crate::commands::AppState;
use crate::commands::app_settings::current_zone;
use crate::commands::{with_vault, with_vault_mut};
use crate::ipc::IpcError;
use crate::ipc::commands::ipc_commands;

fn build_report(
    app: &AppHandle,
    state: &State<'_, AppState>,
    kind: ReportKind,
    filter: &ReportFilter,
) -> Result<Report, IpcError> {
    let zone = current_zone(app);
    with_vault(state, |vault| {
        overview::report(&vault.books(), kind, filter, now_secs(), &zone)
    })
}

#[tauri::command]
pub async fn get_report(
    kind: ReportKind,
    filter: ReportFilter,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Report, IpcError> {
    build_report(&app, &state, kind, &filter)
}

/// The analytics start screen: one card per question, the filter's last
/// month against the one before.
#[tauri::command]
pub async fn get_report_overview(
    filter: ReportFilter,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ReportOverview, IpcError> {
    let zone = current_zone(&app);
    with_vault(&state, |vault| {
        insights::report_overview(&vault.books(), &filter, now_secs(), &zone)
    })
}

/// The capital checkup (plan C4): the light, the fund, loans and DTI, the
/// savings rate, passive income and the freedom index.
#[tauri::command]
pub async fn capital_health(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<HealthReport, IpcError> {
    let zone = current_zone(&app);
    with_vault(&state, |vault| {
        health::capital_health(&vault.books(), now_secs(), &zone)
    })
}

/// The yearly return the freedom potential assumes; `None` = the key rate.
#[tauri::command]
pub async fn health_settings_set(
    expected_return: Option<Amount>,
    state: State<'_, AppState>,
) -> Result<(), IpcError> {
    with_vault_mut(&state, |vault| {
        health::set_freedom_rate(vault, expected_return)
    })
}

/// Save a report as CSV through the native save dialog. `None` when the
/// user closed the dialog. The webview cannot download files itself.
#[tauri::command]
pub async fn export_report_csv(
    kind: ReportKind,
    filter: ReportFilter,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<String>, IpcError> {
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
        .map_err(IpcError::from)
}

/// What each month of the filter's period left and what carried over.
#[tauri::command]
pub async fn get_month_balance(
    filter: ReportFilter,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<MonthBalance, IpcError> {
    let zone = current_zone(&app);
    with_vault(&state, |vault| {
        month_balance::month_balance(vault, &filter, now_secs(), &zone)
    })
}

/// Carry the unspent money over from `month` (`YYYY-MM`); `None` stops it.
#[tauri::command]
pub async fn set_carry_over(
    month: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), IpcError> {
    with_vault_mut(&state, |vault| {
        month_balance::set_carry_over(vault, month.as_deref())
    })
}

/// Spending by day of a `YYYY-MM` month; the current month when omitted.
#[tauri::command]
pub async fn get_calendar(
    month: Option<String>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<CalendarMonth, IpcError> {
    let zone = current_zone(&app);
    let month = month.unwrap_or_else(|| month_key(Calendar::new(&zone).date_of(now_secs())));
    with_vault(&state, |vault| {
        overview::calendar_month(&vault.books(), &month, now_secs(), &zone)
    })
}

/// The dashboard «Spending» widget (plan H2): the period against the previous
/// one, its series and the last seven days; the dashboard period when omitted.
#[tauri::command]
pub async fn get_spending_overview(
    period: Option<DashPeriod>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<SpendingOverview, IpcError> {
    let zone = current_zone(&app);
    with_vault(&state, |vault| {
        let period = period.unwrap_or(vault.doc.settings.dashboard_period);
        Ok(dashboard::spending_overview(
            &vault.books(),
            period,
            now_secs(),
            &zone,
        ))
    })
}

/// "Daily budget" and "This month" (plan H3): the month so far against the
/// previous month to the same date, the plan to date and the growing categories.
#[tauri::command]
pub async fn get_month_pace(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<MonthPace, IpcError> {
    let zone = current_zone(&app);
    with_vault(&state, |vault| {
        Ok(dashboard_tiles::month_pace(
            &vault.books(),
            now_secs(),
            &zone,
        ))
    })
}

/// "Net worth" (plan H3): the holdings now against the period's start, and
/// their series; the dashboard period when omitted.
#[tauri::command]
pub async fn get_net_worth_trend(
    period: Option<DashPeriod>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<NetWorthTrend, IpcError> {
    let zone = current_zone(&app);
    with_vault(&state, |vault| {
        let period = period.unwrap_or(vault.doc.settings.dashboard_period);
        Ok(dashboard_tiles::net_worth_trend(
            &vault.books(),
            period,
            now_secs(),
            &zone,
        ))
    })
}

/// The "Today" strip (plan H5): capital and its change, the light, what
/// may be spent today and up to three things to do, in one request.
#[tauri::command]
pub async fn get_dashboard_today(
    period: Option<DashPeriod>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DashboardToday, IpcError> {
    let zone = current_zone(&app);
    with_vault(&state, |vault| {
        let period = period.unwrap_or(vault.doc.settings.dashboard_period);
        Ok(dashboard_today::dashboard_today(
            &vault.books(),
            period,
            now_secs(),
            &zone,
        ))
    })
}

/// The "Scenario" widget (plan H4): a calculator scenario's plan to date
/// against the money its wallets hold.
#[tauri::command]
pub async fn get_scenario_status(
    id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ScenarioStatus, IpcError> {
    let zone = current_zone(&app);
    with_vault(&state, |vault| {
        dashboard_today::scenario_status(&vault.books(), &id, now_secs(), &zone)
    })
}

/// The card of any entity (plan J1): its head and what links to it.
#[tauri::command]
pub async fn get_entity_card(
    target: EntityRef,
    state: State<'_, AppState>,
) -> Result<EntityCard, IpcError> {
    with_vault(&state, |vault| Ok(relations::entity_card(vault, &target)))
}

/// Everything the palette can find by name (plan J2); wallets come from the tree.
#[tauri::command]
pub async fn get_search_index(state: State<'_, AppState>) -> Result<Vec<SearchEntry>, IpcError> {
    with_vault(&state, |vault| Ok(search::search_index(&vault.doc)))
}

/// Operations matching a text or an amount, newest first. `categories` are
/// the built-in category keys whose translated names matched on the frontend.
#[tauri::command]
pub async fn search_transactions(
    query: String,
    categories: Vec<String>,
    limit: usize,
    state: State<'_, AppState>,
) -> Result<Vec<Transaction>, IpcError> {
    with_vault(&state, |vault| {
        Ok(search::search_transactions(
            &vault.doc,
            &query,
            &categories,
            limit,
        ))
    })
}

/// The sidebar's pinned entities and saved filters with their short values.
#[tauri::command]
pub async fn get_pinned(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<PinnedView>, IpcError> {
    let zone = current_zone(&app);
    with_vault(&state, |vault| {
        Ok(search_scenarios::pinned(vault, now_secs(), &zone))
    })
}

/// Replaces the pinned list (order kept) and returns it valued.
#[tauri::command]
pub async fn set_pinned(
    pins: Vec<Pinned>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<PinnedView>, IpcError> {
    let zone = current_zone(&app);
    with_vault_mut(&state, |vault| {
        search_scenarios::set_pinned(vault, pins)?;
        Ok(search_scenarios::pinned(vault, now_secs(), &zone))
    })
}

ipc_commands!(analytics_commands, ANALYTICS_NAMES, commands::analytics: [get_report, get_report_overview, capital_health, health_settings_set, export_report_csv, get_month_balance, set_carry_over, get_calendar, get_spending_overview, get_month_pace, get_net_worth_trend, get_dashboard_today, get_scenario_status, get_entity_card, get_search_index, search_transactions, get_pinned, set_pinned]);
