//! Linux, Windows, macOS: native dialogs return paths, and a vault is
//! opened and written where it lies.

use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;
use trade_core::error::Result;
use trade_core::ipc_contract::PickedVault;
use trade_core::storage::ensure_wallet_ext;
use trade_core::vault_io::VaultLocation;

use super::location_of;

const FILTER: &str = "Multi Wallet Tracker vault";

pub fn pick_vault(app: &AppHandle, save: bool) -> Result<Option<PickedVault>> {
    let ru = crate::scheduler::russian(app);
    let dialog = app.dialog().file().add_filter(FILTER, &["wallet"]);
    let picked = if save {
        dialog
            .set_file_name("vault.wallet")
            .set_title(crate::scheduler::pick(
                ru,
                "Сохранить vault как…",
                "Save vault as…",
            ))
            .blocking_save_file()
    } else {
        dialog
            .set_title(crate::scheduler::pick(ru, "Открыть vault", "Open vault"))
            .blocking_pick_file()
    };
    let Some(location) = picked.map(location_of) else {
        return Ok(None);
    };
    let location = match location {
        // The save dialog returns exactly what was typed; a vault always gets its extension.
        VaultLocation::Path(path) if save => VaultLocation::Path(ensure_wallet_ext(&path)),
        other => other,
    };
    Ok(Some(PickedVault {
        path: location.to_string(),
        display_name: location.display_name(),
        imported: false,
    }))
}

pub fn pick_link_target(
    app: &AppHandle,
    existing: bool,
    name: &str,
) -> Result<Option<VaultLocation>> {
    let dialog = app.dialog().file().add_filter(FILTER, &["wallet"]);
    let picked = if existing {
        dialog.blocking_pick_file()
    } else {
        dialog.set_file_name(name).blocking_save_file()
    };
    Ok(picked.map(location_of).map(|location| match location {
        VaultLocation::Path(path) => VaultLocation::Path(ensure_wallet_ext(&path)),
        other => other,
    }))
}

/// Paths carry no grant to give back.
pub fn release_link_target(_app: &AppHandle, _location: &VaultLocation) {}
