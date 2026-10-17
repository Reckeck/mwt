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
