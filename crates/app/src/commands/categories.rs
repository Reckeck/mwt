use tauri::State;

use crate::commands::{AppState, err_to_string, require_vault};

#[tauri::command]
pub async fn add_category(category: String, state: State<'_, AppState>) -> Result<String, String> {
    let mut g = require_vault(&state).map_err(err_to_string)?;
    let v = g.as_mut().unwrap();
    v.add_category(&category)
        .map_err(crate::commands::err_to_string)
}

#[tauri::command]
pub async fn list_categories(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let g = require_vault(&state).map_err(err_to_string)?;
    Ok(g.as_ref().unwrap().doc.categories.clone())
}
