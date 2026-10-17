//! Android side of LAN sync: keeping the device key wrapped by the Android
//! Keystore. The plugin lives in `crates/plugin-net`.

use tauri::AppHandle;

/// `secret` encrypted with a non-exportable Keystore key; `None` when the
/// Keystore refused.
pub fn wrap_secret(app: &AppHandle, secret: &[u8]) -> Option<Vec<u8>> {
    tauri_plugin_net::wrap(app, secret).ok()
}

/// The inverse of [`wrap_secret`].
pub fn unwrap_secret(app: &AppHandle, wrapped: &[u8]) -> Option<Vec<u8>> {
    tauri_plugin_net::unwrap(app, wrapped).ok()
}
