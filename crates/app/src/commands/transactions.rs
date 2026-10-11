use tauri::{AppHandle, State};

use trade_core::ipc_contract::{
    AdjustmentDraftInput, PreviewTransactionInput, TransactionInput, TransactionKinds,
    UpdateTransactionInput,
};
use trade_core::model::store::StoreKind;
use trade_core::model::transaction::Transaction;

use trade_core::service::{AdjustmentDraft, TxFilter, TxGraph, TxPreview};

use crate::commands::app_settings::current_zone;
use crate::commands::enrichment::enrich_in_background;
use crate::commands::{AppState, err_to_string, require_vault};

/// The operations list: every filter field is optional, `None` = everything.
#[tauri::command]
pub async fn list_transactions(
    filter: Option<TxFilter>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<Transaction>, String> {
    let zone = current_zone(&app);
    let guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_ref()
        .unwrap()
        .list_transactions(&filter.unwrap_or_default(), &zone)
        .map_err(err_to_string)
}

/// The same operations as lanes and edges; `collapsed` folds a store's
/// sub-accounts into its own lane. More than 2000 nodes is `TOO_LARGE`.
#[tauri::command]
pub async fn get_tx_graph(
    filter: Option<TxFilter>,
    collapsed: Option<Vec<String>>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<TxGraph, String> {
    let zone = current_zone(&app);
    let guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_ref()
        .unwrap()
        .tx_graph(
            &filter.unwrap_or_default(),
            &collapsed.unwrap_or_default(),
            &zone,
        )
        .map_err(err_to_string)
}

/// Every kind of transaction goes through here: build, auto-categorize,
/// validate, append, re-derive holdings, persist.
#[tauri::command]
pub async fn add_transaction(
    input: TransactionInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Transaction, String> {
    let saved = {
        let mut input = input;
        let mut guard = require_vault(&state).map_err(err_to_string)?;
        let vault = guard.as_mut().unwrap();
        vault
            .resolve_input_price(&mut input)
            .map_err(err_to_string)?;
        vault
            .add_transaction(input.to_transaction())
            .map_err(err_to_string)?
    };
    enrich_in_background(&app, saved.meta().id.clone());
    Ok(saved)
}

/// What a draft would do to every touched position; writes nothing. A draft
/// that would be refused still previews, with the refusal in `error`.
#[tauri::command]
pub async fn preview_transaction(
    input: PreviewTransactionInput,
    state: State<'_, AppState>,
) -> Result<TxPreview, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    let vault = guard.as_ref().unwrap();
    let mut draft = input.transaction;
    // A missing rate is not a preview error: the draft previews unpriced.
    if vault.resolve_input_price(&mut draft).is_err() {
        draft.common_mut().price = None;
    }
    Ok(vault.preview_transaction(draft.to_transaction(), input.replaces.as_deref()))
}

/// Replace a transaction with an edited version; its id and origin are kept.
#[tauri::command]
pub async fn update_transaction(
    input: UpdateTransactionInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Transaction, String> {
    let saved = {
        let mut draft = input.transaction;
        let mut guard = require_vault(&state).map_err(err_to_string)?;
        let vault = guard.as_mut().unwrap();
        vault
            .resolve_input_price(&mut draft)
            .map_err(err_to_string)?;
        vault
            .update_transaction(&input.id, draft.to_transaction())
            .map_err(err_to_string)?
    };
    enrich_in_background(&app, saved.meta().id.clone());
    Ok(saved)
}

/// The correction screen of one account: its assets and their ledger
/// balances at the moment asked.
#[tauri::command]
pub async fn get_adjustment_draft(
    input: AdjustmentDraftInput,
    state: State<'_, AppState>,
) -> Result<AdjustmentDraft, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_ref()
        .unwrap()
        .adjustment_draft(&input)
        .map_err(err_to_string)
}

#[tauri::command]
pub async fn delete_transaction(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .delete_transaction(&id)
        .map_err(err_to_string)
}

/// Which transaction kinds a wallet of this kind accepts.
///
/// Pure: the binding is static domain data, so this needs no vault and takes no
/// lock. The frontend calls it with the kind of the wallet it is showing, which
/// keeps "what may a bank record" in exactly one place.
#[tauri::command]
pub async fn transaction_kinds_for_store_kind(kind: StoreKind) -> Result<TransactionKinds, String> {
    Ok(TransactionKinds::for_store_kind(kind))
}
