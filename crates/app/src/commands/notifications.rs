//! The notification center: the log of what was shown, read marks, a test
//! notice and whether the system lets the app show any.

use tauri::{AppHandle, State};
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
