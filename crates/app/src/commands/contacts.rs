use tauri::State;

use mwt_household::contacts::{
    ContactBalance, ContactDraft, ContactSummary, ContactTurnover, DebtTermsDraft, debt_terms,
};
use mwt_model::{Contact, DebtTerms};
use mwt_usecases::ipc_contract::{ContactTurnoverInput, UpdateContactInput};
use mwt_usecases::usecases::household::contacts;
use mwt_usecases::usecases::money::rebase::rebase_contact_input;

use crate::commands::AppState;
use crate::commands::{with_vault, with_vault_mut};
use crate::ipc::IpcError;
use crate::ipc::commands::ipc_commands;

#[tauri::command]
pub async fn list_contacts(state: State<'_, AppState>) -> Result<Vec<Contact>, IpcError> {
    with_vault(&state, |vault| Ok(vault.doc.contacts.clone()))
}

#[tauri::command]
pub async fn create_contact(
    input: ContactDraft,
    state: State<'_, AppState>,
) -> Result<Contact, IpcError> {
    with_vault_mut(&state, |vault| contacts::create_contact(vault, input))
}

#[tauri::command]
pub async fn update_contact(
    input: UpdateContactInput,
    state: State<'_, AppState>,
) -> Result<Contact, IpcError> {
    with_vault_mut(&state, |vault| {
        let input = rebase_contact_input(&vault.doc, input)?;
        contacts::update_contact(vault, &input.id, input.contact)
    })
}

#[tauri::command]
pub async fn archive_contact(
    id: String,
    archived: bool,
    state: State<'_, AppState>,
) -> Result<(), IpcError> {
    with_vault_mut(&state, |vault| {
        contacts::archive_contact(vault, &id, archived)
    })
}

#[tauri::command]
pub async fn delete_contact(id: String, state: State<'_, AppState>) -> Result<(), IpcError> {
    with_vault_mut(&state, |vault| contacts::delete_contact(vault, &id))
}

/// Money sent to and received from a contact (optionally one account of it).
#[tauri::command]
pub async fn get_contact_turnover(
    input: ContactTurnoverInput,
    state: State<'_, AppState>,
) -> Result<ContactTurnover, IpcError> {
    with_vault(&state, |vault| {
        contacts::contact_turnover(vault, &input.contact_id, input.account_id.as_deref())
    })
}

/// Personal debts: who owes me and whom I owe, per contact and asset.
#[tauri::command]
pub async fn get_contact_balances(
    state: State<'_, AppState>,
) -> Result<Vec<ContactBalance>, IpcError> {
    with_vault(&state, |vault| Ok(contacts::contact_balances(vault)))
}

/// One summary per contact for the contacts table: activity and debts in base currency.
#[tauri::command]
pub async fn list_contact_summaries(
    state: State<'_, AppState>,
) -> Result<Vec<ContactSummary>, IpcError> {
    with_vault(&state, contacts::contact_summaries)
}

/// Due dates and notes of debts, of one contact or of all.
#[tauri::command]
pub async fn list_debt_terms(
    contact_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<DebtTerms>, IpcError> {
    with_vault(&state, |vault| {
        Ok(debt_terms(&vault.doc, contact_id.as_deref()))
    })
}

/// Set one debt's due date and note; empty both to remove them.
#[tauri::command]
pub async fn set_debt_terms(
    input: DebtTermsDraft,
    state: State<'_, AppState>,
) -> Result<Option<DebtTerms>, IpcError> {
    with_vault_mut(&state, |vault| contacts::set_debt_terms(vault, input))
}

ipc_commands!(contacts_commands, CONTACTS_NAMES, commands::contacts: [list_contacts, create_contact, update_contact, archive_contact, delete_contact, get_contact_turnover, get_contact_balances, list_contact_summaries, list_debt_terms, set_debt_terms]);
