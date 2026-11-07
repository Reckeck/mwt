//! Exchange API keys on this device (H2).
//!
//! At security level 3 the keys live only in the device secrets file
//! (`crypto::device_secrets`), never in the vault document: `doc.exchanges`
//! is an in-memory index of them (store, sub-account targets, no key and no
//! secret) that saving takes out of the document, so the vault file, its
//! mirror, backups, transfers and sync never carry a key. A key is read from
//! the file for one request and zeroized when that request drops it.
//!
//! Below level 3 a key is never stored nor used: what an old vault still
//! holds in its document stays there untouched (so raising the level later
//! moves it) and only says that a key existed.

use std::collections::BTreeMap;

use super::UnlockedVault;
use crate::crypto::SealKey;
use crate::crypto::device_secrets::{self, DeviceKey, DeviceSecrets};
use crate::crypto::header::VaultHeader;
use crate::error::{Error, Result};
use crate::ipc_contract::security::{ExchangeKeyState, SecurityLevel};
use crate::model::store::StoreKind;
use crate::model::vault::ExchangeKeys;

/// `(store_id, spot_store_id, futures_store_id)` of one key.
type Target = (String, Option<String>, Option<String>);

/// What the session knows about the keys in the secrets file, without them.
#[derive(Default)]
pub(crate) struct DeviceKeys {
    /// Per key store: its mask for the UI and whether the read-only check passed.
    meta: BTreeMap<String, KeyMeta>,
    /// The targets as the file has them: a change of the index is written
    /// to the file on the next save.
    written: Vec<Target>,
    /// The file exists but does not open (damaged): no key is used and the
    /// next stored key replaces it.
    unreadable: bool,
}

struct KeyMeta {
    hint: String,
    verified: bool,
}

/// The index entry of a key: everything but the key and the secret.
fn blank(keys: &ExchangeKeys) -> ExchangeKeys {
    ExchangeKeys {
        store_id: keys.store_id.clone(),
        api_key: String::new(),
        api_secret: String::new(),
        spot_store_id: keys.spot_store_id.clone(),
        futures_store_id: keys.futures_store_id.clone(),
    }
}

fn has_secret(keys: &ExchangeKeys) -> bool {
    !keys.api_key.is_empty() && !keys.api_secret.is_empty()
}

impl UnlockedVault {
    /// The security level of this session, from the key it was opened with.
    pub fn security_level(&self) -> SecurityLevel {
        match &self.key {
            Some(key) if key.is_plain() => SecurityLevel::Ledger,
            Some(key) if key.kdf().id == VaultHeader::KDF_ARGON2ID_HARDENED => {
                SecurityLevel::Crypto
            }
            _ => SecurityLevel::Pin,
        }
    }

    /// Keys live in the device file (level 3, a real vault).
    pub(super) fn keys_on_device(&self) -> bool {
        !self.sandbox && self.security_level() == SecurityLevel::Crypto
    }

    fn device_seal_key(&self) -> Result<std::sync::Arc<SealKey>> {
        self.key
            .clone()
            .ok_or_else(|| Error::Crypto("the vault has no key".into()))
    }

    fn key_targets(&self) -> Vec<Target> {
        self.doc
            .exchanges
            .bybit
            .iter()
            .map(|keys| {
                (
                    keys.store_id.clone(),
                    keys.spot_store_id.clone(),
                    keys.futures_store_id.clone(),
                )
            })
            .collect()
    }

    fn load_device_secrets(&self) -> Result<DeviceSecrets> {
        device_secrets::load(&self.path, &*self.device_seal_key()?)
    }

    /// The whole device file for one request (the caller drops it soon).
    pub(super) fn device_secrets_for_request(&self) -> Result<DeviceSecrets> {
        if self.device_keys.unreadable {
            return Err(Error::Secret);
        }
        self.load_device_secrets()
    }

    /// The index and the masks from the file.
    fn index_from(&mut self, secrets: &DeviceSecrets) {
        self.doc.exchanges.bybit = secrets
            .bybit
            .iter()
            .map(|entry| blank(&entry.keys))
            .collect();
        self.device_keys.meta = secrets
            .bybit
            .iter()
            .map(|entry| {
                (
                    entry.keys.store_id.clone(),
                    KeyMeta {
                        hint: entry.keys.hint(),
                        verified: entry.verified,
                    },
                )
            })
            .collect();
        self.device_keys.written = self.key_targets();
    }

