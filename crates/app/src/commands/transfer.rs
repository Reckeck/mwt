//! Handing the vault to a new device (docs/sync.md, "Vault transfer"):
//! receiving works without a vault open, sending needs the vault and its
//! password typed again. The work is in `crate::sync::handover`.

use mwt_usecases::ipc_contract::{
    TransferNearby, TransferReceiveView, TransferSendInput, TransferSendView,
};
use tauri::AppHandle;

use crate::ipc::IpcError;
use crate::ipc::commands::ipc_commands;
use crate::sync::handover;

/// "Receive a vault": visible nearby by this device's name for five minutes.
#[tauri::command]
pub async fn transfer_receive_start(app: AppHandle) -> Result<TransferReceiveView, IpcError> {
    crate::commands::sandbox::refuse_app(&app, "handing the vault over")?;
    handover::receive_start(&app).await.map_err(IpcError::from)
}

#[tauri::command]
pub async fn transfer_receive_stop(app: AppHandle) -> Result<(), IpcError> {
    handover::receive_stop(&app);
    Ok(())
}

/// "Accept" (`true`) or "Decline" the request on screen.
#[tauri::command]
pub async fn transfer_receive_decide(app: AppHandle, accept: bool) -> Result<(), IpcError> {
    handover::receive_decide(&app, accept).map_err(IpcError::from)
}

/// The sending screen opened: look for devices waiting to receive.
#[tauri::command]
pub async fn transfer_browse_start(app: AppHandle) -> Result<Vec<TransferNearby>, IpcError> {
    crate::commands::sandbox::refuse_app(&app, "handing the vault over")?;
    handover::browse_start(&app).map_err(IpcError::from)
}

#[tauri::command]
pub async fn transfer_browse_stop(app: AppHandle) -> Result<(), IpcError> {
    handover::browse_stop(&app);
    Ok(())
}

/// "Send": `SECRET` when the password is not this vault's.
#[tauri::command]
pub async fn transfer_send(
    app: AppHandle,
    input: TransferSendInput,
) -> Result<TransferSendView, IpcError> {
    crate::commands::sandbox::refuse_app(&app, "handing the vault over")?;
    handover::send_start(&app, &input).map_err(IpcError::from)
}

/// The code shown on the receiving device.
#[tauri::command]
pub async fn transfer_send_code(app: AppHandle, code: String) -> Result<(), IpcError> {
    handover::send_code(&app, &code).map_err(IpcError::from)
}

#[tauri::command]
pub async fn transfer_send_cancel(app: AppHandle) -> Result<(), IpcError> {
    handover::send_cancel(&app);
    Ok(())
}

ipc_commands!(transfer_commands, TRANSFER_NAMES, commands::transfer: [transfer_receive_start, transfer_receive_stop, transfer_receive_decide, transfer_browse_start, transfer_browse_stop, transfer_send, transfer_send_code, transfer_send_cancel]);
