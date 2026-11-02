//! Which vault keeps a copy in which external document:
//! `<app config>/vault_links.json`, keyed by the local vault path.
//!
//! Only addresses and hashes live here — no secrets: the copy itself is the
//! sealed vault file.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use trade_core::error::{Error, Result};
use trade_core::ipc_contract::{ConflictSide, VaultLinkStatus};
use trade_core::service::{ExternalMerge, UnlockedVault};
use trade_core::storage::VaultFile;
use trade_core::vault_io::mirror::{
    PushOutcome, SyncAction, compare_and_push, content_hash, reconcile,
};
use trade_core::vault_io::{VaultLocation, import::validate_vault_bytes};

use super::{read_location, release_link_target, shown, write_location};

/// How the external copy is named in the sync history.
const MERGE_PEER_NAME: &str = "Внешняя копия";

const LINKS_FILE: &str = "vault_links.json";

/// The mirror thread and the commands both update the file.
static FILE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct VaultLink {
    /// Path or document URI (`VaultLocation` as a string).
    pub target: String,
    /// Hash of the bytes both sides held after the last push or pull.
    #[serde(default)]
    pub last_synced_hash: Option<String>,
    #[serde(default)]
    pub last_synced_at: Option<String>,
    #[serde(default)]
    pub last_error: Option<String>,
    /// Another device saved the copy since the last sync: pushing stopped (it
    /// would overwrite that), and the next open merges the copy in.
    #[serde(default)]
    pub remote_changed: bool,
}

/// Code of the link error while `remote_changed` holds; the message is
/// `"CODE: text"` like the errors that cross IPC, so the UI can translate it.
pub const REMOTE_CHANGED_CODE: &str = "VAULT_REMOTE_CHANGED";

/// The link error while `remote_changed` holds.
pub fn remote_changed_message() -> String {
    format!(
        "{REMOTE_CHANGED_CODE}: the external copy was changed on another device; \
         saving to it is paused and it is merged in the next time the vault is opened"
    )
}

impl VaultLink {
    pub fn location(&self) -> VaultLocation {
        VaultLocation::parse(&self.target)
    }

    pub fn status(&self) -> VaultLinkStatus {
        VaultLinkStatus {
            linked: true,
            display_name: Some(shown(&self.location())),
            last_synced_at: self.last_synced_at.clone(),
            last_error: self.last_error.clone(),
        }
    }
}

type Links = BTreeMap<String, VaultLink>;

fn links_file(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|dir| dir.join(LINKS_FILE))
}

