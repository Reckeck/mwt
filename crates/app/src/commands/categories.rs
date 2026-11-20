use tauri::State;

use trade_core::ipc_contract::{
    CreateCategoryInput, CreateRuleInput, UpdateCategoryInput, UpdateRuleInput,
};
use trade_core::model::category::Category;
use trade_core::model::planning::CategorizationRule;
use trade_core::money::categories::categorization_rules;
use trade_core::usecases::household::categories;
use trade_core::usecases::money::rebase::rebase_category_input;

use crate::commands::AppState;
use crate::commands::planning::{with_vault, with_vault_mut};
use crate::ipc::IpcError;
use crate::ipc::commands::ipc_commands;

#[tauri::command]
pub async fn list_categories(state: State<'_, AppState>) -> Result<Vec<Category>, IpcError> {
    with_vault(&state, |vault| Ok(vault.doc.categories.clone()))
}

#[tauri::command]
pub async fn create_category(
    input: CreateCategoryInput,
    state: State<'_, AppState>,
) -> Result<Category, IpcError> {
    with_vault_mut(&state, |vault| {
        categories::create_category(vault, input.key.as_deref(), &input.name, input.group)
    })
}

#[tauri::command]
pub async fn update_category(
    input: UpdateCategoryInput,
    state: State<'_, AppState>,
) -> Result<Category, IpcError> {
    with_vault_mut(&state, |vault| {
        let input = rebase_category_input(&vault.doc, input)?;
        categories::update_category(
            vault,
            &input.key,
            input.name.as_deref(),
            input.group,
            input.essential,
            input.passive,
        )
    })
}

#[tauri::command]
pub async fn archive_category(
    key: String,
    archived: bool,
    state: State<'_, AppState>,
) -> Result<(), IpcError> {
    with_vault_mut(&state, |vault| {
        categories::archive_category(vault, &key, archived)
    })
}

#[tauri::command]
pub async fn delete_category(key: String, state: State<'_, AppState>) -> Result<(), IpcError> {
    with_vault_mut(&state, |vault| categories::delete_category(vault, &key))
}

#[tauri::command]
pub async fn list_categorization_rules(
    state: State<'_, AppState>,
) -> Result<Vec<CategorizationRule>, IpcError> {
    with_vault(&state, |vault| Ok(categorization_rules(&vault.doc)))
}

#[tauri::command]
pub async fn create_categorization_rule(
    input: CreateRuleInput,
    state: State<'_, AppState>,
) -> Result<CategorizationRule, IpcError> {
    with_vault_mut(&state, |vault| {
        categories::create_categorization_rule(vault, &input.pattern, &input.category)
    })
}

#[tauri::command]
pub async fn update_categorization_rule(
    input: UpdateRuleInput,
    state: State<'_, AppState>,
) -> Result<CategorizationRule, IpcError> {
    with_vault_mut(&state, |vault| {
        categories::update_categorization_rule(
            vault,
            &input.id,
            &input.pattern,
            &input.category,
            input.active,
        )
    })
}

#[tauri::command]
pub async fn delete_categorization_rule(
    id: String,
    state: State<'_, AppState>,
) -> Result<(), IpcError> {
    with_vault_mut(&state, |vault| {
        categories::delete_categorization_rule(vault, &id)
    })
}

#[tauri::command]
pub async fn reorder_categorization_rules(
    ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<(), IpcError> {
    with_vault_mut(&state, |vault| {
        categories::reorder_categorization_rules(vault, &ids)
    })
}

ipc_commands!(categories_commands, CATEGORIES_NAMES, commands::categories: [list_categories, create_category, update_category, archive_category, delete_category, list_categorization_rules, create_categorization_rule, update_categorization_rule, delete_categorization_rule, reorder_categorization_rules]);
