//! Storage layer: serializing the [`VaultDoc`] snapshot and persisting the
//! encrypted container.
//!
//! Snapshot encoding depends on the build profile:
//! - `debug` build  -> pretty JSON (readable, easy to inspect & diff)
//! - `release` build -> MessagePack with field names
//!
//! v1 release builds wrote bincode, which is positional: a field added
//! anywhere broke every older file. It is still read, never written. Both
//! JSON and MessagePack carry field names, so a field added with
//! `#[serde(default)]` keeps older v2 files readable without a version bump.
//!
//! Every encoding lives inside the SAME encrypted container (see `crypto`),
//! and a `payload_fmt` byte in the header makes any file self-describing.
//!
//! ## Versions and migrations
//!
//! The payload's `version` is read first, then the document is decoded with
//! the schema of that version and migrated up step by step (`v1` → `v2` →
//! current; v3 runs automations as workflows, see `v2`). The steps are
//! composed from one listing in [`chain`], not by hand.
//! A version from the future is refused. To change the schema in a way an
//! older file cannot be read with: freeze the current shape in a `vN` module,
//! bump [`crate::VAULT_FORMAT_VERSION`], add the step (one `From`, one
//! `impl Legacy`, one line in `schema_chain!`) and its tests.

use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;

use crate::crypto;
use crate::crypto::SealKey;
use crate::crypto::kdf_profile::KdfParams;
use crate::error::{Error, Result};
use crate::ipc_contract::security::RaiseStage;
use crate::model::vault::VaultDoc;
use crate::vault_io::local::{
    create_synced, read_back, reencrypt_tmp_path, replace_with, still_the_written_file,
};

pub mod chain;
pub mod v1;
pub mod v2;

pub use crate::vault_io::local::{VAULT_EXT, VaultLock, backup_path, ensure_wallet_ext};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotFormat {
    Json,
    /// Legacy bincode (v1 release builds); read-only.
    Bin,
    /// MessagePack with field names (release builds from payload v2).
    Pack,
}

impl SnapshotFormat {
    pub fn from_u8(raw: u8) -> Option<Self> {
        match raw {
            crypto::header::VaultHeader::FMT_JSON => Some(SnapshotFormat::Json),
            crypto::header::VaultHeader::FMT_BIN => Some(SnapshotFormat::Bin),
            crypto::header::VaultHeader::FMT_PACK => Some(SnapshotFormat::Pack),
            _ => None,
        }
    }

    pub fn as_u8(self) -> u8 {
        match self {
            SnapshotFormat::Json => crypto::header::VaultHeader::FMT_JSON,
            SnapshotFormat::Bin => crypto::header::VaultHeader::FMT_BIN,
            SnapshotFormat::Pack => crypto::header::VaultHeader::FMT_PACK,
        }
    }

    /// The concrete format for this build profile (dev vs prod).
    pub fn for_profile() -> Self {
        if cfg!(debug_assertions) {
            SnapshotFormat::Json
        } else {
            SnapshotFormat::Pack
        }
    }

    /// The format a document read in `self` is written back in: bincode is
    /// never written again.
    pub fn for_writing(self) -> Self {
        match self {
            SnapshotFormat::Bin => SnapshotFormat::Pack,
            other => other,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SnapshotFormat::Json => "json",
            SnapshotFormat::Bin => "bin",
            SnapshotFormat::Pack => "pack",
        }
    }
}

/// Encode a snapshot into the raw payload bytes for a given format.
pub fn encode(doc: &VaultDoc, fmt: SnapshotFormat) -> Result<Vec<u8>> {
    match fmt {
        SnapshotFormat::Json => {
            serde_json::to_vec_pretty(doc).map_err(|err| Error::Serialize(err.to_string()))
        }
        SnapshotFormat::Bin => bincode::serde::encode_to_vec(doc, bincode::config::standard())
            .map_err(|err| Error::Serialize(err.to_string())),
        SnapshotFormat::Pack => {
            rmp_serde::to_vec_named(doc).map_err(|err| Error::Serialize(err.to_string()))
        }
    }
}

/// A decoded document and the payload version it was stored at.
#[derive(Debug, Clone)]
pub struct Decoded {
    pub doc: VaultDoc,
    /// The version on disk; below [`crate::VAULT_FORMAT_VERSION`] when migrated.
    pub from_version: u32,
}

/// Decode raw payload bytes according to the format written in the file,
/// migrating an older version up to the current one.
pub fn decode(bytes: &[u8], fmt: SnapshotFormat) -> Result<VaultDoc> {
    decode_versioned(bytes, fmt).map(|decoded| decoded.doc)
}

/// [`decode`], also telling which version the payload was stored at.
///
/// A version from the future is [`Error::Unsupported`]; a payload that claims
/// a known version but does not fit its schema is [`Error::IncompatibleVault`].
pub fn decode_versioned(bytes: &[u8], fmt: SnapshotFormat) -> Result<Decoded> {
    let Some(version) = stored_version(bytes, fmt) else {
        // Not even a version to read: report why the payload does not parse.
        parse::<VaultDoc>(bytes, fmt)?;
        return Err(Error::Unsupported);
    };
    let doc = chain::upgrade(version, bytes, fmt)?;
    Ok(Decoded {
        doc,
        from_version: version,
    })
}

/// Just the `version` of a payload.
#[derive(serde::Deserialize)]
struct VersionProbe {
    version: u32,
}

/// The `version` a payload claims, read without decoding the rest of it.
/// `version` is the first field of `VaultDoc`, so for bincode it is the first value.
fn stored_version(bytes: &[u8], fmt: SnapshotFormat) -> Option<u32> {
    match fmt {
        SnapshotFormat::Json => serde_json::from_slice::<serde_json::Value>(bytes)
            .ok()?
            .get("version")?
            .as_u64()
            .and_then(|version| u32::try_from(version).ok()),
        SnapshotFormat::Bin => {
            bincode::serde::decode_from_slice::<u32, _>(bytes, bincode::config::standard())
                .ok()
                .map(|(version, _)| version)
        }
        SnapshotFormat::Pack => rmp_serde::from_slice::<VersionProbe>(bytes)
            .ok()
            .map(|probe| probe.version),
    }
}

/// Decode a payload as `T` in the given format.
fn parse<T: DeserializeOwned>(bytes: &[u8], fmt: SnapshotFormat) -> Result<T> {
    match fmt {
        SnapshotFormat::Json => {
            serde_json::from_slice::<T>(bytes).map_err(|err| Error::Deserialize(err.to_string()))
        }
        SnapshotFormat::Bin => {
            let (doc, consumed) =
                bincode::serde::decode_from_slice::<T, _>(bytes, bincode::config::standard())
                    .map_err(|err| Error::Deserialize(err.to_string()))?;
            if consumed != bytes.len() {
                return Err(Error::Deserialize(
                    "trailing bytes in bincode payload".into(),
                ));
            }
            Ok(doc)
        }
        SnapshotFormat::Pack => {
            rmp_serde::from_slice::<T>(bytes).map_err(|err| Error::Deserialize(err.to_string()))
        }
    }
}

