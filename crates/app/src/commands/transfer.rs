//! Handing the vault to a new device (docs/sync.md, «Передача хранилища»):
//! receiving works without a vault open, sending needs the vault and its
//! password typed again. The work is in `crate::sync::handover`.

use tauri::AppHandle;
use trade_core::ipc_contract::{
    TransferNearby, TransferReceiveView, TransferSendInput, TransferSendView,
};

use crate::commands::err_to_string;
use crate::sync::handover;

/// «Получить хранилище»: visible nearby by this device's name for five minutes.
#[tauri::command]
pub async fn transfer_receive_start(app: AppHandle) -> Result<TransferReceiveView, String> {
    crate::commands::sandbox::refuse_app(&app, "handing the vault over")?;
    handover::receive_start(&app).await.map_err(err_to_string)
}

#[tauri::command]
pub async fn transfer_receive_stop(app: AppHandle) -> Result<(), String> {
    handover::receive_stop(&app);
    Ok(())
}

/// «Принять» (`true`) or «Отклонить» the request on screen.
#[tauri::command]
pub async fn transfer_receive_decide(app: AppHandle, accept: bool) -> Result<(), String> {
    handover::receive_decide(&app, accept).map_err(err_to_string)
}

/// The sending screen opened: look for devices waiting to receive.
#[tauri::command]
pub async fn transfer_browse_start(app: AppHandle) -> Result<Vec<TransferNearby>, String> {
    crate::commands::sandbox::refuse_app(&app, "handing the vault over")?;
    handover::browse_start(&app).map_err(err_to_string)
}

#[tauri::command]
pub async fn transfer_browse_stop(app: AppHandle) -> Result<(), String> {
    handover::browse_stop(&app);
    Ok(())
}

/// «Передать»: `SECRET` when the password is not this vault's.
#[tauri::command]
pub async fn transfer_send(
    app: AppHandle,
    input: TransferSendInput,
) -> Result<TransferSendView, String> {
    crate::commands::sandbox::refuse_app(&app, "handing the vault over")?;
    handover::send_start(&app, &input).map_err(err_to_string)
}

/// The code shown on the receiving device.
#[tauri::command]
pub async fn transfer_send_code(app: AppHandle, code: String) -> Result<(), String> {
    handover::send_code(&app, &code).map_err(err_to_string)
}

#[tauri::command]
pub async fn transfer_send_cancel(app: AppHandle) -> Result<(), String> {
    handover::send_cancel(&app);
    Ok(())
}
