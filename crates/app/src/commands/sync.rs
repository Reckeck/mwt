//! Device-to-device vault sync (docs/sync.md): status, settings, pairing,
//! confirmations, trusted devices, history and undo. The work is in
//! `crate::sync` (network) and `trade_core` (merge).

use tauri::{AppHandle, State};
use trade_core::appconfig::{UpdateAppSettingsInput, load_settings};
use trade_core::ipc_contract::{
    AllowMergingInput, SyncDecisionInput, SyncPeerView, SyncSettingsInput, SyncStatus,
};
use trade_core::service::UndoReport;
use trade_core::sync::history::AppliedBatch;
use trade_core::sync::journal::SyncJournalEntry;
use trade_core::usecases::sync::peer_sync;

use crate::commands::{AppState, require_vault};
use crate::ipc::IpcError;
use crate::ipc::commands::ipc_commands;
use crate::sync::{self, Order};
use trade_core::sync::history as history_reads;
use trade_core::usecases::sync::repair as sync_repair;
use trade_core::usecases::sync::sync_journal as journal_cases;
use trade_core::usecases::sync::trust as sync_trust;

#[tauri::command]
pub async fn sync_status(app: AppHandle) -> Result<SyncStatus, IpcError> {
    Ok(sync::status(&app))
}

