//! The identifier of this build. The dev channel (development, local builds)
//! keeps `tauri.conf.json`'s `dev.mwt.desktop`; a release from main is built
//! with `mwt.desktop` (crates/xtask/src/tauri_config.rs), so both install side by
//! side with their own data. Tauri joins it to the data directories; the
//! keyring entries and the quick-add socket are keyed by it as well.
// Only the desktop reads it back (keyring, socket, TradeApp move); mobile
// just records it.
#![cfg_attr(mobile, allow(dead_code))]

use std::sync::OnceLock;

/// `identifier` in tauri.conf.json: the dev channel (checked by a test below).
pub const DEV: &str = "dev.mwt.desktop";

static IDENTIFIER: OnceLock<String> = OnceLock::new();

/// Called first thing in `run`, with the identifier the build was made with.
pub fn set(identifier: &str) {
    let _ = IDENTIFIER.set(identifier.to_owned());
}

pub fn get() -> &'static str {
    IDENTIFIER.get().map_or(DEV, String::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dev_matches_the_tauri_config() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert_eq!(config["identifier"], DEV);
    }

    #[test]
    fn unset_falls_back_to_dev() {
        assert_eq!(get(), DEV);
    }
}
