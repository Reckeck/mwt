//! Files and links of operations and goals (Full-New-Attach 1–7).
//!
//! Pure part of the area: the metadata decisions over a document, no clock,
//! no randomness and no file system (ids and `now` come in as arguments). The
//! file work (seal, write, read, wipe) is in `usecases::household::attachments`.

use std::path::{Path, PathBuf};

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use mwt_base::error::{Error, Result};
use mwt_model::VaultDoc;
use mwt_model::{Attachment, AttachmentKind, MAX_ATTACHMENT_BYTES};
use mwt_model::{EntityKind, EntityRef};

/// The length of a file key. It equals `mwt_crypto::KEY_LEN`; a test of the facade pins the two together.
pub const KEY_LEN: usize = 32;

/// A file being attached: what the picker or the drop handed over.
pub struct NewFile<'a> {
    pub name: &'a str,
    pub mime: Option<&'a str>,
    pub bytes: &'a [u8],
}

/// What attaching a file decided: the record already there, or a new one
/// whose sealed bytes the scenario still has to write.
pub enum FilePlan {
    Existing(Attachment),
    New(Attachment),
}

/// Where the sealed files live: `vault.wallet` → `vault.wallet.files/`.
pub fn attachments_dir(vault_path: &Path) -> PathBuf {
    let mut name = vault_path.to_path_buf().into_os_string();
    name.push(".files");
    PathBuf::from(name)
}

/// The files and links of one entity (or of all), oldest first.
pub fn list_attachments(doc: &VaultDoc, owner: Option<&EntityRef>) -> Vec<Attachment> {
    doc.attachments
        .iter()
        .filter(|item| item.removed_at.is_none())
        .filter(|item| owner.is_none_or(|owner| &item.owner == owner))
        .cloned()
        .collect()
}

/// Ids of the operations that have at least one file or link: the paperclip
/// in the list and the "With attachments" filter.
pub fn transactions_with_attachments(doc: &VaultDoc) -> Vec<String> {
    let mut ids: Vec<String> = list_attachments(doc, None)
        .into_iter()
        .filter(|item| item.owner.kind == EntityKind::Tx)
        .map(|item| item.owner.id)
        .collect();
    ids.sort();
    ids.dedup();
    ids
}

/// The record of a new link; `id` is the fresh attachment id.
pub fn plan_link(
    doc: &VaultDoc,
    owner: EntityRef,
    url: &str,
    title: &str,
    id: String,
    now: &str,
) -> Result<Attachment> {
    check_owner(doc, &owner)?;
    let url = normalize_url(url)?;
    let title = match title.trim() {
        "" => url.clone(),
        title => title.to_string(),
    };
    Ok(Attachment {
        id,
        owner,
        kind: AttachmentKind::Link,
        title,
        url: Some(url),
        mime: None,
        size: 0,
        sha256: None,
        created_at: now.to_string(),
        removed_at: None,
    })
}

/// Check a file and decide its record; `id` is the fresh attachment id. The
/// same file added to the same entity again gives back the one already there.
pub fn plan_file(
    doc: &VaultDoc,
    owner: EntityRef,
    file: &NewFile<'_>,
    id: String,
    now: &str,
) -> Result<FilePlan> {
    check_owner(doc, &owner)?;
    let name = file.name.trim();
    if name.is_empty() {
        return Err(Error::InvalidInput("a file needs a name".into()));
    }
    if file.bytes.is_empty() {
        return Err(Error::InvalidInput("the file is empty".into()));
    }
    if file.bytes.len() > MAX_ATTACHMENT_BYTES {
        return Err(Error::TooLarge(format!("{name}: more than 25 MB")));
    }
    let sha = hex(&Sha256::digest(file.bytes));
    if let Some(same) = doc.attachments.iter().find(|item| {
        item.removed_at.is_none() && item.owner == owner && item.sha256.as_deref() == Some(&sha)
    }) {
        return Ok(FilePlan::Existing(same.clone()));
    }
    Ok(FilePlan::New(Attachment {
        id,
        owner,
        kind: AttachmentKind::File,
        title: name.to_string(),
        url: None,
        mime: file
            .mime
            .map(str::trim)
            .filter(|mime| !mime.is_empty())
            .map(str::to_string),
        size: file.bytes.len() as u64,
        sha256: Some(sha),
        created_at: now.to_string(),
        removed_at: None,
    }))
}