/// Read-only helpers applied to a vault file on disk.
pub struct VaultFile;

/// The points of a re-encryption a test can break.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReencryptStep {
    Written,
    Verified,
}

impl VaultFile {
    /// Create a brand new encrypted vault at `path` (refuses to overwrite).
    /// Tests only: it seals at the standard cost, so outside them a vault is
    /// created with a [`SealKey`] (`create_keyed`) that carries its cost.
    #[cfg(test)]
    pub(crate) fn create(path: &Path, password: &str) -> Result<PathBuf> {
        if path.exists() {
            return Err(Error::AlreadyExists(path.display().to_string()));
        }
        let doc = VaultDoc::default();
        let fmt = SnapshotFormat::for_profile();
        Self::save_impl(path, password, &doc, fmt, /*overwrite=*/ false)?;
        Ok(path.to_path_buf())
    }

    /// Open an existing vault, decrypt and decode it.
    pub fn open(path: &Path, password: &str) -> Result<(VaultDoc, SnapshotFormat)> {
        Self::open_versioned(path, password).map(|(decoded, fmt)| (decoded.doc, fmt))
    }

    /// [`Self::open`], also telling the payload version found on disk.
    pub fn open_versioned(path: &Path, password: &str) -> Result<(Decoded, SnapshotFormat)> {
        Self::open_keyed(path, password).map(|(decoded, fmt, _)| (decoded, fmt))
    }

    /// [`Self::open_versioned`], also handing back the derived key: the
    /// session saves with it instead of running the KDF again.
    pub fn open_keyed(path: &Path, password: &str) -> Result<(Decoded, SnapshotFormat, SealKey)> {
        let bytes = crypto::read_all(path)?;
        let (plaintext, fmt, key) = crypto::open_bytes_keyed(&bytes, password.as_bytes())?;
        let decoded = decode_versioned(&plaintext, fmt)?;
        Ok((decoded, fmt, key))
    }

    /// Create a brand new vault at `path` sealed with `key` (refuses to overwrite).
    pub fn create_keyed(path: &Path, key: &SealKey) -> Result<PathBuf> {
        if path.exists() {
            return Err(Error::AlreadyExists(path.display().to_string()));
        }
        let fmt = SnapshotFormat::for_profile();
        let plaintext = encode(&VaultDoc::default(), fmt)?;
        Self::seal_and_write(path, key, &plaintext, fmt)?;
        Ok(path.to_path_buf())
    }

    /// Seal an encoded snapshot with the session key and write it atomically;
    /// hands back the bytes written (for a mirror copy). Runs no KDF, so a
    /// save costs the cipher and the disk, not a second of Argon2.
    pub fn seal_and_write(
        path: &Path,
        key: &SealKey,
        plaintext: &[u8],
        fmt: SnapshotFormat,
    ) -> Result<Vec<u8>> {
        let file = crypto::seal_with(key, fmt, plaintext)?;
        crypto::atomic_write(path, &file)?;
        Ok(file)
    }

    /// Move the vault at `path` onto a new key at the cost `kdf`, losing
    /// nothing: the new file is written next to the old one, opened from the
    /// disk with `password` and compared with `doc`, and only then takes the
    /// old file's place. On any failure the old file is as it was and the
    /// session key still fits it. On success the caller swaps in the returned
    /// key: until then saves would go out at the old cost.
    ///
    /// An empty `password` is refused like in `set_password`: the crypto
    /// level is never a vault without one.
    ///
    /// `doc` must be what the file holds, so the caller flushes pending saves
    /// first and keeps the vault locked. `on_stage` reports each stage before
    /// it starts; the backup before all this is the caller's.
    pub fn reencrypt_verified(
        path: &Path,
        doc: &VaultDoc,
        fmt: SnapshotFormat,
        password: &str,
        kdf: KdfParams,
        on_stage: impl FnMut(RaiseStage),
    ) -> Result<SealKey> {
        Self::reencrypt_with_faults(path, doc, fmt, password, kdf, on_stage, |_, _| Ok(()))
    }

    /// [`Self::reencrypt_verified`] with a `fault` called after each step, so
    /// a test can break the run exactly there.
    fn reencrypt_with_faults(
        path: &Path,
        doc: &VaultDoc,
        fmt: SnapshotFormat,
        password: &str,
        kdf: KdfParams,
        mut on_stage: impl FnMut(RaiseStage),
        mut fault: impl FnMut(ReencryptStep, &Path) -> Result<()>,
    ) -> Result<SealKey> {
        if password.is_empty() {
            return Err(Error::InvalidInput(
                "the new password must not be empty".into(),
            ));
        }
        on_stage(RaiseStage::Reencrypt);
        let fmt = fmt.for_writing();
        let key = SealKey::fresh_with(password.as_bytes(), kdf)?;
        let sealed = crypto::seal_with(&key, fmt, &encode(doc, fmt)?)?;

        // A name of its own per run (a leftover of a crash is swept when the
        // vault opens). The handle `create_synced` keeps is what gets read
        // back and verified, and the rename only goes ahead while the name
        // still points at that very file with those very bytes.
        let tmp = reencrypt_tmp_path(path);
        let mut file = create_synced(&tmp, &sealed)?;
        let result = fault(ReencryptStep::Written, &tmp)
            .and_then(|()| {
                on_stage(RaiseStage::Verify);
                let written = read_back(&mut file)?;
                Self::verify_reencrypted(&written, doc, password, kdf)?;
                if written != sealed {
                    return Err(Error::Crypto(
                        "the re-encrypted vault is not what was written".into(),
                    ));
                }
                Ok(written)
            })
            .and_then(|written| {
                fault(ReencryptStep::Verified, &tmp)?;
                still_the_written_file(&file, &tmp, &written)?;
                replace_with(&tmp, path)
            });
        drop(file);
        if result.is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
        result.map(|()| key)
    }

    fn verify_reencrypted(
        written: &[u8],
        doc: &VaultDoc,
        password: &str,
        kdf: KdfParams,
    ) -> Result<()> {
        let (plaintext, fmt, key) = crypto::open_bytes_keyed(written, password.as_bytes())?;
        let decoded = decode_versioned(&plaintext, fmt)?;
        if key.kdf() != kdf {
            return Err(Error::Crypto(
                "the re-encrypted vault has the wrong key cost".into(),
            ));
        }
        if decoded.doc != *doc {
            return Err(Error::Crypto(
                "the re-encrypted vault differs from the open one".into(),
            ));
        }
        Ok(())
    }