    /// At level 3: keys an old document still holds move to the device file
    /// (unverified, so the next import checks them), and the index is read
    /// from the file. Runs on open and after the level is raised to 3 (H5).
    /// `true` = the document lost its keys and must be rewritten.
    pub fn adopt_device_secrets(&mut self) -> Result<bool> {
        if !self.keys_on_device() {
            return Ok(false);
        }
        let key = self.device_seal_key()?;
        let (mut secrets, unreadable) = match device_secrets::load(&self.path, &key) {
            Ok(secrets) => (secrets, false),
            Err(Error::Secret) => (DeviceSecrets::default(), true),
            Err(err) => return Err(err),
        };
        let legacy: Vec<ExchangeKeys> = self
            .doc
            .exchanges
            .bybit
            .iter()
            .filter(|keys| has_secret(keys))
            .cloned()
            .collect();
        let migrated = !legacy.is_empty();
        for keys in legacy {
            if secrets.bybit_for(&keys.store_id).is_none() {
                secrets.bybit.push(DeviceKey {
                    keys,
                    verified: false,
                });
            }
        }
        if migrated {
            device_secrets::save(&self.path, &key, &secrets)?;
        }
        self.device_keys.unreadable = unreadable && !migrated;
        self.index_from(&secrets);
        Ok(migrated)
    }

    /// Before a save at level 3: a key dropped from the index (its store
    /// deleted, merged, the key cleared) or new sub-account targets reach the
    /// file. Nothing to do when the index is as written.
    pub(super) fn flush_device_keys(&mut self) -> Result<()> {
        if !self.keys_on_device() || self.device_keys.unreadable {
            return Ok(());
        }
        let now = self.key_targets();
        if now == self.device_keys.written {
            return Ok(());
        }
        let mut secrets = self.load_device_secrets()?;
        secrets
            .bybit
            .retain(|entry| now.iter().any(|(id, ..)| *id == entry.keys.store_id));
        for entry in &mut secrets.bybit {
            if let Some((_, spot, futures)) = now.iter().find(|(id, ..)| *id == entry.keys.store_id)
            {
                entry.keys.spot_store_id = spot.clone();
                entry.keys.futures_store_id = futures.clone();
            }
        }
        device_secrets::save(&self.path, &*self.device_seal_key()?, &secrets)?;
        self.device_keys
            .meta
            .retain(|id, _| now.iter().any(|(kept, ..)| kept == id));
        self.device_keys.written = now;
        Ok(())
    }

    /// The mask of the key stored for exactly this store.
    pub(super) fn exchange_key_hint(&self, keys: &ExchangeKeys) -> String {
        if self.keys_on_device() {
            self.device_keys
                .meta
                .get(&keys.store_id)
                .map(|meta| meta.hint.clone())
                .unwrap_or_default()
        } else {
            keys.hint()
        }
    }

    /// The key that serves a store (its own, else its wallet's), read from
    /// the device file for this one request. Below level 3:
    /// `SECURITY_LEVEL_REQUIRED`; a damaged file: `SECRET`.
    pub fn device_key(&self, store_id: &str) -> Result<Option<DeviceKey>> {
        if !self.keys_on_device() {
            return Err(Error::SecurityLevelRequired);
        }
        // A file that no longer opens hides which stores it held.
        if self.device_keys.unreadable {
            return Err(Error::Secret);
        }
        let Some(owner) = self
            .bybit_key_for(store_id)
            .map(|keys| keys.store_id.clone())
        else {
            return Ok(None);
        };
        let secrets = self.load_device_secrets()?;
        Ok(secrets.bybit_for(&owner).cloned())
    }

    /// The first key of the vault (a call that names no store).
    pub fn first_device_key(&self) -> Result<Option<DeviceKey>> {
        match self
            .doc
            .exchanges
            .bybit
            .first()
            .map(|keys| keys.store_id.clone())
        {
            Some(store_id) => self.device_key(&store_id),
            None if self.device_keys.unreadable => Err(Error::Secret),
            None if self.keys_on_device() => Ok(None),
            None => Err(Error::SecurityLevelRequired),
        }
    }

