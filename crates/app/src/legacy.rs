//! The app used to be TradeApp (`dev.tradeapp.desktop`). On the first start as
//! Multi Wallet Tracker its directories, the keyring key and the autostart
//! entry move over once, so the vault, settings and reminders stay where the
//! user left them.

use std::path::PathBuf;

use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;

/// `identifier` in tauri.conf.json (checked by a test below).
pub const IDENTIFIER: &str = "dev.mwt.desktop";
pub const LEGACY_IDENTIFIER: &str = "dev.tradeapp.desktop";
/// The old `productName`: what the autostart entry was registered under.
const LEGACY_NAME: &str = "TradeApp";

/// Move the old directories. Runs before the builder: Tauri opens the window,
/// and with it the webview storage in the new directory, before `setup`.
/// The bases are the ones Tauri's path resolver joins the identifier to.
pub fn migrate_dirs() {
    let bases = [
        dirs::data_dir(),
        dirs::config_dir(),
        dirs::data_local_dir(),
        dirs::cache_dir(),
    ];
    let mut moves: Vec<(PathBuf, PathBuf)> = Vec::new();
    for base in bases.into_iter().flatten() {
        let new = base.join(IDENTIFIER);
        // On Windows and macOS several bases are the same directory.
        if moves.iter().any(|(_, done)| *done == new) {
            continue;
        }
        let old = base.join(LEGACY_IDENTIFIER);
        match trade_core::app_dirs::move_legacy_dir(&old, &new) {
            Ok(true) => moves.push((old, new)),
            Ok(false) => {}
            Err(err) => eprintln!("moving {} to {}: {err}", old.display(), new.display()),
        }
    }
    if let (false, Some(config)) = (moves.is_empty(), dirs::config_dir()) {
        crate::commands::vault_recents::rebase_paths(&config.join(IDENTIFIER), &moves);
    }
}

/// Start with the system under the new name if the old one did.
pub fn migrate_autostart(app: &AppHandle) {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let mut builder = auto_launch::AutoLaunchBuilder::new();
    builder
        .set_app_name(LEGACY_NAME)
        .set_app_path(&exe.display().to_string())
        .set_args(&["--hidden"]);
    #[cfg(target_os = "macos")]
    builder.set_use_launch_agent(true);
    let Ok(legacy) = builder.build() else {
        return;
    };
    if legacy.is_enabled().unwrap_or(false)
        && legacy.disable().is_ok()
        && let Err(err) = app.autolaunch().enable()
    {
        eprintln!("autostart under the new name: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifier_matches_the_tauri_config() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert_eq!(config["identifier"], IDENTIFIER);
        assert_ne!(IDENTIFIER, LEGACY_IDENTIFIER);
    }
}
