//! The system-wide "+ Transaction" key (plan F4). Off unless the user picks a
//! combination: on Hyprland and the like the window manager owns most keys,
//! and a silent clash would be worse than no shortcut.

#[cfg(desktop)]
use tauri::plugin::TauriPlugin;
use tauri::{AppHandle, Runtime};
#[cfg(desktop)]
use tauri::{Emitter, Manager};
#[cfg(desktop)]
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

/// Command-line flag that opens "+ Transaction" in the running instance — what a
/// Hyprland/Sway binding calls, since Wayland gives apps no global keys.
pub const QUICK_ADD_FLAG: &str = "--quick-add";

/// Whether this desktop lets an app grab keys system-wide. On Wayland the
/// compositor owns them; the X11 grab the plugin uses only sees keys while an
/// XWayland window has focus.
pub fn global_keys_supported() -> bool {
    cfg!(desktop) && (!cfg!(target_os = "linux") || std::env::var("WAYLAND_DISPLAY").is_err())
}

/// The plugin with the one handler every registered combination shares.
#[cfg(desktop)]
pub fn plugin<R: Runtime>() -> TauriPlugin<R> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                show_quick_add(app);
            }
        })
        .build()
}

/// Bring the main window up and tell the UI to open "+ Transaction".
#[cfg(desktop)]
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
#[cfg(desktop)]
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

/// Phones have no system-wide keys: clearing works, setting one does not.
#[cfg(mobile)]
pub fn apply<R: Runtime>(_app: &AppHandle<R>, shortcut: Option<&str>) -> Result<(), String> {
    match shortcut {
        None => Ok(()),
        Some(_) => Err("UNSUPPORTED: global shortcuts are not available on this platform".into()),
    }
}
