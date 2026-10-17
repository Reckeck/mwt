//! Android and iOS: a picked document is a `content://` URI (Android) or a
//! security-scoped `file://` URL (iOS). Neither can be replaced atomically or
//! given a lock and `.bak` files next to it, so the vault the app works on is
//! a copy in app storage.

use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;
use trade_core::error::{Error, Result};
use trade_core::ipc_contract::PickedVault;
use trade_core::vault_io::VaultLocation;
use trade_core::vault_io::import::import_into;

use super::{location_of, read_location, vaults_dir};

/// Open = copy the picked document into app storage and open the copy.
/// Create has no dialog here: the form asks for a name
/// ([`super::resolve_new_vault`]).
pub fn pick_vault(app: &AppHandle, save: bool) -> Result<Option<PickedVault>> {
    if save {
        return Err(Error::Unsupported);
    }
    // No MIME type exists for `.wallet`, so the picker shows every file and
    // the header check below refuses anything that is not a vault.
    let Some(location) = app.dialog().file().blocking_pick_file().map(location_of) else {
        return Ok(None);
    };
    let bytes = read_location(app, &location)?;
    let path = import_into(&vaults_dir(app)?, &location.display_name(), &bytes)?;
    let local = VaultLocation::Path(path);
    Ok(Some(PickedVault {
        path: local.to_string(),
        display_name: local.display_name(),
        imported: true,
    }))
}

#[cfg(target_os = "android")]
pub fn pick_link_target(
    app: &AppHandle,
    existing: bool,
    name: &str,
) -> Result<Option<VaultLocation>> {
    use tauri_plugin_saf::SafExt;
    let saf = app.saf();
    let picked = if existing {
        saf.open_document()
    } else {
        saf.create_document(name)
    }
    .map_err(|err| Error::Io(err.to_string()))?;
    Ok(picked.map(|uri| VaultLocation::parse(&uri)))
}

#[cfg(target_os = "android")]
pub fn release_link_target(app: &AppHandle, location: &VaultLocation) {
    use tauri_plugin_saf::SafExt;
    if let VaultLocation::Uri(uri) = location {
        let _ = app.saf().release(uri);
    }
}

/// iOS: the dialog's security-scoped URL; it may need picking again after
/// a restart (no persistable grant is taken yet).
#[cfg(not(target_os = "android"))]
pub fn pick_link_target(
    app: &AppHandle,
    existing: bool,
    name: &str,
) -> Result<Option<VaultLocation>> {
    let dialog = app.dialog().file();
    let picked = if existing {
        dialog.blocking_pick_file()
    } else {
        dialog.set_file_name(name).blocking_save_file()
    };
    Ok(picked.map(location_of))
}

#[cfg(not(target_os = "android"))]
pub fn release_link_target(_app: &AppHandle, _location: &VaultLocation) {}
