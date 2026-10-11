use tauri::State;

use trade_core::ipc_contract::{ContactTurnoverInput, UpdateContactInput};
use trade_core::model::contact::{Contact, DebtTerms};
use trade_core::service::{ContactBalance, ContactDraft, ContactTurnover, DebtTermsDraft};

use crate::commands::{AppState, err_to_string, require_vault};

#[tauri::command]
pub async fn list_contacts(state: State<'_, AppState>) -> Result<Vec<Contact>, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    Ok(guard.as_ref().unwrap().doc.contacts.clone())
}

#[tauri::command]
pub async fn create_contact(
    input: ContactDraft,
    state: State<'_, AppState>,
) -> Result<Contact, String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .create_contact(input)
        .map_err(err_to_string)
}

#[tauri::command]
pub async fn update_contact(
    input: UpdateContactInput,
    state: State<'_, AppState>,
) -> Result<Contact, String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .update_contact(&input.id, input.contact)
        .map_err(err_to_string)
}

#[tauri::command]
pub async fn archive_contact(
    id: String,
    archived: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .archive_contact(&id, archived)
        .map_err(err_to_string)
}

#[tauri::command]
pub async fn delete_contact(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .delete_contact(&id)
        .map_err(err_to_string)
}

/// Money sent to and received from a contact (optionally one account of it).
#[tauri::command]
pub async fn get_contact_turnover(
    input: ContactTurnoverInput,
    state: State<'_, AppState>,
) -> Result<ContactTurnover, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_ref()
        .unwrap()
        .contact_turnover(&input.contact_id, input.account_id.as_deref())
        .map_err(err_to_string)
}

/// Personal debts: who owes me and whom I owe, per contact and asset.
#[tauri::command]
pub async fn get_contact_balances(
    state: State<'_, AppState>,
) -> Result<Vec<ContactBalance>, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    Ok(guard.as_ref().unwrap().contact_balances())
}

/// Due dates and notes of debts, of one contact or of all.
#[tauri::command]
pub async fn list_debt_terms(
    contact_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<DebtTerms>, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    Ok(guard.as_ref().unwrap().debt_terms(contact_id.as_deref()))
}

/// Set one debt's due date and note; empty both to remove them.
#[tauri::command]
pub async fn set_debt_terms(
    input: DebtTermsDraft,
    state: State<'_, AppState>,
) -> Result<Option<DebtTerms>, String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .set_debt_terms(input)
        .map_err(err_to_string)
}
