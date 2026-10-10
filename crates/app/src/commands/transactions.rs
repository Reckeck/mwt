use tauri::{AppHandle, State};

use trade_core::ipc_contract::{TransactionInput, TransactionKinds, UpdateTransactionInput};
use trade_core::model::store::StoreKind;
use trade_core::model::transaction::Transaction;

use trade_core::service::{TxFilter, TxGraph};

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
        let mut guard = require_vault(&state).map_err(err_to_string)?;
        guard
            .as_mut()
            .unwrap()
            .add_transaction(input.to_transaction())
            .map_err(err_to_string)?
    };
    enrich_in_background(&app, saved.meta().id.clone());
    Ok(saved)
}

/// Replace a transaction with an edited version; its id and origin are kept.
#[tauri::command]
pub async fn update_transaction(
    input: UpdateTransactionInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Transaction, String> {
    let saved = {
        let mut guard = require_vault(&state).map_err(err_to_string)?;
        guard
            .as_mut()
            .unwrap()
            .update_transaction(&input.id, input.transaction.to_transaction())
            .map_err(err_to_string)?
    };
    enrich_in_background(&app, saved.meta().id.clone());
    Ok(saved)
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
