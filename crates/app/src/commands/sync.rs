//! Device-to-device vault sync (docs/sync.md): status, settings, pairing,
//! confirmations, trusted devices, history and undo. The work is in
//! `crate::sync` (network) and `trade_core` (merge).

use tauri::{AppHandle, State};
use trade_core::appconfig::{UpdateAppSettingsInput, load_settings};
use trade_core::ipc_contract::{SyncDecisionInput, SyncPeerView, SyncSettingsInput, SyncStatus};
use trade_core::service::{AppliedBatch, UndoReport};
use trade_core::sync::journal::SyncJournalEntry;

use crate::commands::{AppState, err_to_string, require_vault};
use crate::sync::{self, Order};

#[tauri::command]
pub async fn sync_status(app: AppHandle) -> Result<SyncStatus, String> {
    Ok(sync::status(&app))
}

/// Change this device's sync settings; turning sync on or off starts or
/// stops it at once.
#[tauri::command]
pub async fn sync_set_settings(
    app: AppHandle,
    input: SyncSettingsInput,
) -> Result<SyncStatus, String> {
    let dir = crate::commands::app_settings::config_dir(&app);
    let mut settings = load_settings(&dir);
    let mut prefs = settings.sync;
    if let Some(enabled) = input.enabled {
        prefs.enabled = enabled;
    }
    if let Some(confirm) = input.confirm_first {
        prefs.confirm_first = confirm;
    }
    if let Some(prefer) = input.prefer_main {
        prefs.prefer_main = prefer;
    }
    if let Some(minutes) = input.background_lock_min {
        prefs.background_lock_min = Some(minutes);
    }
    let restart =
        prefs.enabled != settings.sync.enabled || prefs.prefer_main != settings.sync.prefer_main;
    settings.apply(&UpdateAppSettingsInput {
        sync: Some(prefs),
        device_name: input.device_name.clone(),
        ..Default::default()
    });
    settings.ensure_device_id();
    crate::commands::app_settings::save(&dir, &settings).map_err(err_to_string)?;
    #[cfg(target_os = "android")]
    if input.enabled == Some(true) {
        crate::platform_net::ask_notifications(&app);
    }
    // Sync never starts on the sandbox's demo data.
    if (restart || input.device_name.is_some()) && !crate::commands::sandbox::is_active_app(&app) {
        sync::restart(&app);
    }
    Ok(sync::status(&app))
}

/// "Open port": let the other devices through this device's firewall
/// (asks for the system password).
#[tauri::command]
pub async fn sync_open_firewall(app: AppHandle) -> Result<SyncStatus, String> {
    sync::firewall::open().await.map_err(err_to_string)?;
    sync::hub(&app).inbound_blocked = false;
    sync::emit_status(&app);
    Ok(sync::status(&app))
}

/// "Pair" on the banner: both screens show the code. With a `device_id`
/// instead of a session (the device list, "Pair again"): the device cut
/// this one off, so it is dialed once with the pairing request.
#[tauri::command]
pub async fn sync_pair(
    app: AppHandle,
    session_id: Option<String>,
    device_id: Option<String>,
) -> Result<(), String> {
    match (session_id, device_id) {
        (Some(session_id), _) => send(&app, &session_id, Order::StartPairing),
        (None, Some(device_id)) => sync::pair_again(&app, &device_id).map_err(err_to_string),
        (None, None) => Err(err_to_string(trade_core::Error::InvalidInput(
            "a session or a device is needed".into(),
        ))),
    }
}

/// "Not now" on the banner: the device is not offered again until sync restarts.
#[tauri::command]
pub async fn sync_dismiss_offer(app: AppHandle, session_id: String) -> Result<(), String> {
    send(&app, &session_id, Order::Dismiss)
}

/// The answer to the pairing code (`accept` = the codes match).
#[tauri::command]
pub async fn sync_confirm_pairing(app: AppHandle, input: SyncDecisionInput) -> Result<(), String> {
    send(&app, &input.session_id, Order::Decide(input.accept))
}

