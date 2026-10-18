//! Android side of LAN sync: the multicast lock and the device key wrapped by
//! the Android Keystore (`crates/plugin-net`).

use tauri::AppHandle;
use tauri_plugin_net::NetExt;

/// `secret` encrypted with a non-exportable Keystore key; `None` when the
/// Keystore refused.
pub fn wrap_secret(app: &AppHandle, secret: &[u8]) -> Option<Vec<u8>> {
    app.net().wrap(secret).ok()
}

/// The inverse of [`wrap_secret`].
pub fn unwrap_secret(app: &AppHandle, wrapped: &[u8]) -> Option<Vec<u8>> {
    app.net().unwrap(wrapped).ok()
}

/// Hold the Wi-Fi multicast lock while sync runs (mDNS needs it).
pub fn multicast(app: &AppHandle, on: bool) {
    let _ = app.net().multicast(on);
}

/// Arm background sync (or not), with the notification's texts.
pub fn background(app: &AppHandle, state: &tauri_plugin_net::Background) {
    let _ = app.net().background(state);
}

/// Whether the app is in the background, and the notification button pressed.
pub fn poll(app: &AppHandle) -> Option<tauri_plugin_net::Poll> {
    app.net().poll().ok()
}

/// Android 13+: ask for the permission the background notification needs.
pub fn ask_notifications(app: &AppHandle) {
    let _ = app.net().ask_notifications();
}

/// The system's language (`ru`, `en`, …), asked once.
pub fn language(app: &AppHandle) -> String {
    static LANGUAGE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    LANGUAGE
        .get_or_init(|| app.net().language().unwrap_or_default())
        .clone()
}
