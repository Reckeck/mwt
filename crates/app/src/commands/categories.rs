use tauri::State;

use trade_core::ipc_contract::{
    CreateCategoryInput, CreateRuleInput, UpdateCategoryInput, UpdateRuleInput,
};
use trade_core::model::category::Category;
use trade_core::model::planning::CategorizationRule;

use crate::commands::{AppState, require_vault};
use crate::ipc::IpcError;
use crate::ipc::commands::ipc_commands;

#[tauri::command]
pub async fn list_categories(state: State<'_, AppState>) -> Result<Vec<Category>, IpcError> {
    let guard = require_vault(&state)?;
    Ok(guard.as_ref().unwrap().doc.categories.clone())
}

#[tauri::command]
pub async fn create_category(
    input: CreateCategoryInput,
    state: State<'_, AppState>,
) -> Result<Category, IpcError> {
    let mut guard = require_vault(&state)?;
    guard
        .as_mut()
        .unwrap()
        .create_category(input.key.as_deref(), &input.name, input.group)
        .map_err(IpcError::from)
}

#[tauri::command]
pub async fn update_category(
    input: UpdateCategoryInput,
    state: State<'_, AppState>,
) -> Result<Category, IpcError> {
    let mut guard = require_vault(&state)?;
    let vault = guard.as_mut().unwrap();
    let input = vault.rebase_category_input(input)?;
    vault
        .update_category(
            &input.key,
            input.name.as_deref(),
            input.group,
            input.essential,
            input.passive,
        )
        .map_err(IpcError::from)
}

#[tauri::command]
pub async fn archive_category(
    key: String,
    archived: bool,
    state: State<'_, AppState>,
) -> Result<(), IpcError> {
    let mut guard = require_vault(&state)?;
    guard
        .as_mut()
        .unwrap()
        .archive_category(&key, archived)
        .map_err(IpcError::from)
}

#[tauri::command]
pub async fn delete_category(key: String, state: State<'_, AppState>) -> Result<(), IpcError> {
    let mut guard = require_vault(&state)?;
    guard
        .as_mut()
        .unwrap()
        .delete_category(&key)
        .map_err(IpcError::from)
}

#[tauri::command]
pub async fn list_categorization_rules(
    state: State<'_, AppState>,
) -> Result<Vec<CategorizationRule>, IpcError> {
    let guard = require_vault(&state)?;
    Ok(guard.as_ref().unwrap().categorization_rules())
}

#[tauri::command]
pub async fn create_categorization_rule(
    input: CreateRuleInput,
    state: State<'_, AppState>,
) -> Result<CategorizationRule, IpcError> {
    let mut guard = require_vault(&state)?;
    guard
        .as_mut()
        .unwrap()
        .create_categorization_rule(&input.pattern, &input.category)
        .map_err(IpcError::from)
}

#[tauri::command]
pub async fn update_categorization_rule(
    input: UpdateRuleInput,
    state: State<'_, AppState>,
) -> Result<CategorizationRule, IpcError> {
    let mut guard = require_vault(&state)?;
    guard
        .as_mut()
        .unwrap()
        .update_categorization_rule(&input.id, &input.pattern, &input.category, input.active)
        .map_err(IpcError::from)
}

#[tauri::command]
pub async fn delete_categorization_rule(
    id: String,
    state: State<'_, AppState>,
) -> Result<(), IpcError> {
    let mut guard = require_vault(&state)?;
    guard
        .as_mut()
        .unwrap()
        .delete_categorization_rule(&id)
        .map_err(IpcError::from)
}

#[tauri::command]
pub async fn reorder_categorization_rules(
    ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<(), IpcError> {
    let mut guard = require_vault(&state)?;
    guard
        .as_mut()
        .unwrap()
        .reorder_categorization_rules(&ids)
        .map_err(IpcError::from)
}

ipc_commands!(categories_commands, CATEGORIES_NAMES, commands::categories: [list_categories, create_category, update_category, archive_category, delete_category, list_categorization_rules, create_categorization_rule, update_categorization_rule, delete_categorization_rule, reorder_categorization_rules]);