    /// Copy the file as it is before a migration rewrites it:
    /// `vault.wallet` → `vault.wallet.v1.bak`. An existing copy is kept, so
    /// the first, untouched file survives repeated opens.
    pub fn backup_version(path: &Path, version: u32) -> Result<PathBuf> {
        let mut name = path.file_name().unwrap_or_default().to_os_string();
        name.push(format!(".v{version}.bak"));
        let target = path.with_file_name(name);
        if !target.exists() {
            std::fs::copy(path, &target).map_err(|err| Error::Io(err.to_string()))?;
        }
        Ok(target)
    }

    /// Re-encrypt & atomically persist the whole document. Tests only, like
    /// [`Self::create`]: a fresh standard header would quietly lower a
    /// hardened vault's cost; real saves go through [`Self::seal_and_write`].
    #[cfg(test)]
    pub(crate) fn save(
        path: &Path,
        password: &str,
        doc: &VaultDoc,
        fmt: SnapshotFormat,
    ) -> Result<()> {
        Self::save_sealed(path, password, doc, fmt).map(|_| ())
    }

    /// [`Self::save`], handing back the exact bytes written (for a mirror copy).
    #[cfg(test)]
    pub(crate) fn save_sealed(
        path: &Path,
        password: &str,
        doc: &VaultDoc,
        fmt: SnapshotFormat,
    ) -> Result<Vec<u8>> {
        Self::save_impl(path, password, doc, fmt, /*overwrite=*/ true)
    }

    /// Export the decrypted snapshot as readable JSON (for debugging/backup).
    /// Exchange keys (or their index) never appear in it: they are the
    /// device's, not the document's.
    pub fn export_json(doc: &VaultDoc) -> Result<String> {
        let mut value =
            serde_json::to_value(doc).map_err(|err| Error::Serialize(err.to_string()))?;
        if let Some(fields) = value.as_object_mut() {
            fields.remove("exchanges");
        }
        serde_json::to_string_pretty(&value).map_err(|err| Error::Serialize(err.to_string()))
    }

    /// Copy the vault file on disk to its backup slot, replacing an older one.
    ///
    /// Used before a destructive whole-document replace so the previous vault
    /// stays recoverable. A missing source is not an error: there is simply
    /// nothing to back up yet, and the target path is still returned.
    pub fn backup(path: &Path) -> Result<PathBuf> {
        if !path.exists() {
            return Ok(backup_path(path));
        }
        let target = backup_path(path);
        std::fs::copy(path, &target).map_err(|err| Error::Io(err.to_string()))?;
        Ok(target)
    }

    #[cfg(test)]
    fn save_impl(
        path: &Path,
        password: &str,
        doc: &VaultDoc,
        fmt: SnapshotFormat,
        overwrite: bool,
    ) -> Result<Vec<u8>> {
        if path.exists() && !overwrite {
            return Err(Error::AlreadyExists(path.display().to_string()));
        }
        let plaintext = encode(doc, fmt)?;
        let (header, _, _) = crypto::random_header(fmt);
        let file = crypto::seal(&header, password.as_bytes(), &plaintext)?;
        crypto::atomic_write(path, &file)?;
        Ok(file)
    }
}

