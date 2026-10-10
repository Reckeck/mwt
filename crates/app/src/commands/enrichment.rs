//! Enrichment (plan §8.2): look an operation's hash up in a public explorer —
//! only with the user's consent — and keep the finding for them to apply.

use tauri::{AppHandle, Emitter, Manager, State};

use trade_core::model::enrichment::EnrichmentRecord;
use trade_core::txinfo::TxInfoService;

use crate::commands::app_settings::current_settings;
use crate::commands::{AppState, err_to_string, require_vault};

/// Look a transaction up and store what was found. `Ok(None)` when it has
/// no hash or the explorer had nothing; the guard is released around the
/// network call (AGENTS.md rule 3).
pub async fn enrich(app: &AppHandle, tx_id: &str) -> Result<Option<EnrichmentRecord>, String> {
    let state = app.state::<AppState>();
    let request = {
        let guard = require_vault(&state).map_err(err_to_string)?;
        guard.as_ref().unwrap().enrichment_request(tx_id)
    };
    let Some((hash, network)) = request else {
        return Ok(None);
    };
    let info = TxInfoService::default()
        .inspect(&hash, network)
        .await
        .map_err(err_to_string)?;
    let record = {
        let mut guard = require_vault(&state).map_err(err_to_string)?;
        let vault = guard.as_mut().unwrap();
        let record = vault.apply_enrichment_result(tx_id, &info);
        if record.is_some() {
            vault.persist().map_err(err_to_string)?;
        }
        record
    };
    if record.is_some() {
        let _ = app.emit("enrichment-ready", serde_json::json!({ "tx_id": tx_id }));
    }
    Ok(record)
}

/// After a save: look the hash up in the background, if the user agreed.
pub fn enrich_in_background(app: &AppHandle, tx_id: String) {
    if current_settings(app).enrichment_opt_in != Some(true) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = enrich(&app, &tx_id).await;
    });
}

#[tauri::command]
pub async fn get_enrichments(
    tx_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<EnrichmentRecord>, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    Ok(guard.as_ref().unwrap().enrichments(tx_id.as_deref()))
}

#[tauri::command]
pub async fn apply_enrichment(
    id: String,
    state: State<'_, AppState>,
) -> Result<EnrichmentRecord, String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .apply_enrichment(&id)
        .map_err(err_to_string)
}

#[tauri::command]
pub async fn reject_enrichment(
    id: String,
    state: State<'_, AppState>,
) -> Result<EnrichmentRecord, String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .reject_enrichment(&id)
        .map_err(err_to_string)
}

/// Look a transaction up again now. Refused without the user's consent.
#[tauri::command]
pub async fn re_enrich(tx_id: String, app: AppHandle) -> Result<Option<EnrichmentRecord>, String> {
    if current_settings(&app).enrichment_opt_in != Some(true) {
        return Err("INVALID_INPUT: looking transactions up is turned off".into());
    }
    enrich(&app, &tx_id).await
}
