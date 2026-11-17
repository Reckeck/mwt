use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use tauri::{AppHandle, Manager, State};

use trade_core::ipc_contract::{
    AddAttachmentFileInput, AddAttachmentLinkInput, AttachmentIdInput, ListAttachmentsInput,
    RenameAttachmentInput,
};
use trade_core::kernel::events::EventBus;
use trade_core::model::attachment::{Attachment, AttachmentKind};
use trade_core::service::NewFile;

use crate::commands::{AppState, require_vault};
use crate::ipc::IpcError;
use crate::ipc::commands::ipc_commands;
use crate::ipc::opener::UrlKind;
use crate::state::on_vault_locked;

#[tauri::command]
pub async fn list_attachments(
    input: ListAttachmentsInput,
    state: State<'_, AppState>,
) -> Result<Vec<Attachment>, IpcError> {
    let guard = require_vault(&state)?;
    Ok(guard
        .as_ref()
        .unwrap()
        .list_attachments(input.owner.as_ref()))
}

#[tauri::command]
pub async fn add_attachment_link(
    input: AddAttachmentLinkInput,
    state: State<'_, AppState>,
) -> Result<Attachment, IpcError> {
    let mut guard = require_vault(&state)?;
    Ok(guard
        .as_mut()
        .unwrap()
        .add_attachment_link(input.owner, &input.url, &input.title)?)
}

#[tauri::command]
pub async fn add_attachment_file(
    input: AddAttachmentFileInput,
    state: State<'_, AppState>,
) -> Result<Attachment, IpcError> {
    let bytes = BASE64
        .decode(input.data_base64.as_bytes())
        .map_err(|_| IpcError::from("the file did not arrive whole"))?;
    let mut guard = require_vault(&state)?;
    Ok(guard.as_mut().unwrap().add_attachment_file(
        input.owner,
        NewFile {
            name: &input.name,
            mime: input.mime.as_deref(),
            bytes: &bytes,
        },
    )?)
}

/// A link opens in the browser; a file is opened into the app's cache and
/// handed to the system viewer. The previous opened copies are cleared first,
/// so plain bytes do not pile up next to the sealed ones.
#[tauri::command]
pub async fn open_attachment(
    app: AppHandle,
    input: AttachmentIdInput,
    state: State<'_, AppState>,
) -> Result<(), IpcError> {
    let (item, bytes) = {
        let guard = require_vault(&state)?;
        let vault = guard.as_ref().unwrap();
        let item = vault
            .list_attachments(None)
            .into_iter()
            .find(|item| item.id == input.id)
            .ok_or_else(|| format!("attachment {} is gone", input.id))?;
        if item.kind == AttachmentKind::Link {
            (item, Vec::new())
        } else {
            vault.read_attachment(&input.id)?
        }
    };
    if let Some(url) = item.url.as_deref() {
        // The record may have come from a paired device or the JSON editor,
        // neither of which checks the link: only a web or mail address goes
        // to the system, and the error does not repeat the link.
        return state.opener.open_url(url, UrlKind::Attachment);
    }
    let cache = opened_dir(&app).ok_or("no cache directory")?;
    let _ = std::fs::remove_dir_all(&cache);
    let dir = cache.join(&item.id);
    std::fs::create_dir_all(&dir).map_err(|err| IpcError::from(err.to_string()))?;
    let path = dir.join(safe_name(&item.title));
    std::fs::write(&path, bytes).map_err(|err| IpcError::from(err.to_string()))?;
    state
        .opener
        .open_path(&path)
        .map_err(|_| IpcError::from(format!("failed to open {}", item.title)))
}

/// Where decrypted attachments are put to be opened by another program.
fn opened_dir(app: &AppHandle) -> Option<std::path::PathBuf> {
    Some(app.path().app_cache_dir().ok()?.join("opened"))
}