    /// The key a network request for `store_id` (`None` = any wallet, as the
    /// global trade form asks) uses, decrypted for this request only.
    /// `Ok(None)` = no key. Below level 3 a key still in the vault is not
    /// used: `SecurityLevelRequired`, so the caller offers the upgrade; with
    /// no such key it is simply `Ok(None)`. An unreadable file is `Secret`.
    pub fn request_key(&self, store_id: Option<&str>) -> Result<Option<DeviceKey>> {
        if self.keys_on_device() {
            return match store_id {
                Some(id) => self.device_key(id),
                None => self.first_device_key(),
            };
        }
        let legacy = match store_id {
            Some(id) => self.bybit_key_for(id).is_some_and(has_secret),
            None => self.doc.exchanges.bybit.iter().any(has_secret),
        };
        if legacy {
            Err(Error::SecurityLevelRequired)
        } else {
            Ok(None)
        }
    }

    /// Where the key of a store stands on this device.
    pub fn exchange_key_state(&self, store_id: &str) -> ExchangeKeyState {
        if self.security_level() != SecurityLevel::Crypto {
            return ExchangeKeyState::LevelRequired;
        }
        let Some(keys) = self.bybit_key_for(store_id) else {
            return ExchangeKeyState::Missing;
        };
        if self.device_keys.unreadable {
            return ExchangeKeyState::Missing;
        }
        match self.device_keys.meta.get(&keys.store_id) {
            Some(meta) if meta.verified => ExchangeKeyState::ReadOnly,
            Some(_) => ExchangeKeyState::NotVerified,
            None => ExchangeKeyState::Missing,
        }
    }

    /// Whether this device holds an exchange key of this vault.
    pub fn has_device_keys(&self) -> bool {
        self.keys_on_device() && !self.doc.exchanges.bybit.is_empty()
    }

    /// The checks a key passes before anything is asked of the exchange:
    /// level 3, both parts given, an exchange store with a supported provider.
    pub fn check_exchange_key_input(
        &self,
        store_id: &str,
        api_key: &str,
        api_secret: &str,
    ) -> Result<()> {
        if self.security_level() != SecurityLevel::Crypto {
            return Err(Error::SecurityLevelRequired);
        }
        if api_key.trim().is_empty() || api_secret.trim().is_empty() {
            return Err(Error::InvalidInput(
                "api key and secret are required".into(),
            ));
        }
        let store = self
            .find_store(store_id)
            .ok_or_else(|| Error::Missing(format!("store {store_id}")))?;
        if store.kind != StoreKind::Exchange {
            return Err(Error::InvalidInput("store is not an exchange".into()));
        }
        // Keys only make sense once the user said which exchange this is.
        if !store
            .exchange
            .as_ref()
            .is_some_and(|profile| profile.provider.supported())
        {
            return Err(Error::WalletRule(
                crate::error::WalletRule::ProviderRequired,
            ));
        }
        Ok(())
    }

    /// Store (or replace) the key of an exchange store in the device file.
    /// `verified` = the read-only check passed (`false`: it could not run).
    /// Sub-account targets of a replaced key are kept.
    pub fn store_device_key(
        &mut self,
        store_id: &str,
        api_key: &str,
        api_secret: &str,
        verified: bool,
    ) -> Result<()> {
        self.check_exchange_key_input(store_id, api_key, api_secret)?;
        if self.sandbox {
            return Err(Error::Sandbox(
                "exchange keys are not kept in a sandbox".into(),
            ));
        }
        let (spot_store_id, futures_store_id) = self
            .doc
            .exchanges
            .bybit
            .iter()
            .find(|entry| entry.store_id == store_id)
            .map(|entry| (entry.spot_store_id.clone(), entry.futures_store_id.clone()))
            .unwrap_or_default();
        let keys = ExchangeKeys {
            store_id: store_id.to_string(),
            api_key: api_key.trim().to_string(),
            api_secret: api_secret.trim().to_string(),
            spot_store_id,
            futures_store_id,
        };
        // A damaged file holds nothing usable: the new key replaces it.
        let mut secrets = if self.device_keys.unreadable {
            DeviceSecrets::default()
        } else {
            self.load_device_secrets()?
        };
        secrets
            .bybit
            .retain(|entry| entry.keys.store_id != store_id);
        // Keys of stores the index dropped since the last save go too.
        let index = self.key_targets();
        secrets
            .bybit
            .retain(|entry| index.iter().any(|(id, ..)| *id == entry.keys.store_id));
        self.doc
            .exchanges
            .bybit
            .retain(|entry| entry.store_id != store_id);
        self.doc.exchanges.bybit.push(blank(&keys));
        secrets.bybit.push(DeviceKey { keys, verified });
        device_secrets::save(&self.path, &*self.device_seal_key()?, &secrets)?;
        self.device_keys.unreadable = false;
        self.index_from(&secrets);
        Ok(())
    }

