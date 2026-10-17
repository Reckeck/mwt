//! The one place in the app that deals with file dialogs, document URIs and
//! where vaults live — for Linux, Windows, macOS, Android and iOS. (Plain app
//! files — settings, recents, notices — stay in `app_config_dir()`.)
//!
//! Core works on a local vault file only (`trade_core::vault_io`). This module
//! gets one there: on desktops the picked file is used where it lies, on
//! Android/iOS a picked document is copied into app storage, and a document
//! can be linked to receive a copy after every save (`links`, `mirror_sink`).
//! See `docs/storage-platforms.md`.

#[cfg(desktop)]
mod desktop;
pub mod links;
mod mirror_sink;
#[cfg(mobile)]
mod mobile;

#[cfg(desktop)]
use desktop as platform;
#[cfg(mobile)]
use mobile as platform;

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;

use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::{DialogExt, FilePath};
use tauri_plugin_fs::{FsExt, OpenOptions};
use trade_core::error::{Error, Result};
use trade_core::ipc_contract::{PickedVault, VaultStorage};
use trade_core::storage::ensure_wallet_ext;
use trade_core::vault_io::VaultLocation;
use trade_core::vault_io::import::{import_into, safe_file_name};

pub use mirror_sink::attach_mirror;

/// File name of the vault the app suggests first.
const DEFAULT_VAULT_FILE: &str = "vault.wallet";

/// How vault files are kept on this platform.
pub fn storage() -> VaultStorage {
    if cfg!(mobile) {
        VaultStorage::App
    } else {
        VaultStorage::Files
    }
}

/// Where the default, new-by-name and imported vaults live: the app data dir
/// (XDG data / `%APPDATA%` / `~/Library/Application Support` / Android and
/// iOS app storage).
pub fn vaults_dir(app: &AppHandle) -> Result<PathBuf> {
    app.path()
        .app_data_dir()
        .map_err(|err| Error::Io(format!("app data dir: {err}")))
}

pub fn default_vault_path(app: &AppHandle) -> PathBuf {
    vaults_dir(app)
        .expect("app data dir")
        .join(DEFAULT_VAULT_FILE)
}

/// The local file a new vault goes to. A bare name (what the mobile form
/// asks for) lands in [`vaults_dir`]; a full path is kept. A document URI
/// cannot be a vault: it is linked after creating instead.
pub fn resolve_new_vault(app: &AppHandle, raw: &str) -> Result<PathBuf> {
    match VaultLocation::parse(raw) {
        VaultLocation::Uri(_) => Err(Error::InvalidInput(
            "a vault is created in app storage; link a document to keep a copy there".into(),
        )),
        VaultLocation::Path(_) if is_bare_name(raw) => {
            Ok(vaults_dir(app)?.join(ensure_wallet_ext(Path::new(&safe_file_name(raw.trim())))))
        }
        VaultLocation::Path(path) => Ok(path),
    }
}

/// The local file to open for what the UI handed over. A document URI (an
/// old recents entry, a share intent) is copied into app storage first.
pub fn resolve_vault_to_open(app: &AppHandle, raw: &str) -> Result<PathBuf> {
    match VaultLocation::parse(raw) {
        VaultLocation::Path(path) => Ok(path),
        location @ VaultLocation::Uri(_) => {
            let bytes = read_location(app, &location)?;
            import_into(&vaults_dir(app)?, &location.display_name(), &bytes)
        }
    }
}

/// System dialog for a vault to open (`save = false`) or create.
pub fn pick_vault(app: &AppHandle, save: bool) -> Result<Option<PickedVault>> {
    platform::pick_vault(app, save)
}