/// The answer to the list of changes.
#[tauri::command]
pub async fn sync_decide_plan(app: AppHandle, input: SyncDecisionInput) -> Result<(), String> {
    send(&app, &input.session_id, Order::Decide(input.accept))
}

fn send(app: &AppHandle, session_id: &str, order: Order) -> Result<(), String> {
    if sync::send(app, session_id, order) {
        Ok(())
    } else {
        Err(err_to_string(trade_core::Error::NotFound(
            "the other device is no longer connected".into(),
        )))
    }
}

#[tauri::command]
pub async fn sync_peers(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<SyncPeerView>, String> {
    let status = sync::status(&app);
    let me = status.device_id.clone();
    let online: Vec<String> = status
        .sessions
        .into_iter()
        .map(|session| session.device_id)
        .collect();
    let guard = require_vault(&state).map_err(err_to_string)?;
    let vault = guard
        .as_ref()
        .ok_or_else(|| err_to_string(trade_core::Error::RequiresUnlock))?;
    Ok(vault
        .sync_state()
        .peers
        .iter()
        // The list also carries this device as the others trust it.
        .filter(|peer| peer.device_id != me)
        .map(|peer| SyncPeerView {
            device_id: peer.device_id.clone(),
            name: peer.name.clone(),
            paired_at: peer.paired_at.clone(),
            last_synced_at: peer.last_synced_at.clone(),
            online: online.contains(&peer.device_id),
            paused: vault.sync_is_paused(&peer.device_id),
            cut_by: vault.sync_cut_reason(&peer.device_id),
            forgotten: peer.revoked.is_some(),
        })
        .collect())
}

/// "Forget device": refused here at once, everywhere once it spreads.
#[tauri::command]
pub async fn sync_forget_peer(
    app: AppHandle,
    device_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    {
        let mut guard = require_vault(&state).map_err(err_to_string)?;
        let vault = guard
            .as_mut()
            .ok_or_else(|| err_to_string(trade_core::Error::RequiresUnlock))?;
        vault.sync_forget_peer(&device_id).map_err(err_to_string)?;
    }
    sync::forget_device(&app, &device_id);
    sync::emit_status(&app);
    Ok(())
}

/// "Resume": lift the pause set by declining a list of changes and
/// connect again; the list is asked about again.
#[tauri::command]
pub async fn sync_resume_peer(
    app: AppHandle,
    device_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    {
        let mut guard = require_vault(&state).map_err(err_to_string)?;
        let vault = guard
            .as_mut()
            .ok_or_else(|| err_to_string(trade_core::Error::RequiresUnlock))?;
        vault.sync_resume_peer(&device_id).map_err(err_to_string)?;
    }
    sync::reconnect(&app);
    sync::emit_status(&app);
    Ok(())
}

#[tauri::command]
pub async fn sync_history(state: State<'_, AppState>) -> Result<Vec<AppliedBatch>, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    let vault = guard
        .as_ref()
        .ok_or_else(|| err_to_string(trade_core::Error::RequiresUnlock))?;
    Ok(vault.sync_history())
}

/// The journal of taken batches (applied, declined, failed), newest first.
#[tauri::command]
pub async fn sync_journal(state: State<'_, AppState>) -> Result<Vec<SyncJournalEntry>, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    let vault = guard
        .as_ref()
        .ok_or_else(|| err_to_string(trade_core::Error::RequiresUnlock))?;
    Ok(vault.sync_journal())
}

/// Undo one batch; the undo itself reaches the other devices.
#[tauri::command]
pub async fn sync_undo(batch_id: String, state: State<'_, AppState>) -> Result<UndoReport, String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    let vault = guard
        .as_mut()
        .ok_or_else(|| err_to_string(trade_core::Error::RequiresUnlock))?;
    vault.sync_undo(&batch_id).map_err(err_to_string)
}

/// The emergency exit: everything as before the last session's first batch.
#[tauri::command]
pub async fn sync_restore_backup(state: State<'_, AppState>) -> Result<(), String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    let vault = guard
        .as_mut()
        .ok_or_else(|| err_to_string(trade_core::Error::RequiresUnlock))?;
    vault.sync_restore_backup().map_err(err_to_string)
}
