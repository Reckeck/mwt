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
use trade_core::storage::VaultFile;
use trade_core::vault_io::mirror::{SyncAction, content_hash, reconcile};
use trade_core::vault_io::{VaultLocation, import::validate_vault_bytes};

use super::{read_location, release_link_target, shown, write_location};

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

/// Bring a linked vault and its external copy together before it is opened.
///
/// An unreadable copy (grant lost, drive offline) does not block the vault:
/// the error is kept for the status line and the local file opens as it is.
pub fn sync_before_open(app: &AppHandle, vault: &Path) -> Result<()> {
    let Some(link) = get(app, vault) else {
        return Ok(());
    };
    let remote = match read_location(app, &link.location()) {
        Ok(bytes) => Some(bytes),
        Err(Error::NotFound(_)) => None,
        Err(err) => {
            mark_failed(app, vault, &err.to_string());
            return Ok(());
        }
    };
    let local = std::fs::read(vault).ok();
    match reconcile(
        local.as_deref(),
        remote.as_deref(),
        link.last_synced_hash.as_deref(),
    ) {
        SyncAction::InSync => {
            if let Some(bytes) = &local {
                mark_synced(app, vault, bytes);
            }
            Ok(())
        }
        SyncAction::PushLocal => {
            push(app, vault, &link, local.as_deref().unwrap_or_default());
            Ok(())
        }
        SyncAction::PullRemote => pull(app, vault, remote.as_deref().unwrap_or_default()),
        SyncAction::Conflict => Err(Error::SyncConflict(shown(&link.location()))),
    }
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

/// Write the local vault to its copy, remembering the outcome.
pub fn push(app: &AppHandle, vault: &Path, link: &VaultLink, bytes: &[u8]) {
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
    fn links_round_trip_and_tolerate_old_entries() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(LINKS_FILE);
        std::fs::write(&file, r#"{"/v.wallet":{"target":"content://x/y"}}"#).unwrap();
        let links = read_links(&file);
        let link = &links["/v.wallet"];
        assert_eq!(link.location(), VaultLocation::Uri("content://x/y".into()));
        assert_eq!(link.last_synced_hash, None);
        assert!(link.status().linked);
        assert_eq!(read_links(&dir.path().join("missing.json")), Links::new());
    }
}