/// Hide the attachment (its file stays until the vault is opened next).
pub fn mark_removed(doc: &mut VaultDoc, id: &str, now: &str) -> Result<Attachment> {
    let item = find_attachment_mut(doc, id)?;
    item.removed_at = Some(now.to_string());
    Ok(item.clone())
}

/// The name shown in the list; a file keeps its bytes, a link its address.
pub fn mark_renamed(doc: &mut VaultDoc, id: &str, title: &str) -> Result<Attachment> {
    let title = title.trim();
    if title.is_empty() {
        return Err(Error::InvalidInput("a name is required".into()));
    }
    let item = find_attachment_mut(doc, id)?;
    item.title = title.to_string();
    Ok(item.clone())
}

/// Bring a removed attachment back.
pub fn mark_restored(doc: &mut VaultDoc, id: &str) -> Result<Attachment> {
    let item = doc
        .attachments
        .iter_mut()
        .find(|item| item.id == id)
        .ok_or_else(|| Error::Missing(format!("attachment {id}")))?;
    item.removed_at = None;
    Ok(item.clone())
}

/// Only operations and goals that exist take files.
pub fn check_owner(doc: &VaultDoc, owner: &EntityRef) -> Result<()> {
    if !matches!(owner.kind, EntityKind::Tx | EntityKind::Goal) {
        return Err(Error::InvalidInput(
            "files are kept only with operations and goals".into(),
        ));
    }
    if !owner_exists(doc, owner) {
        return Err(Error::Missing(format!("{:?} {}", owner.kind, owner.id)));
    }
    Ok(())
}

pub fn owner_exists(doc: &VaultDoc, owner: &EntityRef) -> bool {
    match owner.kind {
        EntityKind::Tx => doc.transactions.iter().any(|tx| tx.meta().id == owner.id),
        EntityKind::Goal => doc.goals.iter().any(|goal| goal.id == owner.id),
        _ => false,
    }
}

pub fn find_attachment<'doc>(doc: &'doc VaultDoc, id: &str) -> Result<&'doc Attachment> {
    doc.attachments
        .iter()
        .find(|item| item.id == id && item.removed_at.is_none())
        .ok_or_else(|| Error::Missing(format!("attachment {id}")))
}

fn find_attachment_mut<'doc>(doc: &'doc mut VaultDoc, id: &str) -> Result<&'doc mut Attachment> {
    doc.attachments
        .iter_mut()
        .find(|item| item.id == id && item.removed_at.is_none())
        .ok_or_else(|| Error::Missing(format!("attachment {id}")))
}

/// The key of a file, decoded from the document.
pub fn attachment_key(doc: &VaultDoc, id: &str) -> Result<Zeroizing<[u8; KEY_LEN]>> {
    let raw = doc
        .attachment_keys
        .iter()
        .find(|key| key.id == id)
        .ok_or_else(|| Error::Crypto("the file's key is missing".into()))?;
    let bytes = Zeroizing::new(
        BASE64
            .decode(&raw.key)
            .map_err(|_| Error::Crypto("broken file key".into()))?,
    );
    let mut key = Zeroizing::new([0u8; KEY_LEN]);
    if bytes.len() != KEY_LEN {
        return Err(Error::Crypto("broken file key".into()));
    }
    key.copy_from_slice(&bytes);
    Ok(key)
}

/// The base64 form of a fresh file key, as the document keeps it.
pub fn encode_key(key: &[u8; KEY_LEN]) -> String {
    BASE64.encode(key)
}

/// `example.com/x` → `https://example.com/x`; only web and mail addresses.
fn normalize_url(raw: &str) -> Result<String> {
    let url = raw.trim();
    if url.is_empty() || url.contains(char::is_whitespace) {
        return Err(Error::InvalidInput("not an address".into()));
    }
    let lower = url.to_lowercase();
    if lower.starts_with("https://") || lower.starts_with("http://") || lower.starts_with("mailto:")
    {
        return Ok(url.to_string());
    }
    if lower.contains("://") || !url.contains('.') {
        return Err(Error::InvalidInput("not a web address".into()));
    }
    Ok(format!("https://{url}"))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
