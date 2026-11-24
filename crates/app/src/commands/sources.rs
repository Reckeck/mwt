//! The user's own data sources (plan I2): the picker's page preview, a test
//! run of a method, the list, saving, refreshing and confirming a jump.
//! Fetching happens without the vault lock (rule 3).

use crate::ipc::events::{AppEvent, PricesUpdated, RatesUpdated, SourcesUpdated};
use mwt_model::DataSource;
use mwt_model::NoticeRecord;
use mwt_planning::datasources as source_reads;
use mwt_planning::datasources::{DataSourceDraft, DataSourceView};
use mwt_usecases::datasource::{self, Extracted, SourcePreview};
use mwt_usecases::usecases::money::inflation;
use mwt_usecases::usecases::planning::datasources;
use tauri::{AppHandle, Manager, State};

use crate::commands::{AppState, require_vault};
use crate::commands::{with_vault, with_vault_mut};
use crate::ipc::IpcError;
use crate::ipc::commands::ipc_commands;

/// Fetch a source's page and take its value out; the error text is what the
/// source's status shows. The core's `apply_source_fetch` takes that text as a
/// `String`, so every error here (the sandbox refusal, the page fetch and the
/// extraction) becomes that status `String`, not only the sandbox refusal.
async fn fetch_value(app: &AppHandle, source: &DataSource) -> Result<Extracted, String> {
    let state = app.state::<AppState>();
    crate::commands::sandbox::refuse(&state, "fetching a data source")
        .map_err(|error| error.to_string())?;
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
    let _ = SourcesUpdated::emit(app, &());
    let _ = PricesUpdated::emit(app, &None);
    let _ = RatesUpdated::emit(app, &());
}

/// Fetch one source and record the result (value, failure or jump).
async fn refresh_one(app: &AppHandle, source: &DataSource) -> Result<Vec<NoticeRecord>, IpcError> {
    // The core records a failed fetch as the text the status shows.
    let fetched = fetch_value(app, source).await;
    let russian = crate::scheduler::russian(app);
    let state = app.state::<AppState>();
    with_vault_mut(&state, |vault| {
        datasources::apply_source_fetch(vault, &source.id, fetched, russian)
    })
}

