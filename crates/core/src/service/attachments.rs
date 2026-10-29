//! Files and links of operations and goals (Full-New-Attach 1–7).

use std::path::PathBuf;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use super::UnlockedVault;
use crate::crypto::{KEY_LEN, open_blob, random_blob_key, seal_blob};
use crate::error::{Error, Result};
use crate::model::attachment::{Attachment, AttachmentKey, AttachmentKind, MAX_ATTACHMENT_BYTES};
use crate::model::entity_ref::{EntityKind, EntityRef};
use crate::model::vault::now_iso;

/// A file being attached: what the picker or the drop handed over.
pub struct NewFile<'a> {
    pub name: &'a str,
    pub mime: Option<&'a str>,
    pub bytes: &'a [u8],
}

impl UnlockedVault {
    /// Where the sealed files live: `vault.wallet` → `vault.wallet.files/`.
    pub fn attachments_dir(&self) -> PathBuf {
        let mut name = self.path.clone().into_os_string();
        name.push(".files");
        PathBuf::from(name)
    }

    /// The files and links of one entity (or of all), oldest first.
    pub fn list_attachments(&self, owner: Option<&EntityRef>) -> Vec<Attachment> {
        self.doc
            .attachments
            .iter()
            .filter(|item| item.removed_at.is_none())
            .filter(|item| owner.is_none_or(|owner| &item.owner == owner))
            .cloned()
            .collect()
    }

    /// Ids of the operations that have at least one file or link: the paperclip
    /// in the list and the "With attachments" filter.
    pub fn transactions_with_attachments(&self) -> Vec<String> {
        let mut ids: Vec<String> = self
            .list_attachments(None)
            .into_iter()
            .filter(|item| item.owner.kind == EntityKind::Tx)
            .map(|item| item.owner.id)
            .collect();
        ids.sort();
        ids.dedup();
        ids
    }

    pub fn add_attachment_link(
        &mut self,
        owner: EntityRef,
        url: &str,
        title: &str,
    ) -> Result<Attachment> {
        self.check_owner(&owner)?;
        let url = normalize_url(url)?;
        let title = match title.trim() {
            "" => url.clone(),
            title => title.to_string(),
        };
        let item = Attachment {
            id: uuid::Uuid::new_v4().to_string(),
            owner,
            kind: AttachmentKind::Link,
            title,
            url: Some(url),
            mime: None,
            size: 0,
            sha256: None,
            created_at: now_iso(),
            removed_at: None,
        };
        self.doc.attachments.push(item.clone());
        self.persist()?;
        Ok(item)
    }

    /// Seal the file next to the vault and keep its record. The same file
    /// added to the same entity again gives back the one already there.
    pub fn add_attachment_file(
        &mut self,
        owner: EntityRef,
        file: NewFile<'_>,
    ) -> Result<Attachment> {
        self.check_owner(&owner)?;
        if self.sandbox {
            return Err(Error::InvalidInput(
                "files are not kept in the sandbox".into(),
            ));
        }
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
        if let Some(same) = self.doc.attachments.iter().find(|item| {
            item.removed_at.is_none() && item.owner == owner && item.sha256.as_deref() == Some(&sha)
        }) {
            return Ok(same.clone());
        }

        let id = uuid::Uuid::new_v4().to_string();
        let key = random_blob_key();
        let sealed = seal_blob(&key, id.as_bytes(), file.bytes)?;
        let dir = self.attachments_dir();
        std::fs::create_dir_all(&dir).map_err(io)?;
        let part = dir.join(format!("{id}.part"));
        std::fs::write(&part, sealed).map_err(io)?;
        std::fs::rename(&part, dir.join(&id)).map_err(io)?;

        let item = Attachment {
            id: id.clone(),
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
            created_at: now_iso(),
            removed_at: None,
        };
        self.doc.attachments.push(item.clone());
        self.doc.attachment_keys.push(AttachmentKey {
            id,
            key: BASE64.encode(*key),
        });
        self.persist()?;
        Ok(item)
    }

    /// The file's bytes, opened; a link has none.
    pub fn read_attachment(&self, id: &str) -> Result<(Attachment, Vec<u8>)> {
        let item = self.find_attachment(id)?.clone();
        if item.kind != AttachmentKind::File {
            return Err(Error::InvalidInput("a link has no file".into()));
        }
        let key = self.attachment_key(id)?;
        let path = self.attachments_dir().join(id);
        let sealed = std::fs::read(&path).map_err(|err| match err.kind() {
            std::io::ErrorKind::NotFound => {
                Error::NotFound(format!("{}: not on this device", item.title))
            }
            _ => io(err),
        })?;
        let bytes = open_blob(&key, id.as_bytes(), &sealed)?;
        Ok((item, bytes))
    }

    /// Hide the attachment; [`Self::restore_attachment`] brings it back until the
    /// vault is opened next, when [`Self::prune_attachments`] deletes the file.
    pub fn remove_attachment(&mut self, id: &str) -> Result<Attachment> {
        let item = self.find_attachment_mut(id)?;
        item.removed_at = Some(now_iso());
        let out = item.clone();
        self.persist()?;
        Ok(out)
    }

