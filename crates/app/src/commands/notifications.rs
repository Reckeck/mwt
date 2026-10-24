//! The notification center: the log of what was shown, read marks, a test
//! notice and whether the system lets the app show any.

use tauri::{AppHandle, State};
#[cfg(desktop)]
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_notification::{NotificationExt, PermissionState};

use trade_core::model::notify::{NoticeCategory, NoticeRecord};
use trade_core::service::{notice, notice_title};

use crate::commands::{AppState, err_to_string, require_vault};

/// Every notice shown so far, newest first.
#[tauri::command]
pub async fn list_notifications(state: State<'_, AppState>) -> Result<Vec<NoticeRecord>, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    Ok(guard.as_ref().unwrap().notifications())
}

/// Mark notices read: the keys given, or every one.
#[tauri::command]
pub async fn mark_notifications_read(
    keys: Option<Vec<String>>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .mark_notifications_read(keys.as_deref())
        .map_err(err_to_string)
}

/// Show a notice right now, to check that they come through.
#[tauri::command]
pub async fn send_test_notification(app: AppHandle) -> Result<(), String> {
    crate::commands::sandbox::refuse_app(&app, "system notifications")?;
    let ru = crate::scheduler::russian(&app);
    let record = notice(
        NoticeCategory::Reminder,
        format!("test:{}", trade_core::calendar::now_secs()),
        notice_title(NoticeCategory::Reminder, ru),
        if ru {
            "Уведомления работают".into()
        } else {
            "Notifications work".into()
        },
        None,
        trade_core::model::vault::now_iso(),
    );
    crate::scheduler::deliver(&app, vec![record], true);
    Ok(())
}

/// `granted`, `denied` or `prompt`: whether the system shows the app's notices.
#[tauri::command]
pub async fn notification_permission(app: AppHandle, request: bool) -> Result<String, String> {
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
pub async fn get_autostart(app: AppHandle) -> Result<bool, String> {
    app.autolaunch().is_enabled().map_err(|err| err.to_string())
}

/// Start with the system (hidden in the tray, so reminders come after a
/// reboot) or not.
#[cfg(desktop)]
#[tauri::command]
pub async fn set_autostart(app: AppHandle, enabled: bool) -> Result<bool, String> {
    let launcher = app.autolaunch();
    if enabled {
        launcher.enable().map_err(|err| err.to_string())?;
        crate::autostart::guard_entry(&app);
    } else {
        launcher.disable().map_err(|err| err.to_string())?;
    }
    launcher.is_enabled().map_err(|err| err.to_string())
}

/// A phone starts apps itself; the app never does.
#[cfg(mobile)]
#[tauri::command]
pub async fn get_autostart(_app: AppHandle) -> Result<bool, String> {
    Ok(false)
}

#[cfg(mobile)]
#[tauri::command]
pub async fn set_autostart(_app: AppHandle, enabled: bool) -> Result<bool, String> {
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
pub async fn notification_storage(state: State<'_, AppState>) -> Result<String, String> {
    let persistent = state
        .notify
        .lock()
        .map(|notify| notify.persistent)
        .unwrap_or(false);
    Ok(if persistent { "keyring" } else { "memory" }.into())
}
