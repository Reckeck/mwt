//! Analytics (plan Ф6): reports, their CSV export and the spending calendar.
//! The reports live in core; this only adds «now», the zone and the user's
//! reference inflation from the app settings.

use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use trade_core::calendar::{Calendar, month_key, now_secs};
use trade_core::service::{CalendarMonth, Report, ReportFilter, ReportKind, report_csv};

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
    let picked = app
        .dialog()
        .file()
        .add_filter("CSV", &["csv"])
        .set_file_name(&name)
        .blocking_save_file();
    let Some(picked) = picked else {
        return Ok(None);
    };
    let path = picked.into_path().map_err(|err| format!("IO: {err}"))?;
    std::fs::write(&path, csv).map_err(|err| format!("IO: {err}"))?;
    Ok(Some(path.display().to_string()))
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