    /// The read-only check passed for a key stored unverified.
    pub fn mark_device_key_verified(&mut self, store_id: &str) -> Result<()> {
        if !self.keys_on_device() {
            return Err(Error::SecurityLevelRequired);
        }
        let mut secrets = self.load_device_secrets()?;
        let Some(entry) = secrets
            .bybit
            .iter_mut()
            .find(|entry| entry.keys.store_id == store_id)
        else {
            return Ok(());
        };
        if entry.verified {
            return Ok(());
        }
        entry.verified = true;
        device_secrets::save(&self.path, &*self.device_seal_key()?, &secrets)?;
        if let Some(meta) = self.device_keys.meta.get_mut(store_id) {
            meta.verified = true;
        }
        Ok(())
    }

    /// Forget the key of exactly this store: the index and the device file.
    pub fn clear_device_key(&mut self, store_id: &str) -> Result<()> {
        self.doc
            .exchanges
            .bybit
            .retain(|entry| entry.store_id != store_id);
        if self.keys_on_device() {
            self.flush_device_keys()?;
        }
        self.persist()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::store::ExchangeProvider;
    use crate::service::StoreExtras;
    use crate::service::test_support::*;

    fn bybit(vault: &mut UnlockedVault, name: &str) -> String {
        vault
            .create_store_with(
                name,
                StoreKind::Exchange,
                "#f0b90b",
                "",
                None,
                StoreExtras {
                    provider: Some(ExchangeProvider::Bybit),
                    ..Default::default()
                },
            )
            .unwrap()
            .id
    }

    fn level3_with_key() -> (tempfile::TempDir, UnlockedVault, String) {
        let (dir, mut vault) = empty_vault();
        vault.raise_to_level3_for_tests();
        let store = bybit(&mut vault, "Bybit");
        vault
            .store_device_key(&store, "KEYKEYKEY123", "SECRETSECRET456", true)
            .unwrap();
        vault.flush_pending().unwrap();
        (dir, vault, store)
    }

    fn contains(haystack: &[u8], needle: &str) -> bool {
        haystack
            .windows(needle.len())
            .any(|window| window == needle.as_bytes())
    }

    #[test]
    fn below_level_3_a_key_is_refused_and_the_state_says_so() {
        let (_dir, mut vault) = empty_vault();
        let store = bybit(&mut vault, "Bybit");
        assert!(matches!(
            vault.set_exchange_key(&store, "key", "secret"),
            Err(Error::SecurityLevelRequired)
        ));
        assert_eq!(
            vault.exchange_key_state(&store),
            ExchangeKeyState::LevelRequired
        );
        assert!(matches!(
            vault.device_key(&store),
            Err(Error::SecurityLevelRequired)
        ));
        assert!(!vault.has_device_keys());
        assert_eq!(
            Error::SecurityLevelRequired.code(),
            "SECURITY_LEVEL_REQUIRED"
        );
        assert_eq!(
            Error::ExchangeKeyNotReadOnly.code(),
            "EXCHANGE_KEY_NOT_READ_ONLY"
        );
    }

    #[test]
    fn a_key_lives_in_the_device_file_not_in_the_vault() {
        let (_dir, vault, store) = level3_with_key();
        let file = std::fs::read(&vault.path).unwrap();
        let (doc, _) = crate::storage::VaultFile::open(&vault.path, "pw").unwrap();
        assert!(
            doc.exchanges.bybit.is_empty(),
            "the document field stays empty"
        );
        assert!(!contains(&file, "SECRETSECRET456"));
        assert!(device_secrets::secrets_path(&vault.path).exists());
        let key = vault.device_key(&store).unwrap().expect("key");
        assert_eq!(key.keys.api_secret, "SECRETSECRET456");
        assert!(key.verified);
        assert_eq!(vault.exchange_key_state(&store), ExchangeKeyState::ReadOnly);
        assert_eq!(
            vault.exchange_key_status(&store).hint.as_deref(),
            Some("KEYK…Y123")
        );
        assert!(vault.has_device_keys());
    }

    #[test]
    fn the_json_export_has_no_key() {
        let (_dir, vault, _store) = level3_with_key();
        let json = crate::storage::VaultFile::export_json(&vault.doc).unwrap();
        assert!(!json.contains("SECRETSECRET456"));
        assert!(!json.contains("KEYKEYKEY123"));
        assert!(!json.contains("\"exchanges\""));
    }

    #[test]
    fn a_backup_has_no_key() {
        let (_dir, mut vault, _store) = level3_with_key();
        let backup = vault.create_backup().unwrap();
        let path = crate::vault_io::backups::resolve(&vault.path, &backup.name).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert!(!contains(&bytes, "SECRETSECRET456"));
        let (doc, _) = crate::storage::VaultFile::open(&path, "pw").unwrap();
        assert!(doc.exchanges.bybit.is_empty());
    }

    #[test]
    fn the_sync_records_have_no_key() {
        let (_dir, mut vault, _store) = level3_with_key();
        let manifest = vault.sync_manifest(None).unwrap();
        let (wants, _) = vault.sync_diff(&Default::default(), &manifest);
        let records = vault.sync_records(&wants).unwrap();
        let text = serde_json::to_string(&records).unwrap();
        assert!(!text.contains("SECRETSECRET456"));
        assert!(!text.contains("KEYKEYKEY123"));
        assert!(!manifest.keys().any(|key| key.starts_with("exchanges")));
    }

    #[test]
    fn an_old_vault_with_keys_moves_them_to_the_device_file() {
        let (dir, mut vault) = empty_vault();
        let store = bybit(&mut vault, "Bybit");
        // A document from before H2: the key sits in the vault itself.
        vault.doc.exchanges.bybit.push(ExchangeKeys {
            store_id: store.clone(),
            api_key: "OLDKEY123456".into(),
            api_secret: "OLDSECRET789".into(),
            spot_store_id: None,
            futures_store_id: Some(store.clone()),
        });
        vault.persist().unwrap();
        // Level 2 keeps it as it was, and does not use it.
        assert_eq!(
            vault.exchange_key_state(&store),
            ExchangeKeyState::LevelRequired
        );
        assert!(vault.exchange_key_status(&store).configured);
        vault.raise_to_level3_for_tests();
        let path = vault.path.clone();
        drop(vault);
        assert!(!contains(&std::fs::read(&path).unwrap(), "OLDSECRET789"));

        let reopened = UnlockedVault::open(path.clone(), "pw").unwrap();
        let (doc, _) = crate::storage::VaultFile::open(&path, "pw").unwrap();
        assert!(doc.exchanges.bybit.is_empty());
        let key = reopened.device_key(&store).unwrap().expect("moved");
        assert_eq!(key.keys.api_secret, "OLDSECRET789");
        assert_eq!(key.keys.futures_store_id.as_deref(), Some(store.as_str()));
        // Moved keys were never checked: the next import checks them.
        assert_eq!(
            reopened.exchange_key_state(&store),
            ExchangeKeyState::NotVerified
        );
        drop(dir);
    }

    #[test]
    fn an_old_level3_file_with_keys_moves_them_on_open() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("old.wallet");
        let kdf = crate::crypto::kdf_profile::KdfParams::hardened(
            crate::crypto::kdf_profile::KDF_MEM_KIB_MIN,
            1,
            1,
        );
        let mut doc = crate::model::vault::VaultDoc::default();
        doc.exchanges.bybit.push(ExchangeKeys {
            store_id: "s1".into(),
            api_key: "OLDKEY123456".into(),
            api_secret: "OLDSECRET789".into(),
            spot_store_id: None,
            futures_store_id: None,
        });
        let key = SealKey::fresh_with(b"pw", kdf).unwrap();
        let fmt = crate::storage::SnapshotFormat::Pack;
        crate::storage::VaultFile::seal_and_write(
            &path,
            &key,
            &crate::storage::encode(&doc, fmt).unwrap(),
            fmt,
        )
        .unwrap();
        let vault = UnlockedVault::open(path.clone(), "pw").unwrap();
        drop(vault);
        let (on_disk, _) = crate::storage::VaultFile::open(&path, "pw").unwrap();
        assert!(
            on_disk.exchanges.bybit.is_empty(),
            "rewritten without the key"
        );
        assert!(!contains(&std::fs::read(&path).unwrap(), "OLDSECRET789"));
        let raw = std::fs::read(device_secrets::secrets_path(&path)).unwrap();
        assert!(!contains(&raw, "OLDSECRET789"));
        let vault = UnlockedVault::open(path, "pw").unwrap();
        let moved = vault.device_secrets_for_request().unwrap();
        assert_eq!(moved.bybit.len(), 1);
        assert_eq!(moved.bybit[0].keys.api_secret, "OLDSECRET789");
    }

