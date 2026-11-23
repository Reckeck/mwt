//! Scenarios of the attachments area: each decides in `household::attachments`,
//! does its file work here (open, write, read) and ends in one `vault.transact`.

use std::path::PathBuf;

use crate::crypto::{open_blob, random_blob_key, seal_blob};
use crate::error::{Error, Result};
use crate::household::attachments::{
    FilePlan, NewFile, attachment_key, attachments_dir as dir_of, check_owner, encode_key,
    find_attachment, mark_removed, mark_renamed, mark_restored, plan_file, plan_link,
};
use crate::model::attachment::{Attachment, AttachmentKey, AttachmentKind};
use crate::model::entity_ref::EntityRef;
use crate::model::vault::now_iso;
use crate::service::Session;

/// Where the sealed files live: `vault.wallet` → `vault.wallet.files/`.
pub fn attachments_dir(vault: &Session) -> PathBuf {
    dir_of(&vault.path)
}

pub fn add_attachment_link(
    vault: &mut Session,
    owner: EntityRef,
    url: &str,
    title: &str,
) -> Result<Attachment> {
    let id = uuid::Uuid::new_v4().to_string();
    let item = plan_link(&vault.doc, owner, url, title, id, &now_iso())?;
    vault.transact(|tx| {
        tx.doc_mut().attachments.push(item.clone());
        tx.touch();
        Ok(item)
    })
}

/// Seal the file next to the vault and keep its record. The same file
/// added to the same entity again gives back the one already there.
pub fn add_attachment_file(
    vault: &mut Session,
    owner: EntityRef,
    file: NewFile<'_>,
) -> Result<Attachment> {
    check_owner(&vault.doc, &owner)?;
    if vault.sandbox {
        return Err(Error::InvalidInput(
            "files are not kept in the sandbox".into(),
        ));
    }
    let id = uuid::Uuid::new_v4().to_string();
    let item = match plan_file(&vault.doc, owner, &file, id, &now_iso())? {
        FilePlan::Existing(same) => return Ok(same),
        FilePlan::New(item) => item,
    };

    let key = random_blob_key();
    let sealed = seal_blob(&key, item.id.as_bytes(), file.bytes)?;
    let dir = attachments_dir(vault);
    std::fs::create_dir_all(&dir).map_err(io)?;
    let part = dir.join(format!("{}.part", item.id));
    std::fs::write(&part, sealed).map_err(io)?;
    std::fs::rename(&part, dir.join(&item.id)).map_err(io)?;

    let record = AttachmentKey {
        id: item.id.clone(),
        key: encode_key(&key),
    };
    vault.transact(|tx| {
        let doc = tx.doc_mut();
        doc.attachments.push(item.clone());
        doc.attachment_keys.push(record);
        tx.touch();
        Ok(item)
    })
}

/// The file's bytes, opened; a link has none.
pub fn read_attachment(vault: &Session, id: &str) -> Result<(Attachment, Vec<u8>)> {
    let item = find_attachment(&vault.doc, id)?.clone();
    if item.kind != AttachmentKind::File {
        return Err(Error::InvalidInput("a link has no file".into()));
    }
    let key = attachment_key(&vault.doc, id)?;
    let path = attachments_dir(vault).join(id);
    let sealed = std::fs::read(&path).map_err(|err| match err.kind() {
        std::io::ErrorKind::NotFound => {
            Error::NotFound(format!("{}: not on this device", item.title))
        }
        _ => io(err),
    })?;
    let bytes = open_blob(&key, id.as_bytes(), &sealed)?;
    Ok((item, bytes))
}

/// Hide the attachment; [`restore_attachment`] brings it back until the vault
/// is opened next, when the cleanup deletes the file.
pub fn remove_attachment(vault: &mut Session, id: &str) -> Result<Attachment> {
    vault.transact(|tx| {
        let out = mark_removed(tx.doc_mut(), id, &now_iso())?;
        tx.touch();
        Ok(out)
    })
}

pub fn rename_attachment(vault: &mut Session, id: &str, title: &str) -> Result<Attachment> {
    vault.transact(|tx| {
        let out = mark_renamed(tx.doc_mut(), id, title)?;
        tx.touch();
        Ok(out)
    })
}

pub fn restore_attachment(vault: &mut Session, id: &str) -> Result<Attachment> {
    vault.transact(|tx| {
        let out = mark_restored(tx.doc_mut(), id)?;
        tx.touch();
        Ok(out)
    })
}

fn io(err: std::io::Error) -> Error {
    Error::Io(err.to_string())
}