/// Magic + version of the file for quick format probing (UI detection).
pub fn probe(path: &Path) -> Result<SnapshotFormat> {
    let bytes = crypto::read_all(path)?;
    let (header, _) = crypto::parse_file(&bytes)?;
    header.payload_format().ok_or(Error::Unsupported)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Amount;
    use crate::VAULT_FORMAT_VERSION;
    use crate::model::transaction::{KindDetails, PriceStamp, TopupTx, Transaction, TxMeta};
    use crate::model::vault::{ExchangeKeys, Exchanges};
    use serde_json::Value as Json;

    fn sample_doc() -> VaultDoc {
        let mut doc = VaultDoc::default();
        let store = crate::model::store::Store::new(
            "Binance",
            crate::model::store::StoreKind::Exchange,
            "#f0b90b",
        );
        let btc = crate::model::store::AssetRef::crypto(
            "BTC",
            "Bitcoin",
            Some("bitcoin"),
            crate::model::chain::Chain::Bitcoin,
        );
        let mut meta = TxMeta::new("first buy");
        meta.categories = vec!["salary".into()];
        let tx = Transaction::Topup(TopupTx {
            meta,
            store_id: store.id.clone(),
            asset: btc.clone(),
            amount: Amount::parse("0.5").unwrap(),
            fee: None,
            from: None,
            details: KindDetails::default(),
            price: PriceStamp {
                price_usd: Some(Amount::parse("65000").unwrap()),
                ..Default::default()
            },
        });
        doc.stores.push(store);
        doc.holdings.push(crate::model::store::Holding {
            id: uuid::Uuid::new_v4().to_string(),
            store_id: doc.stores[0].id.clone(),
            asset: btc,
            quantity: Amount::parse("0.5").unwrap(),
            avg_cost: None,
        });
        doc.transactions.push(tx);
        doc
    }

    #[test]
    fn json_roundtrip_keeps_comments_and_amounts() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.wallet");
        let doc = sample_doc();
        VaultFile::create(&path, "pw").unwrap();
        VaultFile::save(&path, "pw", &doc, SnapshotFormat::Json).unwrap();

        let (out, fmt) = VaultFile::open(&path, "pw").unwrap();
        assert_eq!(fmt, SnapshotFormat::Json);
        assert_eq!(out.stores.len(), 1);
        assert_eq!(out.stores[0].name, "Binance");
        let tx = &out.transactions[0];
        assert_eq!(tx.tag(), "topup");
        assert_eq!(tx.meta().comment, "first buy");
        // The whole variant survives the round trip, not just its amount.
        assert_eq!(tx.legs()[0].amount.to_string(), "0.5");
        assert_eq!(tx.legs()[0].price_usd.unwrap().to_string(), "65000");
        assert_eq!(out.transactions, doc.transactions);
    }

    #[test]
    fn bin_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.wallet");
        let doc = sample_doc();
        VaultFile::create(&path, "pw").unwrap();
        VaultFile::save(&path, "pw", &doc, SnapshotFormat::Bin).unwrap();

        let (out, fmt) = VaultFile::open(&path, "pw").unwrap();
        assert_eq!(fmt, SnapshotFormat::Bin);
        assert_eq!(out.holdings[0].quantity.to_string(), "0.5");
        assert_eq!(out.transactions[0].tag(), "topup");
        assert_eq!(out.id, doc.id);
    }

    #[test]
    fn backup_copies_previous_vault_and_survives_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v.wallet");
        let doc = sample_doc();
        VaultFile::create(&path, "pw").unwrap();
        VaultFile::save(&path, "pw", &doc, SnapshotFormat::Json).unwrap();

        let bak = VaultFile::backup(&path).unwrap();
        assert_eq!(bak, dir.path().join("v.wallet.bak"));
        assert!(bak.exists());

        // A later save must not clobber the backup we just took.
        let mut emptied = sample_doc();
        emptied.transactions.clear();
        VaultFile::save(&path, "pw", &emptied, SnapshotFormat::Json).unwrap();

        let (out, _) = VaultFile::open(&bak, "pw").unwrap();
        assert_eq!(out.transactions.len(), doc.transactions.len());
        assert_eq!(out.id, doc.id);
    }

    #[test]
    fn backup_of_missing_vault_is_a_noop() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("absent.wallet");
        let bak = VaultFile::backup(&path).unwrap();
        assert_eq!(bak, dir.path().join("absent.wallet.bak"));
        assert!(!bak.exists());
    }

    #[test]
    fn rejects_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v.wallet");
        VaultFile::create(&path, "pw").unwrap();
        assert!(matches!(
            VaultFile::create(&path, "pw"),
            Err(Error::AlreadyExists(_))
        ));
    }

    #[test]
    fn wrong_file_magic() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("junk.wallet");
        std::fs::write(&path, b"not a vault at all").unwrap();
        assert!(matches!(
            VaultFile::open(&path, "pw"),
            Err(Error::Unsupported)
        ));
    }

    /// Roundtrip: encode then decode a fresh v1 document (JSON + Bin).
    #[test]
    fn roundtrip_v1_doc() {
        let doc = sample_doc();
        for fmt in [
            SnapshotFormat::Json,
            SnapshotFormat::Bin,
            SnapshotFormat::Pack,
        ] {
            let bytes = encode(&doc, fmt).unwrap();
            let out = decode(&bytes, fmt).unwrap();
            assert_eq!(out.version, VAULT_FORMAT_VERSION);
            assert_eq!(out.id, doc.id);
            assert_eq!(out.stores.len(), 1);
            assert_eq!(out.holdings[0].quantity.to_string(), "0.5");
            assert_eq!(out.transactions.len(), 1);
            assert_eq!(out.transactions[0].meta().comment, "first buy");
            assert_eq!(
                out.transactions[0].legs()[0].price_usd.unwrap().to_string(),
                "65000"
            );
        }
    }

    /// Multiple bybit accounts roundtrip (v1 with exchanges.bybit = Vec).
    #[test]
    fn roundtrip_v1_doc_with_multiple_bybit_accounts() {
        let store1 = crate::model::store::Store::new(
            "Bybit Main",
            crate::model::store::StoreKind::Exchange,
            "#f0b90b",
        );
        let store2 = crate::model::store::Store::new(
            "Bybit Sub",
            crate::model::store::StoreKind::Exchange,
            "#ff5722",
        );
        let store1_id = store1.id.clone();
        let store2_id = store2.id.clone();
        let doc = VaultDoc {
            stores: vec![store1, store2],
            exchanges: Exchanges {
                bybit: vec![
                    ExchangeKeys {
                        store_id: store1_id.clone(),
                        api_key: "aaaa1111".into(),
                        api_secret: "secret1".into(),
                        spot_store_id: None,
                        futures_store_id: None,
                    },
                    ExchangeKeys {
                        store_id: store2_id,
                        api_key: "bbbb2222".into(),
                        api_secret: "secret2".into(),
                        spot_store_id: Some("spot".into()),
                        futures_store_id: None,
                    },
                ],
            },
            ..Default::default()
        };
        for fmt in [
            SnapshotFormat::Json,
            SnapshotFormat::Bin,
            SnapshotFormat::Pack,
        ] {
            let bytes = encode(&doc, fmt).unwrap();
            let out = decode(&bytes, fmt).unwrap();
            assert_eq!(out.version, VAULT_FORMAT_VERSION);
            assert_eq!(out.exchanges.bybit.len(), 2);
            assert_eq!(out.exchanges.bybit[0].store_id, store1_id);
            assert_eq!(out.exchanges.bybit[1].api_key, "bbbb2222");
        }
    }

    /// A payload from any other version must be rejected (Unsupported).
    #[test]
    fn future_payload_version_is_rejected() {
        let doc = VaultDoc {
            version: VAULT_FORMAT_VERSION + 1,
            ..sample_doc()
        };
        for fmt in [SnapshotFormat::Bin, SnapshotFormat::Pack] {
            let err = decode(&encode(&doc, fmt).unwrap(), fmt)
                .expect_err("newer payload must be rejected");
            assert!(matches!(err, Error::Unsupported), "got {err:?}");
        }

        let mut json: Json = serde_json::to_value(&doc).unwrap();
        json["version"] = Json::from(VAULT_FORMAT_VERSION + 1);
        let err = decode(&serde_json::to_vec(&json).unwrap(), SnapshotFormat::Json)
            .expect_err("newer payload must be rejected");
        assert!(matches!(err, Error::Unsupported), "got {err:?}");
    }

    /// Plan D11: every persisted type of phases 2–7 exists now. A document
    /// with every collection filled must survive both encodings unchanged —
    /// the bincode half is what catches a type serde can write but not read
    /// back (internal tags, `flatten`, `Value`).
    #[test]
    fn a_full_document_round_trips_in_every_format() {
        let doc = crate::model::fixtures::full_doc();
        for fmt in [
            SnapshotFormat::Json,
            SnapshotFormat::Bin,
            SnapshotFormat::Pack,
        ] {
            let out = decode(&encode(&doc, fmt).unwrap(), fmt).unwrap();
            assert_eq!(out, doc, "{fmt:?}");
        }
    }

    /// The full document as a v1 file holds it, and what it becomes in v2.
    fn as_v1(doc: &VaultDoc) -> (Json, VaultDoc) {
        let mut json: Json = serde_json::to_value(doc).unwrap();
        json["version"] = Json::from(1);
        let root = json.as_object_mut().unwrap();
        root.remove("notification_log");
        root.remove("sync");
        root.remove("emergency_fund");
        root.remove("health_last");
        root.remove("asset_classes");
        root.remove("asset_class_of");
        root.remove("valuations");
        root.remove("tx_properties");
        // v1 had neither workflows nor automations as workflows.
        root.remove("workflows");
        root.remove("workflow_runs");
        root.insert("automations".into(), Json::Array(Vec::new()));
        root.insert("automation_runs".into(), Json::Array(Vec::new()));
        root["settings"]
            .as_object_mut()
            .unwrap()
            .remove("notifications");
        for contact in root["contacts"].as_array_mut().unwrap() {
            contact.as_object_mut().unwrap().remove("group");
        }
        for reminder in root["reminders"].as_array_mut().unwrap() {
            let reminder = reminder.as_object_mut().unwrap();
            for key in [
                "repeat",
                "source",
                "muted",
                "user_edited",
                "snoozed_until",
                "source_key",
            ] {
                reminder.remove(key);
            }
        }
        for reading in root["meter_readings"].as_array_mut().unwrap() {
            let reading = reading.as_object_mut().unwrap();
            reading.remove("day");
            reading.remove("zone");
        }
        for item in root["utility_items"].as_array_mut().unwrap() {
            if let Some(meter) = item
                .get_mut("kind")
                .and_then(|kind| kind.get_mut("metered"))
                .and_then(Json::as_object_mut)
            {
                meter.remove("formula");
                meter.remove("zones");
            }
        }
        let mut expected = doc.clone();
        expected.workflows.clear();
        expected.workflow_runs.clear();
        expected.notification_log.clear();
        expected.sync = crate::sync::SyncState::default();
        expected.emergency_fund = Default::default();
        expected.health_last = None;
        expected.asset_classes.clear();
        expected.asset_class_of.clear();
        expected.valuations.clear();
        expected.tx_properties.clear();
        expected.settings.notifications = crate::model::notify::NotificationSettings::default();
        // A migrated v1 vault has seen no category batch yet, so `open` adds them.
        expected.settings.categories_rev = 0;
        for contact in &mut expected.contacts {
            contact.group.clear();
        }
        for reminder in &mut expected.reminders {
            *reminder = crate::model::planning::Reminder {
                id: reminder.id.clone(),
                done: reminder.done,
                ..crate::model::planning::Reminder::new(&reminder.text, &reminder.due_at)
            };
        }
        for reading in &mut expected.meter_readings {
            reading.day = None;
            reading.zone = None;
        }
        for item in &mut expected.utility_items {
            if let crate::model::planning::UtilityKind::Metered { formula, zones, .. } =
                &mut item.kind
            {
                *formula = None;
                zones.clear();
            }
        }
        (json, expected)
    }

    /// A v1 file in either v1 encoding migrates to v2 with nothing lost.
    #[test]
    fn a_v1_document_migrates_from_json_and_bincode() {
        let (v1_json, expected) = as_v1(&crate::model::fixtures::full_doc());
        let json_bytes = serde_json::to_vec(&v1_json).unwrap();
        let decoded = decode_versioned(&json_bytes, SnapshotFormat::Json).unwrap();
        assert_eq!(decoded.from_version, 1);
        assert_eq!(decoded.doc, expected);

        // A bincode v1 payload is exactly the v1 field list, by position.
        let v1_doc: v1::VaultDocV1 = serde_json::from_value(v1_json).unwrap();
        let bin_bytes =
            bincode::serde::encode_to_vec(&v1_doc, bincode::config::standard()).unwrap();
        let decoded = decode_versioned(&bin_bytes, SnapshotFormat::Bin).unwrap();
        assert_eq!(decoded.from_version, 1);
        assert_eq!(decoded.doc, expected);
        assert_eq!(decoded.doc.version, VAULT_FORMAT_VERSION);
    }

    /// Opening a v1 vault keeps the untouched file as `.v1.bak`, rewrites the
    /// vault as v2 and never writes bincode again.
    #[test]
    fn opening_a_v1_vault_backs_it_up_and_rewrites_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("old.wallet");
        let (v1_json, expected) = as_v1(&crate::model::fixtures::full_doc());
        let v1_doc: v1::VaultDocV1 = serde_json::from_value(v1_json).unwrap();
        let plain = bincode::serde::encode_to_vec(&v1_doc, bincode::config::standard()).unwrap();
        let (header, _, _) = crypto::random_header(SnapshotFormat::Bin);
        let file = crypto::seal(&header, b"pw", &plain).unwrap();
        crypto::atomic_write(&path, &file).unwrap();

        let vault = crate::service::UnlockedVault::open(path.clone(), "pw").unwrap();
        assert_eq!(vault.doc.contacts, expected.contacts);
        assert_eq!(vault.doc.reminders, expected.reminders);
        drop(vault);

        let backup = dir.path().join("old.wallet.v1.bak");
        assert_eq!(
            std::fs::read(&backup).unwrap(),
            file,
            "backup is the untouched file"
        );
        let (decoded, fmt) = VaultFile::open_versioned(&path, "pw").unwrap();
        assert_eq!(fmt, SnapshotFormat::Pack);
        assert_eq!(decoded.from_version, VAULT_FORMAT_VERSION);

        // A second open neither migrates nor replaces the first backup.
        drop(crate::service::UnlockedVault::open(path.clone(), "pw").unwrap());
        assert_eq!(std::fs::read(&backup).unwrap(), file);
    }

    /// A v2 document with automations, as v2 stored them.
    fn v2_with_automations() -> Json {
        let mut json: Json = serde_json::to_value(sample_doc()).unwrap();
        json["version"] = Json::from(2);
        let template = serde_json::to_value(crate::model::planning::TransactionDraft::Topup(
            match crate::model::fixtures::full_doc()
                .transactions
                .into_iter()
                .find(|tx| matches!(tx, crate::model::transaction::Transaction::Topup(_)))
            {
                Some(crate::model::transaction::Transaction::Topup(inner)) => inner,
                _ => unreachable!(),
            },
        ))
        .unwrap();
        json["automations"] = serde_json::json!([{
            "id": "salary", "name": "Salary", "active": true, "preset": "salary",
            "schedule": { "monthly": { "day": 20 } },
            "starts_at": "2026-01-01T00:00:00Z", "template": template,
            "last_planned_at": "2026-09-20T09:00:00Z", "created_at": "2026-01-01T00:00:00Z"
        }]);
        json["automation_runs"] = serde_json::json!([
            { "id": "r1", "automation_id": "salary", "planned_at": "2026-09-20T09:00:00Z",
              "processed_at": "2026-09-20T09:00:01Z", "outcome": { "created": { "tx_id": "tx-1" } } },
            { "id": "r2", "automation_id": "salary", "planned_at": "2026-08-20T09:00:00Z",
              "processed_at": "2026-08-20T09:00:01Z", "outcome": { "rejected": { "reason": { "invalid": { "message": "archived" } } } } },
            { "id": "r3", "automation_id": "salary", "planned_at": "2026-07-20T09:00:00Z",
              "processed_at": "2026-07-20T09:00:01Z", "outcome": "skipped" }
        ]);
        json
    }

    /// v2 → v3: an automation becomes a workflow with the same id, its last
    /// date the schedule's mark, its runs workflow runs with the same outcome;
    /// in both v2 encodings.
    #[test]
    fn a_v2_automation_migrates_to_a_workflow_from_json_and_pack() {
        let json = v2_with_automations();
        for (bytes, fmt) in [
            (serde_json::to_vec(&json).unwrap(), SnapshotFormat::Json),
            (
                rmp_serde::to_vec_named(&json).unwrap(),
                SnapshotFormat::Pack,
            ),
        ] {
            let decoded = decode_versioned(&bytes, fmt).unwrap();
            assert_eq!(decoded.from_version, 2, "{fmt:?}");
            let doc = decoded.doc;
            assert_eq!(doc.version, VAULT_FORMAT_VERSION);
            let workflow = doc
                .workflows
                .iter()
                .find(|item| item.id == "salary")
                .unwrap();
            assert_eq!(workflow.name, "Salary");
            let spec = workflow.automation.as_ref().unwrap();
            assert_eq!(
                spec.schedule,
                crate::model::planning::Schedule::Monthly { day: 20 }
            );
            assert_eq!(
                spec.preset,
                Some(crate::model::planning::AutomationPreset::Salary)
            );
            let graph = crate::workflow::Graph::parse(&workflow.graph).unwrap();
            assert!(!crate::workflow::validate::has_errors(
                &crate::workflow::validate(&graph)
            ));
            let state = doc
                .workflow_state
                .iter()
                .find(|state| state.workflow_id == "salary")
                .unwrap();
            assert_eq!(
                state.marks.get("schedule:t").map(String::as_str),
                Some("2026-09-20T09:00:00Z")
            );
            let outcomes: Vec<_> = doc
                .workflow_runs
                .iter()
                .map(|run| (run.id.as_str(), run.outcome.clone(), run.created.clone()))
                .collect();
            assert!(outcomes.contains(&(
                "r1",
                crate::workflow::RunOutcome::Done,
                vec!["tx-1".to_string()]
            )));
            assert!(outcomes.iter().any(|(id, outcome, _)| *id == "r2"
                && matches!(outcome, crate::workflow::RunOutcome::Rejected { planned_at: Some(at), .. } if at == "2026-08-20T09:00:00Z")));
            assert!(outcomes.contains(&(
                "r3",
                crate::workflow::RunOutcome::Dropped {
                    reason: "skipped".into()
                },
                Vec::new()
            )));
        }
    }

    /// Opening a v2 vault with automations keeps the untouched file as
    /// `.v2.bak`, and the automations show as before.
    #[test]
    fn opening_a_v2_vault_backs_it_up_and_keeps_its_automations() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v2.wallet");
        let plain = rmp_serde::to_vec_named(&v2_with_automations()).unwrap();
        let (header, _, _) = crypto::random_header(SnapshotFormat::Pack);
        let file = crypto::seal(&header, b"pw", &plain).unwrap();
        crypto::atomic_write(&path, &file).unwrap();

        let vault = crate::service::UnlockedVault::open(path.clone(), "pw").unwrap();
        let automations = vault.automations();
        assert_eq!(automations.len(), 1);
        assert_eq!(
            automations[0].last_planned_at.as_deref(),
            Some("2026-09-20T09:00:00Z")
        );
        assert_eq!(vault.automation_runs(Some("salary")).len(), 3);
        drop(vault);
        assert_eq!(
            std::fs::read(dir.path().join("v2.wallet.v2.bak")).unwrap(),
            file
        );
        let (decoded, _) = VaultFile::open_versioned(&path, "pw").unwrap();
        assert_eq!(decoded.from_version, VAULT_FORMAT_VERSION);
    }

    /// v2 carries field names: a field missing from an older v2 file takes its
    /// default instead of breaking the read (the reason bincode was dropped).
    #[test]
    fn a_v2_pack_payload_tolerates_a_missing_defaulted_field() {
        let doc = crate::model::fixtures::full_doc();
        let mut json: Json = serde_json::to_value(&doc).unwrap();
        json.as_object_mut().unwrap().remove("notification_log");
        let bytes = rmp_serde::to_vec_named(&json).unwrap();
        let out = decode(&bytes, SnapshotFormat::Pack).unwrap();
        assert!(out.notification_log.is_empty());
        assert_eq!(out.contacts, doc.contacts);
    }

    /// A version-1 file from before the schema break says so, instead of
    /// reporting a parse error (plan §4.10).
    #[test]
    fn an_old_vault_is_reported_as_incompatible() {
        let mut json: Json = serde_json::to_value(sample_doc()).unwrap();
        json["transactions"] = serde_json::json!([{
            "income": {
                "meta": { "id": "x", "timestamp": "2026-01-01T00:00:00Z", "comment": "", "categories": [] },
                "store_id": "s", "asset": { "symbol": "USD", "name": "", "coingecko_id": null, "chain": null, "contract": null, "quote": "USD" },
                "amount": "1", "source": "salary", "price": {}
            }
        }]);
        let err = decode(&serde_json::to_vec(&json).unwrap(), SnapshotFormat::Json).unwrap_err();
        assert!(matches!(err, Error::IncompatibleVault(_)), "got {err:?}");
        assert_eq!(err.code(), "INCOMPATIBLE_VAULT");

        // bincode: the right version up front, then something else entirely.
        let old = bincode::serde::encode_to_vec(
            (VAULT_FORMAT_VERSION, "id", "2026-01-01T00:00:00Z", 8u32),
            bincode::config::standard(),
        )
        .unwrap();
        let err = decode(&old, SnapshotFormat::Bin).unwrap_err();
        assert!(matches!(err, Error::IncompatibleVault(_)), "got {err:?}");
    }

    #[test]
    fn corrupt_payload_is_an_error_not_a_panic() {
        let junk = vec![0xffu8; 32];
        assert!(decode(&junk, SnapshotFormat::Bin).is_err());
        assert!(decode(b"{not json", SnapshotFormat::Json).is_err());
        assert!(decode(&junk, SnapshotFormat::Pack).is_err());
    }

    /// The vault password now also guards an exchange secret, so the secret must
    /// never be reachable through logging or debugging output.
    #[test]
    fn exchange_keys_debug_redacts_the_secret() {
        let keys = ExchangeKeys {
            store_id: "store-1".into(),
            api_key: "abcdef123456".into(),
            api_secret: "s3cr3t".into(),
            spot_store_id: None,
            futures_store_id: None,
        };
        let shown = format!("{keys:?}");
        assert!(!shown.contains("s3cr3t"), "secret leaked: {shown}");
        assert!(shown.contains("<redacted>"), "{shown}");
        assert_eq!(keys.hint(), "abcd…3456");

        // Nor through the enclosing document.
        let doc = VaultDoc {
            exchanges: Exchanges { bybit: vec![keys] },
            ..Default::default()
        };
        assert!(!format!("{doc:?}").contains("s3cr3t"));
    }

    /// `hint()` must stay non-reversible even for very short keys.
    #[test]
    fn exchange_key_hint_never_reveals_short_keys() {
        assert_eq!(
            ExchangeKeys {
                store_id: "s".into(),
                api_key: "abcd".into(),
                api_secret: "x".into(),
                spot_store_id: None,
                futures_store_id: None,
            }
            .hint(),
            "****"
        );
    }

    // -- files of every version ---------------------------------------------

    /// The header as builds before the crypto level wrote it, field by field:
    /// nothing here follows the current constants.
    fn old_build_header(fmt: SnapshotFormat) -> crypto::header::VaultHeader {
        crypto::header::VaultHeader {
            version: 1,
            kdf_id: 0,
            kdf_mem_kib: 64 * 1024,
            kdf_iters: 3,
            kdf_par: 1,
            salt: [0x11; crypto::SALT_LEN],
            nonce: [0x22; crypto::NONCE_LEN],
            payload_fmt: fmt.as_u8(),
        }
    }

    #[test]
    fn a_full_document_from_an_old_build_opens_in_every_format() {
        let dir = tempfile::tempdir().unwrap();
        let doc = crate::model::fixtures::full_doc();
        for fmt in [
            SnapshotFormat::Json,
            SnapshotFormat::Bin,
            SnapshotFormat::Pack,
        ] {
            let path = dir.path().join(format!("{fmt:?}.wallet"));
            let header = old_build_header(fmt);
            let file = crypto::seal(&header, b"pw", &encode(&doc, fmt).unwrap()).unwrap();
            std::fs::write(&path, file).unwrap();

            let (decoded, read, key) = VaultFile::open_keyed(&path, "pw").unwrap();
            assert_eq!((decoded.doc, read), (doc.clone(), fmt));
            assert_eq!(key.kdf(), KdfParams::STANDARD);
            assert!(matches!(
                VaultFile::open(&path, "wrong"),
                Err(Error::Secret)
            ));
        }
    }

    #[test]
    fn a_plain_file_keeps_its_document() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("plain.wallet");
        let doc = crate::model::fixtures::full_doc();
        let fmt = SnapshotFormat::Pack;
        std::fs::write(&path, crypto::seal_plain(fmt, &encode(&doc, fmt).unwrap())).unwrap();
        let (decoded, _, key) = VaultFile::open_keyed(&path, "").unwrap();
        assert_eq!(decoded.doc, doc);
        assert!(key.is_plain());
    }

    // -- re-encryption ------------------------------------------------------

    /// The cheapest cost the limits allow: these tests run the KDF a lot.
    const TEST_KDF: KdfParams = KdfParams::hardened(64 * 1024, 1, 1);

    fn old_vault(dir: &Path) -> (PathBuf, VaultDoc, Vec<u8>) {
        let path = dir.join("v.wallet");
        let doc = crate::model::fixtures::full_doc();
        let key = SealKey::fresh(b"pw").unwrap();
        let fmt = SnapshotFormat::Pack;
        VaultFile::seal_and_write(&path, &key, &encode(&doc, fmt).unwrap(), fmt).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        (path, doc, bytes)
    }

    fn entries(dir: &Path) -> usize {
        std::fs::read_dir(dir).unwrap().count()
    }

    fn assert_old_vault_intact(path: &Path, doc: &VaultDoc, bytes: &[u8]) {
        assert_eq!(std::fs::read(path).unwrap(), bytes);
        assert_eq!(&VaultFile::open(path, "pw").unwrap().0, doc);
        assert_eq!(entries(path.parent().unwrap()), 1, "no temp file left");
    }

    fn faulted(
        path: &Path,
        doc: &VaultDoc,
        fault: impl FnMut(ReencryptStep, &Path) -> Result<()>,
    ) -> Result<SealKey> {
        VaultFile::reencrypt_with_faults(
            path,
            doc,
            SnapshotFormat::Pack,
            "pw",
            TEST_KDF,
            |_| {},
            fault,
        )
    }

    #[test]
    fn reencrypting_moves_the_vault_to_the_new_cost_with_its_data() {
        for fmt in [SnapshotFormat::Json, SnapshotFormat::Pack] {
            let dir = tempfile::tempdir().unwrap();
            let (path, doc, _) = old_vault(dir.path());
            let mut stages = Vec::new();
            let key = VaultFile::reencrypt_verified(&path, &doc, fmt, "pw", TEST_KDF, |stage| {
                stages.push(stage);
            })
            .unwrap();

            assert_eq!(stages, [RaiseStage::Reencrypt, RaiseStage::Verify]);
            assert_eq!(key.kdf(), TEST_KDF);
            let (decoded, read, reopened) = VaultFile::open_keyed(&path, "pw").unwrap();
            assert_eq!((decoded.doc, read), (doc, fmt));
            assert_eq!(reopened.kdf(), TEST_KDF);
            assert_eq!(entries(dir.path()), 1, "no temp file left");
            let level = crypto::header::level_of_container(&std::fs::read(&path).unwrap());
            assert_eq!(
                level.unwrap(),
                crate::ipc_contract::security::SecurityLevel::Crypto
            );
        }
    }

    #[test]
    fn a_failure_before_the_rename_leaves_the_old_file() {
        for step in [ReencryptStep::Written, ReencryptStep::Verified] {
            let dir = tempfile::tempdir().unwrap();
            let (path, doc, bytes) = old_vault(dir.path());
            let result = faulted(&path, &doc, |at, _| {
                if at == step {
                    Err(Error::Io("the power went out".into()))
                } else {
                    Ok(())
                }
            });
            assert!(matches!(result, Err(Error::Io(_))), "{step:?}");
            assert_old_vault_intact(&path, &doc, &bytes);
        }
    }

    #[test]
    fn a_failed_rename_leaves_the_old_file() {
        let dir = tempfile::tempdir().unwrap();
        let (path, doc, bytes) = old_vault(dir.path());
        let result = faulted(&path, &doc, |at, tmp| {
            if at == ReencryptStep::Verified {
                std::fs::remove_file(tmp).unwrap();
            }
            Ok(())
        });
        assert!(result.is_err());
        assert_old_vault_intact(&path, &doc, &bytes);
    }

    #[test]
    fn a_new_file_that_does_not_open_never_replaces_the_old_one() {
        let dir = tempfile::tempdir().unwrap();
        let (path, doc, bytes) = old_vault(dir.path());
        let result = faulted(&path, &doc, |at, tmp| {
            if at == ReencryptStep::Written {
                let mut file = std::fs::read(tmp).unwrap();
                let last = file.len() - 1;
                file[last] ^= 1;
                std::fs::write(tmp, file).unwrap();
            }
            Ok(())
        });
        assert!(matches!(result, Err(Error::Secret)));
        assert_old_vault_intact(&path, &doc, &bytes);
    }

    #[test]
    fn a_new_file_with_another_document_never_replaces_the_old_one() {
        let dir = tempfile::tempdir().unwrap();
        let (path, doc, bytes) = old_vault(dir.path());
        let result = faulted(&path, &doc, |at, tmp| {
            if at == ReencryptStep::Written {
                let key = SealKey::fresh_with(b"pw", TEST_KDF).unwrap();
                let fmt = SnapshotFormat::Pack;
                let empty = encode(&VaultDoc::default(), fmt).unwrap();
                std::fs::write(tmp, crypto::seal_with(&key, fmt, &empty).unwrap()).unwrap();
            }
            Ok(())
        });
        assert!(matches!(result, Err(Error::Crypto(_))));
        assert_old_vault_intact(&path, &doc, &bytes);
    }

    #[test]
    fn a_new_file_at_a_lower_cost_never_replaces_the_old_one() {
        let dir = tempfile::tempdir().unwrap();
        let (path, doc, bytes) = old_vault(dir.path());
        let result = faulted(&path, &doc, |at, tmp| {
            if at == ReencryptStep::Written {
                let key = SealKey::fresh(b"pw").unwrap();
                let fmt = SnapshotFormat::Pack;
                let same = encode(&doc, fmt).unwrap();
                std::fs::write(tmp, crypto::seal_with(&key, fmt, &same).unwrap()).unwrap();
            }
            Ok(())
        });
        assert!(matches!(result, Err(Error::Crypto(_))));
        assert_old_vault_intact(&path, &doc, &bytes);
    }

    #[test]
    fn a_temp_file_left_by_a_crash_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let (path, doc, bytes) = old_vault(dir.path());
        let leftover = crate::vault_io::local::tests::foreign_reencrypt_leftover(&path);
        std::fs::write(&leftover, b"half a file").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), bytes);

        VaultFile::reencrypt_verified(&path, &doc, SnapshotFormat::Pack, "pw", TEST_KDF, |_| {})
            .unwrap();
        assert_eq!(std::fs::read(&leftover).unwrap(), b"half a file");
        assert_eq!(VaultFile::open(&path, "pw").unwrap().0, doc);
        assert_eq!(entries(dir.path()), 2, "the vault and the old leftover");
    }

    #[test]
    fn opening_the_vault_sweeps_what_a_crashed_reencryption_left() {
        let dir = tempfile::tempdir().unwrap();
        let (path, _, _) = old_vault(dir.path());
        let leftover = crate::vault_io::local::tests::foreign_reencrypt_leftover(&path);
        std::fs::write(&leftover, b"half a file").unwrap();
        let wrong = crate::service::UnlockedVault::open(path.clone(), "wrong");
        assert!(matches!(wrong, Err(Error::Secret)));
        assert!(leftover.exists(), "a wrong password touches nothing");
        let vault = crate::service::UnlockedVault::open(path.clone(), "pw").unwrap();
        assert!(!leftover.exists());
        drop(vault);
    }

    #[test]
    fn a_temp_file_changed_after_verification_never_replaces_the_old_one() {
        let dir = tempfile::tempdir().unwrap();
        let (path, doc, bytes) = old_vault(dir.path());
        // Same document and cost, so only the swap check can catch it.
        let other = SealKey::fresh_with(b"pw", TEST_KDF).unwrap();
        let fmt = SnapshotFormat::Pack;
        let impostor = crypto::seal_with(&other, fmt, &encode(&doc, fmt).unwrap()).unwrap();
        let result = faulted(&path, &doc, |at, tmp| {
            if at == ReencryptStep::Verified {
                std::fs::write(tmp, &impostor).unwrap();
            }
            Ok(())
        });
        assert!(matches!(result, Err(Error::Io(_))), "{:?}", result.err());
        assert_old_vault_intact(&path, &doc, &bytes);
    }

    #[cfg(unix)]
    #[test]
    fn a_temp_file_swapped_for_a_symlink_never_replaces_the_old_one() {
        let dir = tempfile::tempdir().unwrap();
        let (path, doc, bytes) = old_vault(dir.path());
        let elsewhere = tempfile::tempdir().unwrap();
        let target = elsewhere.path().join("planted");
        std::fs::write(&target, b"planted").unwrap();
        let result = faulted(&path, &doc, |at, tmp| {
            if at == ReencryptStep::Verified {
                std::fs::remove_file(tmp).unwrap();
                std::os::unix::fs::symlink(&target, tmp).unwrap();
            }
            Ok(())
        });
        assert!(result.is_err());
        assert_old_vault_intact(&path, &doc, &bytes);
        assert_eq!(std::fs::read(&target).unwrap(), b"planted");
    }

    #[test]
    fn reencrypting_with_an_empty_password_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let (path, doc, bytes) = old_vault(dir.path());
        let mut stages = Vec::new();
        let result = VaultFile::reencrypt_verified(
            &path,
            &doc,
            SnapshotFormat::Pack,
            "",
            TEST_KDF,
            |stage| {
                stages.push(stage);
            },
        );
        assert!(matches!(result, Err(Error::InvalidInput(_))));
        assert!(stages.is_empty());
        assert_old_vault_intact(&path, &doc, &bytes);
    }

    #[test]
    fn a_vault_stays_at_its_cost_through_saves_and_reopening() {
        use crate::service::UnlockedVault;
        let dir = tempfile::tempdir().unwrap();
        let (path, doc, _) = old_vault(dir.path());
        VaultFile::reencrypt_verified(&path, &doc, SnapshotFormat::Pack, "pw", TEST_KDF, |_| {})
            .unwrap();

        let mut vault = UnlockedVault::open(path.clone(), "pw").unwrap();
        vault.switch_format(SnapshotFormat::Json).unwrap();
        drop(vault);

        let (decoded, read, key) = VaultFile::open_keyed(&path, "pw").unwrap();
        assert_eq!(read, SnapshotFormat::Json);
        assert_eq!(key.kdf(), TEST_KDF);
        // Opening repairs this fixture's tree and adds the cash wallet, so the
        // stores are not equal to the input; none of them goes.
        let kept: Vec<_> = decoded.doc.stores.iter().map(|store| &store.id).collect();
        assert!(doc.stores.iter().all(|store| kept.contains(&&store.id)));
    }
}