/// The background job: every source whose interval has passed.
pub async fn refresh_due_sources(app: &AppHandle) {
    let due = {
        let state = app.state::<AppState>();
        let Ok(guard) = require_vault(&state) else {
            return;
        };
        match guard.as_ref() {
            Some(vault) => source_reads::due_data_sources(&vault.doc, chrono::Utc::now()),
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

/// A page fetch for the picker: refused in the sandbox and while locked, so a
/// vault-less call is no read-back channel. The vault guard is dropped before
/// any `.await` (rule 3).
fn allow_page_fetch(state: &AppState) -> Result<(), IpcError> {
    crate::commands::sandbox::refuse(state, "fetching a data source")?;
    drop(require_vault(state).map_err(IpcError::from)?);
    Ok(())
}

async fn preview_page(state: &AppState, url: &str) -> Result<SourcePreview, IpcError> {
    allow_page_fetch(state)?;
    let body = state
        .pricing
        .fetch_page(url)
        .await
        .map_err(IpcError::from)?;
    datasource::preview(url, &body)
        .map_err(|message| IpcError::from(mwt_base::Error::InvalidInput(message)))
}

async fn test_method(state: &AppState, draft: &DataSourceDraft) -> Result<Extracted, IpcError> {
    allow_page_fetch(state)?;
    let body = state
        .pricing
        .fetch_page(&draft.url)
        .await
        .map_err(IpcError::from)?;
    datasource::extract(&draft.method, draft.output, &draft.number, &body)
        .map_err(|message| IpcError::from(mwt_base::Error::InvalidInput(message)))
}

/// The page at `url` for the picker: safe HTML, JSON text or file rows.
#[tauri::command]
pub async fn datasource_preview(
    url: String,
    state: State<'_, AppState>,
) -> Result<SourcePreview, IpcError> {
    preview_page(&state, &url).await
}

/// Run the form's method against the live page without saving anything.
#[tauri::command]
pub async fn datasource_test(
    draft: DataSourceDraft,
    state: State<'_, AppState>,
) -> Result<Extracted, IpcError> {
    test_method(&state, &draft).await
}

/// Every source with the places that use it.
#[tauri::command]
pub async fn datasources_list(state: State<'_, AppState>) -> Result<Vec<DataSourceView>, IpcError> {
    with_vault(&state, |vault| {
        Ok(datasources::data_sources_list(&vault.doc))
    })
}

/// Create or change a source; a new or re-pointed one is fetched right away.
#[tauri::command]
pub async fn datasource_save(
    draft: DataSourceDraft,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DataSourceView, IpcError> {
    let saved =
        with_vault_mut::<_, IpcError>(&state, |vault| datasources::save_data_source(vault, draft))?;
    if saved.source.last.is_none()
        && let Ok(notices) = refresh_one(&app, &saved.source).await
    {
        announce(&app, notices);
    } else {
        let _ = SourcesUpdated::emit(&app, &());
        let _ = RatesUpdated::emit(&app, &());
    }
    with_vault(&state, |vault| {
        Ok(datasources::data_sources_list(&vault.doc)
            .into_iter()
            .find(|view| view.source.id == saved.source.id)
            .unwrap_or(saved))
    })
}

/// Delete a source; the inflation that read it goes back to its default.
#[tauri::command]
pub async fn datasource_delete(
    id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), IpcError> {
    with_vault_mut::<_, IpcError>(&state, |vault| datasources::delete_data_source(vault, &id))?;
    let _ = SourcesUpdated::emit(&app, &());
    let _ = RatesUpdated::emit(&app, &());
    Ok(())
}

/// Fetch a source now ("Refresh", "Fix" after a fix).
#[tauri::command]
pub async fn datasource_refresh(
    id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DataSourceView, IpcError> {
    let source = with_vault::<_, IpcError>(&state, |vault| {
        source_reads::data_source(&vault.doc, &id)
            .cloned()
            .ok_or_else(|| mwt_base::Error::NotFound(format!("data source {id}")))
    })?;
    let notices = refresh_one(&app, &source).await?;
    announce(&app, notices);
    with_vault(&state, |vault| {
        datasources::data_sources_list(&vault.doc)
            .into_iter()
            .find(|view| view.source.id == id)
            .ok_or_else(|| mwt_base::Error::NotFound(format!("data source {id}")))
    })
}

/// Accept or drop a value that jumped more than tenfold.
#[tauri::command]
pub async fn datasource_confirm(
    id: String,
    accept: bool,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), IpcError> {
    let russian = crate::scheduler::russian(&app);
    let notices = with_vault_mut::<_, IpcError>(&state, |vault| {
        datasources::confirm_source_value(vault, &id, accept, russian)
    })?;
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
) -> Result<(), IpcError> {
    with_vault_mut::<_, IpcError>(&state, |vault| {
        inflation::set_inflation_source(vault, &currency, &source_id)
    })?;
    let _ = RatesUpdated::emit(&app, &());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft() -> DataSourceDraft {
        serde_json::from_value(serde_json::json!({
            "name": "rate",
            "url": "http://127.0.0.1:1/",
            "method": { "kind": "css", "selector": "p" },
            "output": "number",
            "interval": "day",
        }))
        .unwrap()
    }

    #[tokio::test]
    async fn locked_vault_refuses_preview_without_a_request() {
        let state = AppState::new();
        let err = preview_page(&state, "http://127.0.0.1:1/")
            .await
            .unwrap_err();
        assert_eq!(err.code(), "REQUIRES_UNLOCK");
    }

    #[tokio::test]
    async fn locked_vault_refuses_test_without_a_request() {
        let state = AppState::new();
        let err = test_method(&state, &draft()).await.unwrap_err();
        assert_eq!(err.code(), "REQUIRES_UNLOCK");
    }
}

ipc_commands!(sources_commands, SOURCES_NAMES, commands::sources: [datasource_preview, datasource_test, datasources_list, datasource_save, datasource_delete, datasource_refresh, datasource_confirm, set_inflation_source]);
