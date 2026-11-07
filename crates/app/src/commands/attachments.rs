use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use tauri::{AppHandle, Manager, State};

use trade_core::ipc_contract::{
    AddAttachmentFileInput, AddAttachmentLinkInput, AttachmentIdInput, ListAttachmentsInput,
    RenameAttachmentInput,
};
use trade_core::model::attachment::{Attachment, AttachmentKind};
use trade_core::service::NewFile;

use crate::commands::{AppState, err_to_string, require_vault};

#[tauri::command]
pub async fn list_attachments(
    input: ListAttachmentsInput,
    state: State<'_, AppState>,
) -> Result<Vec<Attachment>, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    Ok(guard
        .as_ref()
        .unwrap()
        .list_attachments(input.owner.as_ref()))
}

#[tauri::command]
pub async fn add_attachment_link(
    input: AddAttachmentLinkInput,
    state: State<'_, AppState>,
) -> Result<Attachment, String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .add_attachment_link(input.owner, &input.url, &input.title)
        .map_err(err_to_string)
}

#[tauri::command]
pub async fn add_attachment_file(
    input: AddAttachmentFileInput,
    state: State<'_, AppState>,
) -> Result<Attachment, String> {
    let bytes = BASE64
        .decode(input.data_base64.as_bytes())
        .map_err(|_| "the file did not arrive whole".to_string())?;
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .add_attachment_file(
            input.owner,
            NewFile {
                name: &input.name,
                mime: input.mime.as_deref(),
                bytes: &bytes,
            },
        )
        .map_err(err_to_string)
}

/// A link opens in the browser; a file is opened into the app's cache and
/// handed to the system viewer. The previous opened copies are cleared first,
/// so plain bytes do not pile up next to the sealed ones.
#[tauri::command]
pub async fn open_attachment(
    app: AppHandle,
    input: AttachmentIdInput,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let (item, bytes) = {
        let guard = require_vault(&state).map_err(err_to_string)?;
        let vault = guard.as_ref().unwrap();
        let item = vault
            .list_attachments(None)
            .into_iter()
            .find(|item| item.id == input.id)
            .ok_or_else(|| format!("attachment {} is gone", input.id))?;
        if item.kind == AttachmentKind::Link {
            (item, Vec::new())
        } else {
            vault.read_attachment(&input.id).map_err(err_to_string)?
        }
    };
    if let Some(url) = item.url.as_deref() {
        return tauri_plugin_opener::open_url(url, None::<&str>)
            .map_err(|_| format!("failed to open {url}"));
    }
    let cache = opened_dir(&app).ok_or("no cache directory")?;
    let _ = std::fs::remove_dir_all(&cache);
    let dir = cache.join(&item.id);
    std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    let path = dir.join(safe_name(&item.title));
    std::fs::write(&path, bytes).map_err(|err| err.to_string())?;
    tauri_plugin_opener::open_path(&path, None::<&str>)
        .map_err(|_| format!("failed to open {}", item.title))
}

/// Where decrypted attachments are put to be opened by another program.
fn opened_dir(app: &AppHandle) -> Option<std::path::PathBuf> {
    Some(app.path().app_cache_dir().ok()?.join("opened"))
}

/// Delete every decrypted attachment left on disk. Called when the vault
/// locks, so a file that was opened never outlives the unlocked session.
pub fn wipe_opened(app: &AppHandle) {
    if let Some(dir) = opened_dir(app) {
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[tauri::command]
pub async fn remove_attachment(
    input: AttachmentIdInput,
    state: State<'_, AppState>,
) -> Result<Attachment, String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .remove_attachment(&input.id)
        .map_err(err_to_string)
}

#[tauri::command]
pub async fn restore_attachment(
    input: AttachmentIdInput,
    state: State<'_, AppState>,
) -> Result<Attachment, String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .restore_attachment(&input.id)
        .map_err(err_to_string)
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
) -> Result<Attachment, String> {
    let mut guard = require_vault(&state).map_err(err_to_string)?;
    guard
        .as_mut()
        .unwrap()
        .rename_attachment(&input.id, &input.title)
        .map_err(err_to_string)
}

/// "Save as…": the save dialog, then the opened bytes where it points.
/// `None` when the dialog was closed; otherwise what to show as «saved to».
#[tauri::command]
pub async fn save_attachment_as(
    app: AppHandle,
    input: AttachmentIdInput,
    state: State<'_, AppState>,
) -> Result<Option<String>, String> {
    let (item, bytes) = {
        let guard = require_vault(&state).map_err(err_to_string)?;
        guard
            .as_ref()
            .unwrap()
            .read_attachment(&input.id)
            .map_err(err_to_string)?
    };
    let name = safe_name(&item.title);
    let extension = name.rsplit_once('.').map(|(_, ext)| ext.to_string());
    let extensions: Vec<&str> = extension.iter().map(String::as_str).collect();
    crate::platform_fs::save_bytes_dialog(&app, &name, &name, &extensions, &bytes)
        .map_err(err_to_string)
}
