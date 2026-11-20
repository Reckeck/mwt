//! Enrichment (plan §8.2): look an operation's hash up in a public explorer —
//! only with the user's consent — and keep the finding for them to apply.

use crate::ipc::events::{AppEvent, EnrichmentReady};
use tauri::{AppHandle, Manager, State};
use trade_core::ipc_contract::events::EnrichmentReadyPayload;

use trade_core::model::enrichment::EnrichmentRecord;
use trade_core::money::enrichment as enrichment_reads;
use trade_core::txinfo::TxInfoService;
use trade_core::usecases::money::enrichment as enrichment_cases;

use crate::commands::app_settings::current_settings;
use crate::commands::{AppState, require_vault};
use crate::commands::{with_vault, with_vault_mut};
use crate::ipc::IpcError;
use crate::ipc::commands::ipc_commands;

/// Look a transaction up and store what was found. `Ok(None)` when it has
/// no hash or the explorer had nothing; the guard is released around the
/// network call (AGENTS.md rule 3).
pub async fn enrich(app: &AppHandle, tx_id: &str) -> Result<Option<EnrichmentRecord>, IpcError> {
    let state = app.state::<AppState>();
    crate::commands::sandbox::refuse(&state, "looking up a transaction online")?;
    let request = with_vault::<_, IpcError>(&state, |vault| {
        Ok(enrichment_reads::enrichment_request(&vault.doc, tx_id))
    })?;
    let Some((hash, network)) = request else {
        return Ok(None);
    };
    let info = TxInfoService::default().inspect(&hash, network).await?;
    let record = {
        let mut guard = require_vault(&state)?;
        let vault = guard.as_mut().unwrap();
        let record = enrichment_cases::apply_enrichment_result(vault, tx_id, &info);
        if record.is_some() {
            vault.persist()?;
        }
        record
    };
    if record.is_some() {
        let _ = EnrichmentReady::emit(
            app,
            &EnrichmentReadyPayload {
                tx_id: tx_id.to_string(),
            },
        );
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
) -> Result<Vec<EnrichmentRecord>, IpcError> {
    with_vault(&state, |vault| {
        Ok(enrichment_reads::enrichments(&vault.doc, tx_id.as_deref()))
    })
}

#[tauri::command]
pub async fn apply_enrichment(
    id: String,
    state: State<'_, AppState>,
) -> Result<EnrichmentRecord, IpcError> {
    with_vault_mut(&state, |vault| {
        enrichment_cases::apply_enrichment(vault, &id)
    })
}

#[tauri::command]
pub async fn reject_enrichment(
    id: String,
    state: State<'_, AppState>,
) -> Result<EnrichmentRecord, IpcError> {
    with_vault_mut(&state, |vault| {
        enrichment_cases::reject_enrichment(vault, &id)
    })
}

/// Look a transaction up again now. Refused without the user's consent.
#[tauri::command]
pub async fn re_enrich(
    tx_id: String,
    app: AppHandle,
) -> Result<Option<EnrichmentRecord>, IpcError> {
    if current_settings(&app).enrichment_opt_in != Some(true) {
        return Err("INVALID_INPUT: looking transactions up is turned off".into());
    }
    enrich(&app, &tx_id).await
}

ipc_commands!(enrichment_commands, ENRICHMENT_NAMES, commands::enrichment: [get_enrichments, apply_enrichment, reject_enrichment, re_enrich]);