/// Change this device's sync settings; turning sync on or off starts or
/// stops it at once.
#[tauri::command]
pub async fn sync_set_settings(
    app: AppHandle,
    input: SyncSettingsInput,
) -> Result<SyncStatus, IpcError> {
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
    crate::commands::app_settings::save(&dir, &settings)?;
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
pub async fn sync_open_firewall(app: AppHandle) -> Result<SyncStatus, IpcError> {
    sync::firewall::open().await?;
    // On Windows the rule is for the Private profile only: on a Public network
    // it does not apply, so the banner stays (until an incoming link clears
    // it, `link.rs`) and says so. The check runs off the async runtime and
    // only here, never in the status path; it is always false off Windows,
    // where the rule is for any network and the banner clears.
    let public = tauri::async_runtime::spawn_blocking(sync::firewall::on_public_network)
        .await
        .unwrap_or(false);
    {
        let mut hub = sync::hub(&app);
        hub.network_public = public;
        if !cfg!(windows) {
            hub.inbound_blocked = false;
        }
    }
    sync::emit_status(&app);
    Ok(sync::status(&app))
}

/// "Open settings" on the notification hint: the system page where the user
/// allows this app's notifications (Android; elsewhere there is nothing to open).
#[tauri::command]
pub async fn sync_open_notification_settings(app: AppHandle) -> Result<(), IpcError> {
    #[cfg(target_os = "android")]
    if crate::platform_net::open_notification_settings(&app) {
        return Ok(());
    }
    let _ = &app;
    Err(IpcError::from(trade_core::Error::Unsupported))
}

/// "Pair" on the banner: both screens show the code. With a `device_id`
/// instead of a session (the device list, "Pair again"): the device cut
/// this one off, so it is dialed once with the pairing request.
#[tauri::command]
pub async fn sync_pair(
    app: AppHandle,
    session_id: Option<String>,
    device_id: Option<String>,
) -> Result<(), IpcError> {
    match (session_id, device_id) {
        (Some(session_id), _) => send(&app, &session_id, Order::StartPairing),
        (None, Some(device_id)) => sync::pair_again(&app, &device_id).map_err(IpcError::from),
        (None, None) => Err(IpcError::from(trade_core::Error::InvalidInput(
            "a session or a device is needed".into(),
        ))),
    }
}

/// "Not now" on the banner: the device is not offered again until sync restarts.
#[tauri::command]
pub async fn sync_dismiss_offer(app: AppHandle, session_id: String) -> Result<(), IpcError> {
    send(&app, &session_id, Order::Dismiss)
}

/// The answer to the pairing code (`accept` = the codes match).
#[tauri::command]
pub async fn sync_confirm_pairing(
    app: AppHandle,
    input: SyncDecisionInput,
) -> Result<(), IpcError> {
    send(&app, &input.session_id, Order::Decide(input.accept))
}

/// The answer to the list of changes.
#[tauri::command]
pub async fn sync_decide_plan(app: AppHandle, input: SyncDecisionInput) -> Result<(), IpcError> {
    send(&app, &input.session_id, Order::Decide(input.accept))
}

fn send(app: &AppHandle, session_id: &str, order: Order) -> Result<(), IpcError> {
    if sync::send(app, session_id, order) {
        Ok(())
    } else {
        Err(IpcError::from(trade_core::Error::NotFound(
            "the other device is no longer connected".into(),
        )))
    }
}

#[tauri::command]
pub async fn sync_peers(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<SyncPeerView>, IpcError> {
    let status = sync::status(&app);
    let me = status.device_id.clone();
    let online: Vec<String> = status
        .sessions
        .into_iter()
        .map(|session| session.device_id)
        .collect();
    let guard = require_vault(&state)?;
    let vault = guard.as_ref().ok_or(trade_core::Error::RequiresUnlock)?;
    Ok(peer_sync::sync_state(vault)
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
            paused: peer_sync::sync_is_paused(vault, &peer.device_id),
            cut_by: peer_sync::sync_cut_reason(vault, &peer.device_id),
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
) -> Result<(), IpcError> {
    {
        let mut guard = require_vault(&state)?;
        let vault = guard.as_mut().ok_or(trade_core::Error::RequiresUnlock)?;
        sync_trust::sync_forget_peer(vault, &device_id)?;
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
) -> Result<(), IpcError> {
    {
        let mut guard = require_vault(&state)?;
        let vault = guard.as_mut().ok_or(trade_core::Error::RequiresUnlock)?;
        peer_sync::sync_resume_peer(vault, &device_id)?;
    }
    sync::reconnect(&app);
    sync::emit_status(&app);
    Ok(())
}

#[tauri::command]
pub async fn sync_history(state: State<'_, AppState>) -> Result<Vec<AppliedBatch>, IpcError> {
    let guard = require_vault(&state)?;
    let vault = guard.as_ref().ok_or(trade_core::Error::RequiresUnlock)?;
    Ok(history_reads::history(&vault.doc))
}

/// The journal of taken batches (applied, declined, failed), newest first.
#[tauri::command]
pub async fn sync_journal(state: State<'_, AppState>) -> Result<Vec<SyncJournalEntry>, IpcError> {
    let guard = require_vault(&state)?;
    let vault = guard.as_ref().ok_or(trade_core::Error::RequiresUnlock)?;
    Ok(journal_cases::sync_journal(vault))
}

/// Undo one batch; the undo itself reaches the other devices.
#[tauri::command]
pub async fn sync_undo(
    batch_id: String,
    state: State<'_, AppState>,
) -> Result<UndoReport, IpcError> {
    let mut guard = require_vault(&state)?;
    let vault = guard.as_mut().ok_or(trade_core::Error::RequiresUnlock)?;
    peer_sync::sync_undo(vault, &batch_id).map_err(IpcError::from)
}

/// «Allow merging»: clear the «keep» mark of a transaction or a reminder.
#[tauri::command]
pub async fn allow_merging(
    input: AllowMergingInput,
    state: State<'_, AppState>,
) -> Result<(), IpcError> {
    let mut guard = require_vault(&state)?;
    let vault = guard.as_mut().ok_or(trade_core::Error::RequiresUnlock)?;
    sync_repair::allow_merging(vault, input.target, &input.id).map_err(IpcError::from)
}

/// The emergency exit: everything as before the last session's first batch.
#[tauri::command]
pub async fn sync_restore_backup(state: State<'_, AppState>) -> Result<(), IpcError> {
    let mut guard = require_vault(&state)?;
    let vault = guard.as_mut().ok_or(trade_core::Error::RequiresUnlock)?;
    peer_sync::sync_restore_backup(vault).map_err(IpcError::from)
}

ipc_commands!(sync_commands, SYNC_NAMES, commands::sync: [sync_status, sync_set_settings, sync_open_firewall, sync_open_notification_settings, sync_pair, sync_dismiss_offer, sync_confirm_pairing, sync_decide_plan, sync_peers, sync_forget_peer, sync_resume_peer, sync_history, sync_journal, sync_undo, allow_merging, sync_restore_backup]);