/// Save dialog for an export (CSV, JSON); returns what to show as "saved to".
pub fn save_bytes_dialog(
    app: &AppHandle,
    file_name: &str,
    filter: &str,
    extensions: &[&str],
    bytes: &[u8],
) -> Result<Option<String>> {
    let Some(picked) = app
        .dialog()
        .file()
        .add_filter(filter, extensions)
        .set_file_name(file_name)
        .blocking_save_file()
    else {
        return Ok(None);
    };
    let location = location_of(picked);
    write_location(app, &location, bytes)?;
    Ok(Some(shown(&location)))
}

/// Read a whole file or document.
pub fn read_location(app: &AppHandle, location: &VaultLocation) -> Result<Vec<u8>> {
    let mut options = OpenOptions::new();
    options.read(true);
    let mut file = app
        .fs()
        .open(file_path(location), options)
        .map_err(|err| io_error(location, &err))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|err| io_error(location, &err))?;
    Ok(bytes)
}

/// Replace a file or document with `bytes`. A path is replaced atomically;
/// a document is truncated and rewritten (the platform offers nothing else).
pub fn write_location(app: &AppHandle, location: &VaultLocation, bytes: &[u8]) -> Result<()> {
    if let VaultLocation::Path(path) = location {
        return trade_core::crypto::atomic_write(path, bytes);
    }
    let mut options = OpenOptions::new();
    options.read(false).write(true).truncate(true);
    let mut file = app
        .fs()
        .open(file_path(location), options)
        .map_err(|err| io_error(location, &err))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|err| io_error(location, &err))
}

/// Ask for the document to keep the vault's copy in: an existing one (it
/// will be replaced) or a new one. The grant outlives a restart where the
/// platform has such a thing (Android).
pub fn pick_link_target(
    app: &AppHandle,
    existing: bool,
    name: &str,
) -> Result<Option<VaultLocation>> {
    platform::pick_link_target(app, existing, name)
}

/// Give back the long-lived grant of a document that is no longer linked.
pub fn release_link_target(app: &AppHandle, location: &VaultLocation) {
    platform::release_link_target(app, location);
}

/// What a dialog returned, as a path where it is one.
pub(crate) fn location_of(picked: FilePath) -> VaultLocation {
    match picked {
        FilePath::Path(path) => VaultLocation::Path(path),
        FilePath::Url(url) => VaultLocation::parse(url.as_str()),
    }
}

/// A path in full (the user knows where it is), a document by its name.
pub(crate) fn shown(location: &VaultLocation) -> String {
    match location {
        VaultLocation::Path(path) => path.display().to_string(),
        VaultLocation::Uri(_) => location.display_name(),
    }
}

fn file_path(location: &VaultLocation) -> FilePath {
    match location {
        VaultLocation::Path(path) => FilePath::Path(path.clone()),
        VaultLocation::Uri(uri) => {
            FilePath::from_str(uri).unwrap_or_else(|_| FilePath::Path(PathBuf::from(uri)))
        }
    }
}

fn io_error(location: &VaultLocation, err: &std::io::Error) -> Error {
    if err.kind() == std::io::ErrorKind::NotFound {
        Error::NotFound(shown(location))
    } else {
        Error::Io(format!("{}: {err}", shown(location)))
    }
}

fn is_bare_name(raw: &str) -> bool {
    let raw = raw.trim();
    !raw.is_empty() && !raw.contains(['/', '\\'])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_names_are_told_from_paths() {
        assert!(is_bare_name("My money"));
        assert!(is_bare_name("work.wallet"));
        assert!(!is_bare_name("/home/me/work.wallet"));
        assert!(!is_bare_name(r"C:\Users\me\work.wallet"));
        assert!(!is_bare_name("  "));
    }

    #[test]
    fn dialog_results_become_locations() {
        let url = tauri::Url::parse("content://docs/document/primary%3Av.wallet").unwrap();
        assert!(location_of(FilePath::Url(url)).is_uri());
        let file = tauri::Url::parse("file:///tmp/v.wallet").unwrap();
        assert_eq!(
            location_of(FilePath::Url(file)),
            VaultLocation::Path(PathBuf::from("/tmp/v.wallet"))
        );
    }
}