fn read_links(file: &Path) -> Links {
    std::fs::read_to_string(file)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

fn key(vault: &Path) -> String {
    vault.to_string_lossy().into_owned()
}

/// Read-modify-write the links file under one lock.
fn update<T>(app: &AppHandle, change: impl FnOnce(&mut Links) -> T) -> T {
    let _guard = FILE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(file) = links_file(app) else {
        return change(&mut Links::new());
    };
    let mut links = read_links(&file);
    let before = links.clone();
    let out = change(&mut links);
    if links != before
        && let Ok(json) = serde_json::to_string_pretty(&links)
    {
        let _ = trade_core::crypto::atomic_write(&file, json.as_bytes());
    }
    out
}

pub fn get(app: &AppHandle, vault: &Path) -> Option<VaultLink> {
    update(app, |links| links.get(&key(vault)).cloned())
}

pub fn is_linked(app: &AppHandle, vault: &str) -> bool {
    update(app, |links| links.contains_key(vault))
}

pub fn set(app: &AppHandle, vault: &Path, link: VaultLink) {
    update(app, |links| {
        links.insert(key(vault), link);
    });
}

/// Forget the link (and give back the document's grant).
pub fn remove(app: &AppHandle, vault: &Path) {
    if let Some(link) = update(app, |links| links.remove(&key(vault))) {
        release_link_target(app, &link.location());
    }
}

/// A push or pull went through: both sides now hold `bytes`.
pub fn mark_synced(app: &AppHandle, vault: &Path, bytes: &[u8]) {
    let hash = content_hash(bytes);
    let now = chrono::Utc::now().to_rfc3339();
    update(app, |links| {
        if let Some(link) = links.get_mut(&key(vault)) {
            link.last_synced_hash = Some(hash);
            link.last_synced_at = Some(now);
            link.last_error = None;
            link.remote_changed = false;
        }
    });
}

/// The copy holds bytes this device has not seen: stop pushing over them.
pub fn mark_remote_changed(app: &AppHandle, vault: &Path) {
    update(app, |links| {
        if let Some(link) = links.get_mut(&key(vault)) {
            link.remote_changed = true;
            link.last_error = Some(remote_changed_message());
        }
    });
}

/// The vault now includes the copy's `bytes` (merged or taken): they are the
/// base the next push compares against.
fn note_remote_seen(app: &AppHandle, vault: &Path, bytes: &[u8]) {
    let hash = content_hash(bytes);
    update(app, |links| {
        if let Some(link) = links.get_mut(&key(vault)) {
            link.last_synced_hash = Some(hash);
            link.remote_changed = false;
        }
    });
}

pub fn mark_failed(app: &AppHandle, vault: &Path, error: &str) {
    update(app, |links| {
        if let Some(link) = links.get_mut(&key(vault)) {
            link.last_error = Some(error.to_string());
        }
    });
}

/// What opening a linked vault still has to do with its external copy.
#[derive(Debug, PartialEq, Eq)]
pub enum OpenSync {
    /// Nothing: in sync, the local side was pushed, or the copy is unreadable.
    Ready,
    /// Only the copy changed: take its data (the local file is the last sync).
    Pull(Vec<u8>),
    /// Both changed: merge the copy's records into the opened vault.
    Merge(Vec<u8>),
}

/// Compare a linked vault with its external copy before it is opened.
///
/// An unreadable copy (grant lost, drive offline) does not block the vault:
/// the error is kept for the status line and the local file opens as it is.
/// A pull or merge needs the opened vault (the password), so it is returned
/// for [`take_in`] instead of being done here.
pub fn sync_before_open(app: &AppHandle, vault: &Path) -> Result<OpenSync> {
    let Some(link) = get(app, vault) else {
        return Ok(OpenSync::Ready);
    };
    let remote = match read_location(app, &link.location()) {
        Ok(bytes) => Some(bytes),
        Err(Error::NotFound(_)) => None,
        Err(err) => {
            mark_failed(app, vault, &err.to_string());
            return Ok(OpenSync::Ready);
        }
    };
    let local = std::fs::read(vault).ok();
    Ok(
        match reconcile(
            local.as_deref(),
            remote.as_deref(),
            link.last_synced_hash.as_deref(),
        ) {
            SyncAction::InSync => {
                if let Some(bytes) = &local {
                    mark_synced(app, vault, bytes);
                }
                OpenSync::Ready
            }
            SyncAction::PushLocal => {
                // The copy is back at the last synced bytes: pushing may resume.
                let link = VaultLink {
                    remote_changed: false,
                    ..link
                };
                push(app, vault, &link, local.as_deref().unwrap_or_default());
                OpenSync::Ready
            }
            SyncAction::PullRemote => OpenSync::Pull(remote.unwrap_or_default()),
            SyncAction::Conflict => OpenSync::Merge(remote.unwrap_or_default()),
        },
    )
}

/// What [`take_in`] left to the caller.
#[derive(Debug, PartialEq, Eq)]
pub enum TakeIn {
    Done,
    /// The file was replaced under the opened vault: open it again.
    Reopen,
}

/// Finish [`sync_before_open`] on the opened vault. The copy is read with the
/// same password; a copy that does not open with it or is another vault
/// falls back to the old behaviour (a pull replaces the file, a conflict is
/// reported for the user to resolve). `<vault>.bak` keeps the file as it was.
/// Afterwards the result goes back to the copy.
pub fn take_in(
    app: &AppHandle,
    vault: &mut UnlockedVault,
    outcome: OpenSync,
    password: &str,
) -> Result<TakeIn> {
    let path = vault.path.clone();
    let (remote, merging) = match outcome {
        OpenSync::Ready => return Ok(TakeIn::Done),
        OpenSync::Pull(bytes) => (bytes, false),
        OpenSync::Merge(bytes) => (bytes, true),
    };
    VaultFile::backup(&path)?;
    let taken = if merging {
        vault.merge_external_copy(&remote, password, MERGE_PEER_NAME)?
    } else {
        vault.take_external_copy(&remote, password)?
    };
    if taken == ExternalMerge::Unmergeable {
        if merging {
            let target = get(app, &path).map_or_else(String::new, |link| shown(&link.location()));
            return Err(Error::SyncConflict(target));
        }
        pull(app, &path, &remote)?;
        return Ok(TakeIn::Reopen);
    }
    // The copy's bytes are now part of this vault: the next push compares
    // against them.
    note_remote_seen(app, &path, &remote);
    if let Some(link) = get(app, &path) {
        push(app, &path, &link, &std::fs::read(&path)?);
    }
    Ok(TakeIn::Done)
}

/// After `SYNC_CONFLICT`: make both sides `keep`.
pub fn resolve_conflict(app: &AppHandle, vault: &Path, keep: ConflictSide) -> Result<()> {
    let link = get(app, vault).ok_or_else(|| Error::NotFound(key(vault)))?;
    match keep {
        ConflictSide::Local => {
            let local = std::fs::read(vault)?;
            write_location(app, &link.location(), &local)?;
            mark_synced(app, vault, &local);
            Ok(())
        }
        ConflictSide::Remote => {
            let remote = read_location(app, &link.location())?;
            pull(app, vault, &remote)
        }
    }
}

/// Write the local vault to its copy, remembering the outcome. Compare and
/// swap: a copy another device saved since the last sync is not overwritten
/// (it is merged in when the vault is opened), and pushing stays stopped until
/// then.
pub fn push(app: &AppHandle, vault: &Path, link: &VaultLink, bytes: &[u8]) {
    if link.remote_changed {
        return;
    }
    let location = link.location();
    let outcome = compare_and_push(
        || match read_location(app, &location) {
            Ok(remote) => Ok(Some(remote)),
            Err(Error::NotFound(_)) => Ok(None),
            Err(err) => Err(err),
        },
        |data| write_location(app, &location, data),
        link.last_synced_hash.as_deref(),
        bytes,
    );
    match outcome {
        Ok(PushOutcome::Written | PushOutcome::AlreadyThere) => mark_synced(app, vault, bytes),
        Ok(PushOutcome::RemoteChanged) => mark_remote_changed(app, vault),
        Err(err) => mark_failed(app, vault, &err.to_string()),
    }
}

/// Write the vault to a copy the user just picked, replacing whatever is
/// there (they chose that): no comparison.
pub fn push_replacing(app: &AppHandle, vault: &Path, link: &VaultLink, bytes: &[u8]) {
    match write_location(app, &link.location(), bytes) {
        Ok(()) => mark_synced(app, vault, bytes),
        Err(err) => mark_failed(app, vault, &err.to_string()),
    }
}

/// Replace the local vault with the external copy; the local file stays as
/// `<vault>.bak`.
fn pull(app: &AppHandle, vault: &Path, remote: &[u8]) -> Result<()> {
    validate_vault_bytes(remote)?;
    VaultFile::backup(vault)?;
    trade_core::crypto::atomic_write(vault, remote)?;
    mark_synced(app, vault, remote);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_remote_changed_error_carries_its_code() {
        assert!(remote_changed_message().starts_with("VAULT_REMOTE_CHANGED: "));
    }

    #[test]
    fn links_round_trip_and_tolerate_old_entries() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(LINKS_FILE);
        std::fs::write(&file, r#"{"/v.wallet":{"target":"content://x/y"}}"#).unwrap();
        let links = read_links(&file);
        let link = &links["/v.wallet"];
        assert_eq!(link.location(), VaultLocation::Uri("content://x/y".into()));
        assert_eq!(link.last_synced_hash, None);
        assert!(!link.remote_changed);
        assert!(link.status().linked);
        assert_eq!(read_links(&dir.path().join("missing.json")), Links::new());
    }
}
