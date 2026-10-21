use tauri::State;

use trade_core::ipc_contract::{
    CreateCategoryInput, CreateRuleInput, UpdateCategoryInput, UpdateRuleInput,
};
use trade_core::model::category::Category;
use trade_core::model::planning::CategorizationRule;

use crate::commands::{AppState, err_to_string, require_vault};

#[tauri::command]
pub async fn list_categories(state: State<'_, AppState>) -> Result<Vec<Category>, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    Ok(guard.as_ref().unwrap().doc.categories.clone())
}

#[tauri::command]
pub async fn create_category(
    input: CreateCategoryInput,
    state: State<'_, AppState>,
) -> Result<Category, String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .create_category(input.key.as_deref(), &input.name, input.group)
        .map_err(err_to_string)
}

#[tauri::command]
pub async fn update_category(
    input: UpdateCategoryInput,
    state: State<'_, AppState>,
) -> Result<Category, String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .update_category(
            &input.key,
            input.name.as_deref(),
            input.group,
            input.essential,
        )
        .map_err(err_to_string)
}

#[tauri::command]
pub async fn archive_category(
    key: String,
    archived: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .archive_category(&key, archived)
        .map_err(err_to_string)
}

#[tauri::command]
pub async fn delete_category(key: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .delete_category(&key)
        .map_err(err_to_string)
}

#[tauri::command]
pub async fn list_categorization_rules(
    state: State<'_, AppState>,
) -> Result<Vec<CategorizationRule>, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    Ok(guard.as_ref().unwrap().categorization_rules())
}

#[tauri::command]
pub async fn create_categorization_rule(
    input: CreateRuleInput,
    state: State<'_, AppState>,
) -> Result<CategorizationRule, String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .create_categorization_rule(&input.pattern, &input.category)
        .map_err(err_to_string)
}

#[tauri::command]
pub async fn update_categorization_rule(
    input: UpdateRuleInput,
    state: State<'_, AppState>,
) -> Result<CategorizationRule, String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .update_categorization_rule(&input.id, &input.pattern, &input.category, input.active)
        .map_err(err_to_string)
}

#[tauri::command]
pub async fn delete_categorization_rule(
    id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .delete_categorization_rule(&id)
        .map_err(err_to_string)
}

#[tauri::command]
pub async fn reorder_categorization_rules(
    ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .reorder_categorization_rules(&ids)
        .map_err(err_to_string)
}
