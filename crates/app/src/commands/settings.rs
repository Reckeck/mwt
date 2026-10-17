use std::path::PathBuf;
use std::process::{Command, Stdio};

use tauri::{AppHandle, State};

use trade_core::ipc_contract::{DevDumpVaultOutput, UpdateSettingsInput};
use trade_core::model::vault::Settings;
use trade_core::storage::{SnapshotFormat, VaultFile};

use crate::commands::{AppState, err_to_string, require_vault};

/// Pretty-print the unlocked vault document as JSON.
fn vault_json(state: &AppState) -> Result<String, String> {
    let guard = require_vault(state).map_err(err_to_string)?;
    let json = VaultFile::export_json(&guard.as_ref().unwrap().doc)
        .map_err(|err| format!("{}: {}", err.code(), err))?;
    Ok(json)
}

/// Scratch file the "open in editor" button hands to `$EDITOR`.
///
/// Deliberately *not* in the app data dir: it holds the decrypted document and
/// must never be picked up as a vault candidate.
fn editor_scratch_path() -> PathBuf {
    std::env::temp_dir().join("tradeapp-vault.json")
}

/// First editor from the environment, falling back to the usual suspects.
fn editor_command() -> String {
    for var in ["VISUAL", "EDITOR"] {
        if let Ok(value) = std::env::var(var)
            && !value.trim().is_empty()
        {
            return value;
        }
    }
    "vi".to_string()
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<Settings, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    Ok(guard.as_ref().unwrap().doc.settings.clone())
}

#[tauri::command]
pub async fn update_settings(
    input: UpdateSettingsInput,
    state: State<'_, AppState>,
) -> Result<Settings, String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .update_settings(input)
        .map_err(err_to_string)
}

#[tauri::command]
pub async fn dev_dump_vault(state: State<'_, AppState>) -> Result<DevDumpVaultOutput, String> {
    Ok(DevDumpVaultOutput {
        json: vault_json(&state)?,
    })
}

/// Ask for a destination and write the plaintext vault document there.
///
/// The webview cannot download files itself (WebKitGTK drops
/// `URL.createObjectURL` + `<a download>`), so the export happens on the Rust
/// side: a native save dialog followed by a plain write.
///
/// Returns `None` when the user cancels the dialog. The vault guard is
/// released before the dialog opens — holding it would block every other
/// command for as long as the modal is up (AGENTS.md rule 3).
#[tauri::command]
pub async fn dev_export_vault(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<String>, String> {
    let json = vault_json(&state)?;

    crate::platform_fs::save_bytes_dialog(&app, "vault.json", "JSON", &["json"], json.as_bytes())
        .map_err(err_to_string)
}

/// Validate `json` and, if it is a well-formed current `VaultDoc`, replace the
/// live document with it.
///
/// Returns the canonical re-serialization of what is now on disk, so the
/// frontend can resync its buffer instead of trusting its own formatting.
///
/// The previous file is copied to `<vault>.bak` first — this command
/// overwrites the whole encrypted document.
///
/// Rejects (without touching the vault) on invalid JSON, an unknown schema,
/// or a wrong payload version: an unparsable document is the whole point of
/// the editor's draft handling, and a half-written vault is worse than a
/// rejected edit.
#[tauri::command]
pub async fn dev_apply_vault_json(
    json: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    // An edit that does not fit the schema is a typo in the editor, not an old
    // file: report it as a parse error, not as an incompatible vault.
    let decoded = trade_core::storage::decode(json.as_bytes(), SnapshotFormat::Json)
        .map_err(|err| match err {
            trade_core::Error::IncompatibleVault(detail) => trade_core::Error::Deserialize(detail),
            other => other,
        })
        .map_err(|err| format!("{}: {}", err.code(), err))?;

    let mut guard = require_vault(&state).map_err(err_to_string)?;
    let vault = guard.as_mut().unwrap();
    VaultFile::backup(&vault.path).map_err(err_to_string)?;
    vault.replace_doc(decoded).map_err(err_to_string)?;

    VaultFile::export_json(&vault.doc).map_err(|err| format!("{}: {}", err.code(), err))
}

/// JSON Schema of the vault document, for the raw JSON editor.
///
/// The schema is derived from `VaultDoc` in core, so the editor never carries
/// its own copy of the document shape.
#[tauri::command]
pub async fn vault_schema() -> Result<serde_json::Value, String> {
    Ok(trade_core::model::vault::json_schema())
}

/// Write the current document to a scratch file and hand it to `$EDITOR`.
///
/// Returns the scratch path so the UI can offer "reload from editor" after the
/// user is done. The editor is spawned detached and **not** waited on — the
/// command returns immediately so the window stays responsive.
#[tauri::command]
pub async fn dev_open_vault_json_in_editor(state: State<'_, AppState>) -> Result<String, String> {
    let json = vault_json(&state)?;
    let path = editor_scratch_path();
    std::fs::write(&path, json).map_err(|err| format!("IO_ERROR: {err}"))?;

    let editor = editor_command();
    // `$EDITOR` may carry flags (`code --wait`, `emacsclient -nw`), so the
    // command word is taken as-is and the path appended as a separate arg.
    let mut parts = editor.split_whitespace();
    let Some(program) = parts.next() else {
        return Err("IO_ERROR: no editor command".into());
    };
    Command::new(program)
        .args(parts)
        .arg(&path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| format!("IO_ERROR: cannot launch {program}: {err}"))?;

    Ok(path.display().to_string())
}

/// Read the scratch file back, so edits made in the external editor can be
/// pulled into the JSON dialog.
///
/// Takes no path on purpose: the frontend may only ask for the one file this
/// command set up, never for an arbitrary location on disk.
#[tauri::command]
pub async fn dev_reload_vault_json_from_editor() -> Result<String, String> {
    let path = editor_scratch_path();
    std::fs::read_to_string(&path).map_err(|err| format!("IO_ERROR: {err}"))
}