    #[test]
    fn a_damaged_device_file_is_a_secret_error_not_a_panic() {
        let (_dir, vault, store) = level3_with_key();
        let path = vault.path.clone();
        drop(vault);
        let secrets = device_secrets::secrets_path(&path);
        let mut bytes = std::fs::read(&secrets).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 0x40;
        std::fs::write(&secrets, &bytes).unwrap();
        // The vault still opens; the key does not.
        let vault = UnlockedVault::open(path, "pw").unwrap();
        assert!(matches!(vault.device_key(&store), Err(Error::Secret)));
        assert!(matches!(
            vault.device_secrets_for_request(),
            Err(Error::Secret)
        ));
        assert!(vault.import_schedule().is_empty());
    }

    #[test]
    fn retargets_and_removals_reach_the_device_file() {
        let (_dir, mut vault, store) = level3_with_key();
        let sub = vault
            .create_store_with(
                "Futures",
                StoreKind::Exchange,
                "",
                "",
                Some(store.clone()),
                StoreExtras {
                    sub_type: Some(crate::model::store::SubType::Futures),
                    ..Default::default()
                },
            )
            .unwrap()
            .id;
        vault
            .set_exchange_targets(&store, None, Some(sub.clone()))
            .unwrap();
        vault.flush_pending().unwrap();
        let on_file = vault.device_secrets_for_request().unwrap();
        assert_eq!(
            on_file.bybit[0].keys.futures_store_id.as_deref(),
            Some(sub.as_str())
        );
        // A sub-account uses its wallet's key.
        assert!(vault.device_key(&sub).unwrap().is_some());
        vault.clear_device_key(&store).unwrap();
        vault.flush_pending().unwrap();
        assert!(!device_secrets::secrets_path(&vault.path).exists());
        assert_eq!(vault.exchange_key_state(&store), ExchangeKeyState::Missing);
    }