    /// The name shown in the list; a file keeps its bytes, a link its address.
    pub fn rename_attachment(&mut self, id: &str, title: &str) -> Result<Attachment> {
        let title = title.trim();
        if title.is_empty() {
            return Err(Error::InvalidInput("a name is required".into()));
        }
        let item = self.find_attachment_mut(id)?;
        item.title = title.to_string();
        let out = item.clone();
        self.persist()?;
        Ok(out)
    }

    pub fn restore_attachment(&mut self, id: &str) -> Result<Attachment> {
        let item = self
            .doc
            .attachments
            .iter_mut()
            .find(|item| item.id == id)
            .ok_or_else(|| Error::Missing(format!("attachment {id}")))?;
        item.removed_at = None;
        let out = item.clone();
        self.persist()?;
        Ok(out)
    }

    /// Drop what was removed and what lost its owner (a deleted operation or
    /// goal), files included. Runs when the vault is opened: no "Undo" of
    /// the last session can be waiting any more. Returns whether anything went.
    pub(super) fn prune_attachments(&mut self) -> bool {
        let gone: Vec<String> = self
            .doc
            .attachments
            .iter()
            .filter(|item| item.removed_at.is_some() || !self.owner_exists(&item.owner))
            .map(|item| item.id.clone())
            .collect();
        if gone.is_empty() {
            return false;
        }
        if !self.sandbox {
            let dir = self.attachments_dir();
            for id in &gone {
                // A file synced from another device may never have been here.
                let _ = std::fs::remove_file(dir.join(id));
            }
        }
        self.doc.attachments.retain(|item| !gone.contains(&item.id));
        self.doc
            .attachment_keys
            .retain(|key| !gone.contains(&key.id));
        true
    }

    fn check_owner(&self, owner: &EntityRef) -> Result<()> {
        if !matches!(owner.kind, EntityKind::Tx | EntityKind::Goal) {
            return Err(Error::InvalidInput(
                "files are kept only with operations and goals".into(),
            ));
        }
        if !self.owner_exists(owner) {
            return Err(Error::Missing(format!("{:?} {}", owner.kind, owner.id)));
        }
        Ok(())
    }

    fn owner_exists(&self, owner: &EntityRef) -> bool {
        match owner.kind {
            EntityKind::Tx => self
                .doc
                .transactions
                .iter()
                .any(|tx| tx.meta().id == owner.id),
            EntityKind::Goal => self.doc.goals.iter().any(|goal| goal.id == owner.id),
            _ => false,
        }
    }

    fn find_attachment(&self, id: &str) -> Result<&Attachment> {
        self.doc
            .attachments
            .iter()
            .find(|item| item.id == id && item.removed_at.is_none())
            .ok_or_else(|| Error::Missing(format!("attachment {id}")))
    }

    fn find_attachment_mut(&mut self, id: &str) -> Result<&mut Attachment> {
        self.doc
            .attachments
            .iter_mut()
            .find(|item| item.id == id && item.removed_at.is_none())
            .ok_or_else(|| Error::Missing(format!("attachment {id}")))
    }

