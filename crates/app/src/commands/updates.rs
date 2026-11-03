//! Update status and the manual check (logic: `crate::updates`).

use tauri::AppHandle;
use trade_core::ipc_contract::UpdateStatusDto;

use crate::updates;

#[tauri::command]
pub async fn update_status(app: AppHandle) -> Result<UpdateStatusDto, String> {
    Ok(updates::status(&app))
}

#[tauri::command]
pub async fn update_check_now(app: AppHandle) -> Result<UpdateStatusDto, String> {
    updates::run_check(app.clone(), true).await;
    Ok(updates::status(&app))
}

/// Install the downloaded package and start the new version. The batched
/// vault writes go to disk first: the installer may end the process.
#[tauri::command]
pub async fn update_restart_now(app: AppHandle) -> Result<(), String> {
    crate::vault_writer::flush_now(&app).map_err(|err| err.to_string())?;
    if updates::apply_pending(&app)? {
        app.restart();
    }
    Ok(())
}
