//! The system-wide «+ Операция» key (plan Ф4). Off unless the user picks a
//! combination: on Hyprland and the like the window manager owns most keys,
//! and a silent clash would be worse than no shortcut.

use tauri::plugin::TauriPlugin;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

/// The plugin with the one handler every registered combination shares.
pub fn plugin<R: Runtime>() -> TauriPlugin<R> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                show_quick_add(app);
            }
        })
        .build()
}

/// Bring the main window up and tell the UI to open «+ Операция».
pub fn show_quick_add<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        window.show().ok();
        window.unminimize().ok();
        window.set_focus().ok();
    }
    app.emit("quick-add", ()).ok();
}

/// Replace whatever is registered with `shortcut` (`None` = nothing).
/// The error is an IPC string: the combination is malformed, taken by
/// another program, or global keys are unavailable on this desktop.
pub fn apply<R: Runtime>(app: &AppHandle<R>, shortcut: Option<&str>) -> Result<(), String> {
    let keys = app.global_shortcut();
    keys.unregister_all()
        .map_err(|err| format!("SHORTCUT_FAILED: {err}"))?;
    if let Some(raw) = shortcut {
        keys.register(raw)
            .map_err(|err| format!("SHORTCUT_FAILED: {err}"))?;
    }
    Ok(())
}