    fn attachment_key(&self, id: &str) -> Result<Zeroizing<[u8; KEY_LEN]>> {
        let raw = self
            .doc
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

fn io(err: std::io::Error) -> Error {
    Error::Io(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::test_support::*;

    fn tx_owner(vault: &mut UnlockedVault) -> EntityRef {
        let card = store(vault, "Card", crate::model::store::StoreKind::Bank, None);
        let tx = vault
            .add_transaction(income(&card, "100", "salary"))
            .unwrap();
        EntityRef::new(EntityKind::Tx, tx.meta().id.clone())
    }

    fn receipt<'a>(bytes: &'a [u8]) -> NewFile<'a> {
        NewFile {
            name: "чек.pdf",
            mime: Some("application/pdf"),
            bytes,
        }
    }

    #[test]
    fn a_file_is_sealed_next_to_the_vault_and_opens_back() {
        let (_dir, mut vault) = setup();
        let owner = tx_owner(&mut vault);
        let item = vault
            .add_attachment_file(owner.clone(), receipt(b"%PDF-1.7 receipt"))
            .unwrap();

        let on_disk = std::fs::read(vault.attachments_dir().join(&item.id)).unwrap();
        assert!(
            !on_disk.windows(7).any(|part| part == b"receipt"),
            "the file is not sealed"
        );
        let (_, bytes) = vault.read_attachment(&item.id).unwrap();
        assert_eq!(bytes, b"%PDF-1.7 receipt");
        assert_eq!(item.size, 16);
        assert_eq!(vault.list_attachments(Some(&owner)), vec![item.clone()]);
        assert_eq!(
            vault.transactions_with_attachments(),
            vec![owner.id.clone()]
        );

        // The same file again is the same attachment.
        let again = vault
            .add_attachment_file(owner, receipt(b"%PDF-1.7 receipt"))
            .unwrap();
        assert_eq!(again.id, item.id);
    }

    #[test]
    fn a_new_password_keeps_the_files_readable() {
        let (_dir, mut vault) = setup();
        let owner = tx_owner(&mut vault);
        let item = vault.add_attachment_file(owner, receipt(b"bytes")).unwrap();
        vault.flush_pending().unwrap();
        let path = vault.path.clone();
        vault.change_password("pw", "another").unwrap();
        drop(vault);
        let reopened = UnlockedVault::open(path, "another").unwrap();
        assert_eq!(reopened.read_attachment(&item.id).unwrap().1, b"bytes");
    }

    #[test]
    fn removal_waits_for_undo_until_the_next_open() {
        let (_dir, mut vault) = setup();
        let owner = tx_owner(&mut vault);
        let item = vault
            .add_attachment_file(owner.clone(), receipt(b"bytes"))
            .unwrap();
        let file = vault.attachments_dir().join(&item.id);

        vault.remove_attachment(&item.id).unwrap();
        assert!(vault.list_attachments(Some(&owner)).is_empty());
        assert!(file.exists(), "undo still needs the file");
        vault.restore_attachment(&item.id).unwrap();
        assert_eq!(vault.list_attachments(Some(&owner)).len(), 1);

        vault.remove_attachment(&item.id).unwrap();
        vault.flush_pending().unwrap();
        let path = vault.path.clone();
        drop(vault);
        let reopened = UnlockedVault::open(path, "pw").unwrap();
        assert!(reopened.doc.attachments.is_empty());
        assert!(reopened.doc.attachment_keys.is_empty());
        assert!(!file.exists());
    }

    #[test]
    fn a_deleted_operation_takes_its_files_on_the_next_open() {
        let (_dir, mut vault) = setup();
        let owner = tx_owner(&mut vault);
        let item = vault
            .add_attachment_file(owner.clone(), receipt(b"bytes"))
            .unwrap();
        vault
            .add_attachment_link(owner.clone(), "bank.ru/check/1", "")
            .unwrap();
        let file = vault.attachments_dir().join(&item.id);
        vault.delete_transaction(&owner.id).unwrap();
        vault.flush_pending().unwrap();
        let path = vault.path.clone();
        drop(vault);
        let reopened = UnlockedVault::open(path, "pw").unwrap();
        assert!(reopened.doc.attachments.is_empty());
        assert!(!file.exists());
    }

    #[test]
    fn links_are_web_addresses_with_a_caption() {
        let (_dir, mut vault) = setup();
        let owner = tx_owner(&mut vault);
        let link = vault
            .add_attachment_link(owner.clone(), " bank.ru/check/1 ", "")
            .unwrap();
        assert_eq!(link.url.as_deref(), Some("https://bank.ru/check/1"));
        assert_eq!(link.title, "https://bank.ru/check/1");
        let named = vault
            .add_attachment_link(owner.clone(), "https://x.io", "Договор")
            .unwrap();
        assert_eq!(named.title, "Договор");
        assert!(
            vault
                .add_attachment_link(owner.clone(), "file:///etc/passwd", "")
                .is_err()
        );
        assert!(
            vault
                .add_attachment_link(owner, "not an address", "")
                .is_err()
        );
        assert!(vault.read_attachment(&link.id).is_err());
    }

    #[test]
    fn a_renamed_file_is_found_by_its_new_name() {
        let (_dir, mut vault) = setup();
        let owner = tx_owner(&mut vault);
        let item = vault
            .add_attachment_file(owner.clone(), receipt(b"bytes"))
            .unwrap();
        vault
            .rename_attachment(&item.id, "  Чек skycapital  ")
            .unwrap();
        assert!(vault.rename_attachment(&item.id, " ").is_err());
        let filter = crate::service::TxFilter {
            text: Some("skycapital".into()),
            ..Default::default()
        };
        let found = vault.list_transactions(&filter, "UTC").unwrap();
        assert_eq!(found.len(), 1);
        let only_attached = crate::service::TxFilter {
            with_attachments: true,
            ..Default::default()
        };
        assert_eq!(
            vault
                .list_transactions(&only_attached, "UTC")
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn only_operations_and_goals_that_exist_take_files() {
        let (_dir, mut vault) = setup();
        let missing = EntityRef::new(EntityKind::Tx, "nope");
        assert!(
            vault
                .add_attachment_link(missing, "https://x.io", "")
                .is_err()
        );
        let store = EntityRef::new(EntityKind::Store, vault.doc.stores[0].id.clone());
        assert!(
            vault
                .add_attachment_link(store, "https://x.io", "")
                .is_err()
        );
        let owner = tx_owner(&mut vault);
        let big = vec![0u8; MAX_ATTACHMENT_BYTES + 1];
        assert!(matches!(
            vault.add_attachment_file(owner, receipt(&big)),
            Err(Error::TooLarge(_))
        ));
    }
}
