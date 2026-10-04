use tauri::State;

use crate::commands::{AppState, err_to_string, require_vault};

#[tauri::command]
pub async fn add_category(category: String, state: State<'_, AppState>) -> Result<String, String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    let vault = guard.as_mut().unwrap();
    vault.add_category(&category).map_err(err_to_string)
}

#[tauri::command]
pub async fn list_categories(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    Ok(guard.as_ref().unwrap().doc.categories.clone())
}