    #[test]
    fn an_unverified_key_is_marked_once_the_check_passes() {
        let (_dir, mut vault) = empty_vault();
        vault.raise_to_level3_for_tests();
        let store = bybit(&mut vault, "Bybit");
        vault
            .set_exchange_key(&store, "KEYKEYKEY123", "SECRET")
            .unwrap();
        assert_eq!(
            vault.exchange_key_state(&store),
            ExchangeKeyState::NotVerified
        );
        vault.mark_device_key_verified(&store).unwrap();
        assert_eq!(vault.exchange_key_state(&store), ExchangeKeyState::ReadOnly);
        assert!(vault.device_key(&store).unwrap().unwrap().verified);
    }

    #[test]
    fn a_replaced_doc_keeps_the_device_keys() {
        let (_dir, mut vault, store) = level3_with_key();
        let mut copy = vault.doc.clone();
        copy.exchanges.bybit.clear();
        vault.replace_doc(copy).unwrap();
        assert_eq!(vault.exchange_key_state(&store), ExchangeKeyState::ReadOnly);
    }

    #[test]
    fn a_rekey_keeps_the_keys_under_the_new_vault_key() {
        let (_dir, vault, store) = level3_with_key();
        let old = vault.key.clone().unwrap();
        let new = SealKey::fresh_with(b"new", old.kdf()).unwrap();
        device_secrets::rekey_device_secrets(&vault.path, &old, &new).unwrap();
        let secrets = device_secrets::load(&vault.path, &new).unwrap();
        assert_eq!(
            secrets.bybit_for(&store).unwrap().keys.api_secret,
            "SECRETSECRET456"
        );
        assert!(matches!(
            device_secrets::load(&vault.path, &old),
            Err(Error::Secret)
        ));
    }

    #[test]
    fn a_new_vault_drops_a_stale_device_file_of_the_same_name() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v.wallet");
        std::fs::write(device_secrets::secrets_path(&path), b"stale").unwrap();
        let _vault = UnlockedVault::create(path.clone(), "pw").unwrap();
        assert!(!device_secrets::secrets_path(&path).exists());
    }
}
