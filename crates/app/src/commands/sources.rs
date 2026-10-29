//! The user's own data sources (plan I2): the picker's page preview, a test
//! run of a method, the list, saving, refreshing and confirming a jump.
//! Fetching happens without the vault lock (rule 3).

use tauri::{AppHandle, Emitter, Manager, State};
use trade_core::datasource::{self, Extracted, SourcePreview};
use trade_core::model::datasource::DataSource;
use trade_core::model::notify::NoticeRecord;
use trade_core::service::{DataSourceDraft, DataSourceView};

use crate::commands::{AppState, err_to_string, require_vault};

/// Fetch a source's page and take its value out; the error text is what the
/// source's status shows.
async fn fetch_value(app: &AppHandle, source: &DataSource) -> Result<Extracted, String> {
    let state = app.state::<AppState>();
    crate::commands::sandbox::refuse(&state, "fetching a data source")?;
    let body = state
        .pricing
        .fetch_page(&source.url)
        .await
        .map_err(|err| err.to_string())?;
    datasource::extract(&source.method, source.output, &source.number, &body)
}

/// Tell the window that values changed: sources, prices and inflation may
/// all follow a source.
fn announce(app: &AppHandle, notices: Vec<NoticeRecord>) {
    crate::scheduler::record_and_deliver(app, notices);
    let _ = app.emit("sources-updated", ());
    let _ = app.emit("prices-updated", ());
    let _ = app.emit("rates-updated", ());
}

/// Fetch one source and record the result (value, failure or jump).
async fn refresh_one(app: &AppHandle, source: &DataSource) -> Result<Vec<NoticeRecord>, String> {
    let fetched = fetch_value(app, source).await;
    let russian = crate::scheduler::russian(app);
    let state = app.state::<AppState>();
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    let vault = guard.as_mut().unwrap();
    vault
        .apply_source_fetch(&source.id, fetched, russian)
        .map_err(err_to_string)
}

/// The background job: every source whose interval has passed.
pub async fn refresh_due_sources(app: &AppHandle) {
    let due = {
        let state = app.state::<AppState>();
        let Ok(guard) = require_vault(&state) else {
            return;
        };
        match guard.as_ref() {
            Some(vault) => vault.due_data_sources(chrono::Utc::now()),
            None => return,
        }
    };
    if due.is_empty() {
        return;
    }
    let mut notices = Vec::new();
    for source in &due {
        if let Ok(fresh) = refresh_one(app, source).await {
            notices.extend(fresh);
        }
    }
    announce(app, notices);
}

/// The page at `url` for the picker: safe HTML, JSON text or file rows.
#[tauri::command]
pub async fn datasource_preview(
    url: String,
    state: State<'_, AppState>,
) -> Result<SourcePreview, String> {
    crate::commands::sandbox::refuse(&state, "fetching a data source")?;
    let body = state
        .pricing
        .fetch_page(&url)
        .await
        .map_err(err_to_string)?;
    datasource::preview(&url, &body)
        .map_err(|message| err_to_string(trade_core::Error::InvalidInput(message)))
}

/// Run the form's method against the live page without saving anything.
#[tauri::command]
pub async fn datasource_test(
    draft: DataSourceDraft,
    state: State<'_, AppState>,
) -> Result<Extracted, String> {
    crate::commands::sandbox::refuse(&state, "fetching a data source")?;
    let body = state
        .pricing
        .fetch_page(&draft.url)
        .await
        .map_err(err_to_string)?;
    datasource::extract(&draft.method, draft.output, &draft.number, &body)
        .map_err(|message| err_to_string(trade_core::Error::InvalidInput(message)))
}

/// Every source with the places that use it.
#[tauri::command]
pub async fn datasources_list(state: State<'_, AppState>) -> Result<Vec<DataSourceView>, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    Ok(guard.as_ref().unwrap().data_sources_list())
}

/// Create or change a source; a new or re-pointed one is fetched right away.
#[tauri::command]
pub async fn datasource_save(
    draft: DataSourceDraft,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DataSourceView, String> {
    let saved = {
        let mut guard = require_vault(&state).map_err(err_to_string)?;
        guard
            .as_mut()
            .unwrap()
            .save_data_source(draft)
            .map_err(err_to_string)?
    };
    if saved.source.last.is_none()
        && let Ok(notices) = refresh_one(&app, &saved.source).await
    {
        announce(&app, notices);
    } else {
        let _ = app.emit("sources-updated", ());
        let _ = app.emit("rates-updated", ());
    }
    let guard = require_vault(&state).map_err(err_to_string)?;
    let view = guard
        .as_ref()
        .unwrap()
        .data_sources_list()
        .into_iter()
        .find(|view| view.source.id == saved.source.id)
        .unwrap_or(saved);
    Ok(view)
}

/// Delete a source; the inflation that read it goes back to its default.
#[tauri::command]
pub async fn datasource_delete(
    id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    {
        let mut guard = require_vault(&state).map_err(err_to_string)?;
        guard
            .as_mut()
            .unwrap()
            .delete_data_source(&id)
            .map_err(err_to_string)?;
    }
    let _ = app.emit("sources-updated", ());
    let _ = app.emit("rates-updated", ());
    Ok(())
}

/// Fetch a source now ("Refresh", "Fix" after a fix).
#[tauri::command]
pub async fn datasource_refresh(
    id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DataSourceView, String> {
    let source = {
        let guard = require_vault(&state).map_err(err_to_string)?;
        guard
            .as_ref()
            .unwrap()
            .data_source(&id)
            .cloned()
            .ok_or_else(|| {
                err_to_string(trade_core::Error::NotFound(format!("data source {id}")))
            })?
    };
    let notices = refresh_one(&app, &source).await?;
    announce(&app, notices);
    let guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_ref()
        .unwrap()
        .data_sources_list()
        .into_iter()
        .find(|view| view.source.id == id)
        .ok_or_else(|| err_to_string(trade_core::Error::NotFound(format!("data source {id}"))))
}

/// Accept or drop a value that jumped more than tenfold.
#[tauri::command]
pub async fn datasource_confirm(
    id: String,
    accept: bool,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let russian = crate::scheduler::russian(&app);
    let notices = {
        let mut guard = require_vault(&state).map_err(err_to_string)?;
        guard
            .as_mut()
            .unwrap()
            .confirm_source_value(&id, accept, russian)
            .map_err(err_to_string)?
    };
    announce(&app, notices);
    Ok(())
}

/// Read a currency's inflation from a source.
#[tauri::command]
pub async fn set_inflation_source(
    currency: String,
    source_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    {
        let mut guard = require_vault(&state).map_err(err_to_string)?;
        guard
            .as_mut()
            .unwrap()
            .set_inflation_source(&currency, &source_id)
            .map_err(err_to_string)?;
    }
    let _ = app.emit("rates-updated", ());
    Ok(())
}
