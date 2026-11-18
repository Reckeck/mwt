//! Wiping the module out of the vault.
//!
//! Owner: spec E12, with `service/cryptotrack/wipe.rs`. The wipe clears every
//! `crypto_*` field, drops the device secrets and the biometric binding, marks
//! `crypto_wiped_at` so a device that was offline cannot hand the data back,
//! and only then lets the protection go below level 3.
//!
//! The vault's own part of it belongs to core. What is left here is the piece
//! the vault cannot reach: the quick-unlock binding, which the OS keeps and
//! the document does not.

use tauri::{AppHandle, State};

use trade_core::ipc_contract::cryptotrack::wipe::{
    CryptoWipeInput, CryptoWipePreview, CryptoWipeReport,
};

use crate::commands::AppState;
use crate::commands::app_settings;
use crate::commands::planning::{with_vault, with_vault_mut};
use crate::ipc::IpcError;
use crate::ipc::commands::ipc_commands;

/// What a wipe would remove: the records per collection, the wallets that lose
/// their link to the module and are archived, what the capital loses and the
/// newest backup the user already has.
///
/// Counted on demand and stored nowhere, so the screen may ask at any time
/// without changing anything.
#[tauri::command]
pub async fn crypto_wipe_preview(
    state: State<'_, AppState>,
) -> Result<CryptoWipePreview, IpcError> {
    with_vault(&state, |vault| Ok(vault.crypto_wipe_preview()))
}

/// Wipe the module. `input.confirm` is the fixed token the window sends once
/// the user has typed the word; without it nothing happens.
///
/// A backup is taken and opened again before anything is removed, so a copy
/// that does not decrypt ends the wipe with the vault whole. Afterwards the
/// quick-unlock binding of this device is dropped too: the user said the
/// module and its secrets are to go from this device, and that binding is a
/// secret the OS keeps on its behalf.
#[tauri::command]
pub async fn crypto_wipe_module(
    input: CryptoWipeInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<CryptoWipeReport, IpcError> {
    let report =
        with_vault_mut::<_, IpcError>(&state, |vault| vault.crypto_wipe_module(&input.confirm))?;
    if report.wiped_at.is_some() {
        crate::cryptotrack::jobs::forget_live_positions();
        let path = with_vault::<_, IpcError>(&state, |vault| Ok(vault.path.clone()))?;
        crate::biometric::system_gate(&app_settings::config_dir(&app))
            .disable(&path.to_string_lossy());
    }
    Ok(report)
}

ipc_commands!(cryptotrack_wipe_commands, CRYPTOTRACK_WIPE_NAMES, commands::cryptotrack::wipe: [crypto_wipe_preview, crypto_wipe_module]);