/// Wipe the decrypted attachments on every `VaultLocked`, also when the flush
/// failed and the vault stayed open (spec D1, D5).
pub fn wipe_on_lock(bus: &EventBus, app: AppHandle) {
    wipe_dir_on_lock(bus, opened_dir(&app));
}

pub(crate) fn wipe_dir_on_lock(bus: &EventBus, dir: Option<std::path::PathBuf>) {
    if let Some(dir) = dir {
        on_vault_locked(bus, move |_| wipe_dir(&dir));
    }
}

/// Delete every decrypted attachment left on disk. Called on `VaultLocked`,
/// at start and at exit, so a file that was opened never outlives the unlocked session.
pub fn wipe_opened(app: &AppHandle) {
    if let Some(dir) = opened_dir(app) {
        wipe_dir(&dir);
    }
}

/// Remove `dir` with everything in it; a missing one is fine.
fn wipe_dir(dir: &std::path::Path) {
    let _ = std::fs::remove_dir_all(dir);
}

#[cfg(test)]
mod wipe_tests {
    use super::wipe_dir;

    #[test]
    fn wipe_removes_opened_files_and_tolerates_a_missing_dir() {
        let temp = tempfile::tempdir().unwrap();
        let opened = temp.path().join("opened");
        std::fs::create_dir_all(opened.join("att-1")).unwrap();
        std::fs::write(opened.join("att-1").join("file.pdf"), b"secret").unwrap();
        wipe_dir(&opened);
        assert!(!opened.exists());
        wipe_dir(&opened);
    }
}

#[tauri::command]
pub async fn remove_attachment(
    input: AttachmentIdInput,
    state: State<'_, AppState>,
) -> Result<Attachment, IpcError> {
    let mut guard = require_vault(&state)?;
    Ok(guard.as_mut().unwrap().remove_attachment(&input.id)?)
}

#[tauri::command]
pub async fn restore_attachment(
    input: AttachmentIdInput,
    state: State<'_, AppState>,
) -> Result<Attachment, IpcError> {
    let mut guard = require_vault(&state)?;
    Ok(guard.as_mut().unwrap().restore_attachment(&input.id)?)
}

/// The file's own name without anything that could leave the folder.
fn safe_name(title: &str) -> String {
    let name: String = title
        .chars()
        .map(|ch| {
            if matches!(ch, '/' | '\\' | ':' | '\0') {
                '_'
            } else {
                ch
            }
        })
        .collect();
    match name.trim_matches('.').trim() {
        "" => "file".to_string(),
        name => name.to_string(),
    }
}

#[tauri::command]
pub async fn rename_attachment(
    input: RenameAttachmentInput,
    state: State<'_, AppState>,
) -> Result<Attachment, IpcError> {
    let mut guard = require_vault(&state)?;
    Ok(guard
        .as_mut()
        .unwrap()
        .rename_attachment(&input.id, &input.title)?)
}

/// "Save as…": the save dialog, then the opened bytes where it points.
/// `None` when the dialog was closed; otherwise what to show as «saved to».
#[tauri::command]
pub async fn save_attachment_as(
    app: AppHandle,
    input: AttachmentIdInput,
    state: State<'_, AppState>,
) -> Result<Option<String>, IpcError> {
    let (item, bytes) = {
        let guard = require_vault(&state)?;
        guard.as_ref().unwrap().read_attachment(&input.id)?
    };
    let name = safe_name(&item.title);
    let extension = name.rsplit_once('.').map(|(_, ext)| ext.to_string());
    let extensions: Vec<&str> = extension.iter().map(String::as_str).collect();
    Ok(crate::platform_fs::save_bytes_dialog(
        &app,
        &name,
        &name,
        &extensions,
        &bytes,
    )?)
}

ipc_commands!(attachments_commands, ATTACHMENTS_NAMES, commands::attachments: [list_attachments, add_attachment_link, add_attachment_file, open_attachment, remove_attachment, restore_attachment, rename_attachment, save_attachment_as]);
