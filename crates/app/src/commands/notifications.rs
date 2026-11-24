//! The notification center: the log of what was shown, read marks, a test
//! notice and whether the system lets the app show any.

use tauri::{AppHandle, State};
#[cfg(desktop)]
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_notification::{NotificationExt, PermissionState};

use mwt_model::{NoticeCategory, NoticeRecord, notice, notice_title};
use mwt_planning::notifications as notification_reads;
use mwt_usecases::usecases::planning::notifications;

use crate::commands::AppState;
use crate::commands::{with_vault, with_vault_mut};
use crate::ipc::IpcError;
use crate::ipc::commands::ipc_commands;

/// Every notice shown so far, newest first.
#[tauri::command]
pub async fn list_notifications(state: State<'_, AppState>) -> Result<Vec<NoticeRecord>, IpcError> {
    with_vault(&state, |vault| {
        Ok(notification_reads::notifications(&vault.doc))
    })
}

/// Mark notices read: the keys given, or every one.
#[tauri::command]
pub async fn mark_notifications_read(
    keys: Option<Vec<String>>,
    state: State<'_, AppState>,
) -> Result<(), IpcError> {
    with_vault_mut(&state, |vault| {
        notifications::mark_notifications_read(vault, keys.as_deref())
    })
}

/// Show a notice right now, to check that they come through.
#[tauri::command]
pub async fn send_test_notification(app: AppHandle) -> Result<(), IpcError> {
    crate::commands::sandbox::refuse_app(&app, "system notifications")?;
    let ru = crate::scheduler::russian(&app);
    let record = notice(
        NoticeCategory::Reminder,
        format!("test:{}", mwt_base::calendar::now_secs()),
        notice_title(NoticeCategory::Reminder, ru),
        if ru {
            "Уведомления работают".into()
        } else {
            "Notifications work".into()
        },
        None,
        mwt_model::now_iso(),
    );
    crate::scheduler::deliver(&app, vec![record], true);
    Ok(())
}

/// `granted`, `denied` or `prompt`: whether the system shows the app's notices.
#[tauri::command]
pub async fn notification_permission(app: AppHandle, request: bool) -> Result<String, IpcError> {
    let mut state = app
        .notification()
        .permission_state()
        .map_err(|err| err.to_string())?;
    if request && state != PermissionState::Granted {
        state = app
            .notification()
            .request_permission()
            .map_err(|err| err.to_string())?;
    }
    Ok(match state {
        PermissionState::Granted => "granted",
        PermissionState::Denied => "denied",
        _ => "prompt",
    }
    .into())
}

/// Whether the app starts with the system, hidden in the tray.
#[cfg(desktop)]
#[tauri::command]
pub async fn get_autostart(app: AppHandle) -> Result<bool, IpcError> {
    Ok(app
        .autolaunch()
        .is_enabled()
        .map_err(|err| err.to_string())?)
}

/// Start with the system (hidden in the tray, so reminders come after a
/// reboot) or not.
#[cfg(desktop)]
#[tauri::command]
pub async fn set_autostart(app: AppHandle, enabled: bool) -> Result<bool, IpcError> {
    let launcher = app.autolaunch();
    if enabled {
        launcher.enable().map_err(|err| err.to_string())?;
        crate::autostart::guard_entry(&app);
    } else {
        launcher.disable().map_err(|err| err.to_string())?;
    }
    Ok(launcher.is_enabled().map_err(|err| err.to_string())?)
}

/// A phone starts apps itself; the app never does.
#[cfg(mobile)]
#[tauri::command]
pub async fn get_autostart(_app: AppHandle) -> Result<bool, IpcError> {
    Ok(false)
}

#[cfg(mobile)]
#[tauri::command]
pub async fn set_autostart(_app: AppHandle, enabled: bool) -> Result<bool, IpcError> {
    if enabled {
        return Err(
            "UNSUPPORTED: starting with the system is not available on this platform".into(),
        );
    }
    Ok(false)
}

/// Whether notices of a locked vault survive a restart (a keyring answered)
/// or are kept in memory only.
#[tauri::command]
pub async fn notification_storage(state: State<'_, AppState>) -> Result<String, IpcError> {
    let persistent = state
        .notify
        .lock()
        .map(|notify| notify.persistent)
        .unwrap_or(false);
    Ok(if persistent { "keyring" } else { "memory" }.into())
}

ipc_commands!(notifications_commands, NOTIFICATIONS_NAMES, commands::notifications: [list_notifications, mark_notifications_read, send_test_notification, notification_permission, get_autostart, set_autostart, notification_storage]);
