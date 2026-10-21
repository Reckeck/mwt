//! Analytics (plan Ф6): reports, their CSV export and the spending calendar.
//! The reports live in core; this only adds «now», the zone and the user's
//! reference inflation from the app settings.

use tauri::{AppHandle, State};

use trade_core::calendar::{Calendar, month_key, now_secs};
use trade_core::service::{
    CalendarMonth, EntityCard, EntityRef, Report, ReportFilter, ReportKind, ReportOverview,
    report_csv,
};

use crate::commands::app_settings::{current_settings, current_zone};
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

/// The card of any entity (plan J1): its head and what links to it.
#[tauri::command]
pub async fn get_entity_card(
    target: EntityRef,
    state: State<'_, AppState>,
) -> Result<EntityCard, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    Ok(guard.as_ref().unwrap().entity_card(&target))
}
