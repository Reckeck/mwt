//! Storage layer: serializing the [`VaultDoc`] snapshot and persisting the
//! encrypted container.
//!
//! Snapshot encoding depends on the build profile:
//! - `debug` build  -> pretty JSON (readable, easy to inspect & diff)
//! - `release` build -> MessagePack with field names
//!
//! Both JSON and MessagePack carry field names, so a field added with
//! `#[serde(default)]` keeps older files readable without a version bump.
//! The positional binary encoding of old builds is refused (see [`crate::crypto::format::legacy_refusal`]).
//!
//! Every encoding lives inside the SAME encrypted container (see `crypto`),
//! and a `payload_fmt` byte in the header makes any file self-describing.
//!
//! ## Versions
//!
//! The payload's `version` is read first. Container 2 holds payload 1 (the
//! current format); container 1 holds the old payloads, of which only v4 is
//! converted once on open (see [`legacy_v4`]). A version from the future is
//! refused. To change the schema in a way an older file cannot be read with:
//! freeze the current shape in a `legacy_vN` module, bump
//! [`crate::VAULT_FORMAT_VERSION`], add the step and its tests.

use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;

use crate::crypto;
use crate::crypto::SealKey;
use crate::crypto::kdf_profile::KdfParams;
use crate::crypto::level::RaiseStage;
use crate::error::{Error, Result};
use crate::model::vault::VaultDoc;
use crate::vault_io::local::{
    create_synced, read_back, reencrypt_tmp_path, replace_with, still_the_written_file,
};

pub mod legacy_v4;

#[cfg(test)]
mod fixtures_gen;

pub use crate::vault_io::local::{VAULT_EXT, VaultLock, backup_path, ensure_wallet_ext};

#[cfg(test)]
use crate::crypto::format::LAST_LEGACY_BUILD;
pub use crate::crypto::format::SnapshotFormat;
use crate::crypto::format::legacy_refusal;

/// Encode a snapshot into the raw payload bytes for a given format.
pub fn encode(doc: &VaultDoc, fmt: SnapshotFormat) -> Result<Vec<u8>> {
    match fmt {
        SnapshotFormat::Json => {
            serde_json::to_vec_pretty(doc).map_err(|err| Error::Serialize(err.to_string()))
        }
        SnapshotFormat::Pack => {
            rmp_serde::to_vec_named(doc).map_err(|err| Error::Serialize(err.to_string()))
        }
    }
}

/// The payload version of an old v4 file, read once on open and converted
/// (see [`legacy_v4`]).
const TRANSITION_FROM_VERSION: u32 = 4;

/// A decoded document and the payload version it was stored at.
#[derive(Debug)]
pub struct Decoded {
    pub doc: VaultDoc,
    /// The version on disk: [`crate::VAULT_FORMAT_VERSION`] for a current file,
    /// 4 for a file the transition converted.
    pub from_version: u32,
    /// What the transition from a v4 file hands on besides the document;
    /// `None` for a current file.
    pub carry: Option<legacy_v4::Carry>,
}

/// Decode raw payload bytes of a current (container 2) file.
pub fn decode(bytes: &[u8], fmt: SnapshotFormat) -> Result<VaultDoc> {
    decode_versioned(bytes, fmt).map(|decoded| decoded.doc)
}

/// [`decode`], also telling which version the payload was stored at. For a
/// payload this build wrote itself (container 2: a snapshot, a store entry);
/// a payload read out of a file goes through [`decode_file`].
pub(crate) fn decode_versioned(bytes: &[u8], fmt: SnapshotFormat) -> Result<Decoded> {
    decode_in_container(crate::VAULT_CONTAINER_VERSION, bytes, fmt)
}

/// Decode `plaintext`, the opened payload of the vault file `file`, by the
/// container version in the file's header (see [`decode_in_container`]).
pub fn decode_file(file: &[u8], plaintext: &[u8], fmt: SnapshotFormat) -> Result<Decoded> {
    let (header, _) = crypto::parse_file(file)?;
    decode_in_container(header.version, plaintext, fmt)
}

/// Decode a payload from a file of `container` version, by the table: container
/// 2 holds payload 1; container 1 holds the old payloads, of which only v4 is
/// still read (converted by [`legacy_v4::transition`], with a [`Carry`]) and
/// v1-v3 are refused (the positional binary encoding never gets here, see [`crate::crypto`]) with [`Error::LegacyVault`]; every other pair
/// is [`Error::Unsupported`].
///
/// A payload that claims a readable version but does not fit its schema is
/// [`Error::IncompatibleVault`].
///
/// [`Carry`]: legacy_v4::Carry
pub fn decode_in_container(container: u32, bytes: &[u8], fmt: SnapshotFormat) -> Result<Decoded> {
    let version = stored_version(bytes, fmt);
    match (container, version) {
        (crate::VAULT_CONTAINER_VERSION, Some(crate::VAULT_FORMAT_VERSION)) => {
            let doc = parse::<VaultDoc>(bytes, fmt).map_err(unfit)?;
            Ok(Decoded {
                doc,
                from_version: crate::VAULT_FORMAT_VERSION,
                carry: None,
            })
        }
        (crate::VAULT_LEGACY_CONTAINER_VERSION, Some(TRANSITION_FROM_VERSION)) => {
            let old = parse::<legacy_v4::VaultDocV4>(bytes, fmt).map_err(unfit)?;
            let (mut doc, carry) = legacy_v4::transition(old);
            doc.version = crate::VAULT_FORMAT_VERSION;
            Ok(Decoded {
                doc,
                from_version: TRANSITION_FROM_VERSION,
                carry: Some(carry),
            })
        }
        (crate::VAULT_LEGACY_CONTAINER_VERSION, Some(1..=3)) => {
            Err(legacy_refusal(version, encoding_name(fmt)))
        }
        (_, None) => {
            // Not even a version to read: report why the payload does not parse.
            parse::<VaultDoc>(bytes, fmt)?;
            Err(Error::Unsupported)
        }
        _ => Err(Error::Unsupported),
    }
}

/// A payload claiming a readable version but not fitting its schema.
fn unfit(err: Error) -> Error {
    Error::IncompatibleVault(err.to_string())
}

/// How a refusal names the encoding of a payload.
fn encoding_name(fmt: SnapshotFormat) -> &'static str {
    match fmt {
        SnapshotFormat::Json => "json",
        SnapshotFormat::Pack => "MessagePack",
    }
}

/// Just the `version` of a payload.
#[derive(serde::Deserialize)]
struct VersionProbe {
    version: u32,
}

/// The `version` a payload claims, read without decoding the rest of it.
fn stored_version(bytes: &[u8], fmt: SnapshotFormat) -> Option<u32> {
    match fmt {
        SnapshotFormat::Json => serde_json::from_slice::<serde_json::Value>(bytes)
            .ok()?
            .get("version")?
            .as_u64()
            .and_then(|version| u32::try_from(version).ok()),
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
        let bytes = crate::vault_io::local::read_all(path)?;
        let (plaintext, fmt, key) = crypto::open_bytes_keyed(&bytes, password.as_bytes())?;
        let decoded = decode_file(&bytes, &plaintext, fmt)?;
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
        crate::vault_io::local::atomic_write(path, &file)?;
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
        let decoded = decode_file(written, &plaintext, fmt)?;
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
    /// the first, untouched file survives repeated opens. The copy is written
    /// atomically (`copy_atomically`: tmp + rename), so a new short copy cannot
    /// appear; a leftover that does not parse (a short copy an older build
    /// left) is made again. The container header holds no ciphertext length and
    /// the AEAD tag needs the password, so wholeness is judged by the header
    /// alone, never by comparing with the source: that file may have grown.
    pub fn backup_version(path: &Path, version: u32) -> Result<PathBuf> {
        let mut name = path.file_name().unwrap_or_default().to_os_string();
        name.push(format!(".v{version}.bak"));
        let target = path.with_file_name(name);
        let kept = std::fs::read(&target).is_ok_and(|bytes| crypto::parse_file(&bytes).is_ok());
        if !kept {
            copy_atomically(path, &target)?;
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
        copy_atomically(path, &target)?;
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
        crate::vault_io::local::atomic_write(path, &file)?;
        Ok(file)
    }
}

/// Magic + version of the file for quick format probing (UI detection).
pub fn probe(path: &Path) -> Result<SnapshotFormat> {
    let bytes = crate::vault_io::local::read_all(path)?;
    let (header, _) = crypto::parse_file(&bytes)?;
    header.payload_format().ok_or(Error::Unsupported)
}

/// Copy `from` to `to` through the atomic write: `to` is either absent, the
/// old file or the whole new one, never a short one.
fn copy_atomically(from: &Path, to: &Path) -> Result<()> {
    let bytes = std::fs::read(from).map_err(|err| Error::Io(err.to_string()))?;
    crate::vault_io::local::atomic_write(to, &bytes)
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
    fn a_whole_version_copy_is_kept() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v.wallet");
        VaultFile::create(&path, "pw").unwrap();
        let first = VaultFile::backup_version(&path, 3).unwrap();
        let kept = std::fs::read(&first).unwrap();
        VaultFile::save(&path, "pw", &sample_doc(), SnapshotFormat::Json).unwrap();
        VaultFile::backup_version(&path, 3).unwrap();
        assert_eq!(std::fs::read(&first).unwrap(), kept);
    }

    #[test]
    fn a_partial_pre_migration_copy_is_remade() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v.wallet");
        VaultFile::create(&path, "pw").unwrap();
        let target = dir.path().join("v.wallet.v3.bak");
        std::fs::write(&target, b"short").unwrap();
        VaultFile::backup_version(&path, 3).unwrap();
        assert_eq!(
            std::fs::read(&target).unwrap(),
            std::fs::read(&path).unwrap()
        );
        let leftovers = std::fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp."))
            .count();
        assert_eq!(leftovers, 0);
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

    /// A payload from any other version must be rejected (Unsupported).
    #[test]
    fn future_payload_version_is_rejected() {
        let doc = VaultDoc {
            version: VAULT_FORMAT_VERSION + 1,
            ..sample_doc()
        };
        let fmt = SnapshotFormat::Pack;
        let err =
            decode(&encode(&doc, fmt).unwrap(), fmt).expect_err("newer payload must be rejected");
        assert!(matches!(err, Error::Unsupported), "got {err:?}");

        let mut json: Json = serde_json::to_value(&doc).unwrap();
        json["version"] = Json::from(VAULT_FORMAT_VERSION + 1);
        let err = decode(&serde_json::to_vec(&json).unwrap(), SnapshotFormat::Json)
            .expect_err("newer payload must be rejected");
        assert!(matches!(err, Error::Unsupported), "got {err:?}");
    }

    /// Plan D11: every persisted type of phases 2–7 exists now. A document
    /// with every collection filled must survive both encodings unchanged —
    /// the round trip catches a type serde can write but not read back.
    #[test]
    fn a_full_document_round_trips_in_every_format() {
        let doc = crate::model::fixtures::full_doc();
        for fmt in [SnapshotFormat::Json, SnapshotFormat::Pack] {
            let out = decode(&encode(&doc, fmt).unwrap(), fmt).unwrap();
            assert_eq!(out, doc, "{fmt:?}");
        }
    }

    /// The mark of a raise cut short (`Settings::crypto_raise_pending`) is
    /// additive: it survives a save and an open in both file formats, and a
    /// document written before it existed reads as "no raise pending".
    #[test]
    fn the_raise_mark_survives_saving_and_opening_in_both_formats() {
        for fmt in [SnapshotFormat::Json, SnapshotFormat::Pack] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("test.wallet");
            let mut doc = sample_doc();
            doc.settings.crypto_raise_pending = true;
            VaultFile::create(&path, "pw").unwrap();
            VaultFile::save(&path, "pw", &doc, fmt).unwrap();

            let (out, read) = VaultFile::open(&path, "pw").unwrap();
            assert_eq!(read, fmt);
            assert!(out.settings.crypto_raise_pending, "{fmt:?}");

            // A file from a build that did not know the field.
            let mut json = serde_json::to_value(&doc).unwrap();
            json["settings"]
                .as_object_mut()
                .unwrap()
                .remove("crypto_raise_pending")
                .expect("the field is serialized");
            let old = encode_as(&json, fmt);
            let older = decode(&old, fmt).unwrap();
            assert!(!older.settings.crypto_raise_pending, "{fmt:?}");
        }
    }

    /// A raw JSON document as the payload bytes of a format that carries field names.
    fn encode_as(payload: &impl serde::Serialize, fmt: SnapshotFormat) -> Vec<u8> {
        match fmt {
            SnapshotFormat::Pack => rmp_serde::to_vec_named(payload).expect("a packed payload"),
            _ => serde_json::to_vec(payload).expect("a json payload"),
        }
    }

    // -- the reader table (D2) -----------------------------------------------

    /// The payload bytes of a document of that `version`, as the format holds
    /// it: JSON and MessagePack carry field names, so the live document with
    /// another `version` stands in for an old one (the table looks at the
    /// version and the container, not at the rest).
    fn payload_of_version(version: u32, fmt: SnapshotFormat) -> Vec<u8> {
        let mut json: Json = serde_json::to_value(sample_doc()).unwrap();
        json["version"] = Json::from(version);
        match fmt {
            SnapshotFormat::Json => serde_json::to_vec(&json).unwrap(),
            SnapshotFormat::Pack => rmp_serde::to_vec_named(&json).unwrap(),
        }
    }

    /// The payload of the committed v4 fixture of that format, as it is inside
    /// its container 1.
    fn v4_payload(fmt: SnapshotFormat) -> Vec<u8> {
        let (_, _, bytes) = V4_FIXTURES.iter().find(|(_, own, _)| *own == fmt).unwrap();
        let (plaintext, read) = crypto::open_bytes(bytes, V4_PASSWORD.as_bytes()).unwrap();
        assert_eq!(read, fmt);
        plaintext
    }

    fn assert_refused_as_legacy(err: Error, format_text: &str) {
        let Error::LegacyVault(message) = &err else {
            panic!("expected LegacyVault, got {err:?}");
        };
        assert_eq!(err.code(), "UNSUPPORTED");
        assert!(message.contains(format_text), "{message}");
        assert!(message.contains("no longer reads"), "{message}");
        assert!(message.contains(LAST_LEGACY_BUILD), "{message}");
    }

    /// Every row of the table of D2, one by one.
    #[test]
    fn the_reader_table_holds_row_by_row() {
        let json = SnapshotFormat::Json;
        let new_container = crate::VAULT_CONTAINER_VERSION;
        let old_container = crate::VAULT_LEGACY_CONTAINER_VERSION;
        assert_eq!((old_container, new_container), (1, 2));

        // Container 2, payload 1: the current format, nothing to convert.
        let current = decode_in_container(2, &payload_of_version(1, json), json).unwrap();
        assert_eq!(current.from_version, 1);
        assert_eq!(current.doc.version, VAULT_FORMAT_VERSION);
        assert!(current.carry.is_none());

        // Container 1, payload 4 (Json or Pack): the transition.
        for fmt in [SnapshotFormat::Json, SnapshotFormat::Pack] {
            let moved = decode_in_container(1, &v4_payload(fmt), fmt).unwrap();
            assert_eq!(moved.from_version, 4, "{fmt:?}");
            assert_eq!(moved.doc.version, VAULT_FORMAT_VERSION, "{fmt:?}");
            assert!(moved.carry.is_some(), "{fmt:?}");
        }

        // Container 1, payload 1, 2, 3 (Json or Pack): refused as legacy.
        for fmt in [SnapshotFormat::Json, SnapshotFormat::Pack] {
            for version in 1..=3 {
                let err =
                    decode_in_container(1, &payload_of_version(version, fmt), fmt).unwrap_err();
                assert_refused_as_legacy(err, &format!("format {version}"));
            }
        }

        // Container 2 with anything but payload 1, container 1 with 5 or more or 0,
        // any other container: Unsupported.
        for (container, version) in [
            (2, 0),
            (2, 2),
            (2, 4),
            (2, 5),
            (1, 0),
            (1, 5),
            (1, 99),
            (0, 1),
            (3, 1),
            (3, 4),
            (99, 1),
        ] {
            let err = decode_in_container(container, &payload_of_version(version, json), json)
                .unwrap_err();
            assert!(
                matches!(err, Error::Unsupported),
                "container {container}, payload {version}: {err:?}"
            );
        }
    }

    /// An old-v1 payload (the counter before the reset) in container 1 is not read as the new
    /// v1, which is the whole point of the container version.
    #[test]
    fn an_old_v1_payload_is_refused() {
        let doc = crate::model::fixtures::full_doc();
        for fmt in [SnapshotFormat::Json, SnapshotFormat::Pack] {
            let mut json: Json = serde_json::to_value(&doc).unwrap();
            json["version"] = Json::from(1);
            let old = match fmt {
                SnapshotFormat::Pack => rmp_serde::to_vec_named(&json).unwrap(),
                _ => serde_json::to_vec(&json).unwrap(),
            };
            let err = decode_in_container(1, &old, fmt).unwrap_err();
            assert_refused_as_legacy(err, "format 1");
            // The same bytes in container 2 are the new v1.
            assert_eq!(decode_in_container(2, &old, fmt).unwrap().from_version, 1);
        }
    }

    #[test]
    fn an_old_v2_and_v3_payload_is_refused() {
        for fmt in [SnapshotFormat::Json, SnapshotFormat::Pack] {
            for version in [2, 3] {
                let err =
                    decode_in_container(1, &payload_of_version(version, fmt), fmt).unwrap_err();
                assert_refused_as_legacy(err, &format!("format {version}"));
            }
        }
    }

    /// A vault file of container 1 holding an old payload: opening it names the
    /// problem and leaves the file as it was.
    #[test]
    fn an_old_vault_file_is_refused_on_open_and_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        for (version, fmt) in [
            (1, SnapshotFormat::Json),
            (2, SnapshotFormat::Pack),
            (3, SnapshotFormat::Json),
        ] {
            let path = dir.path().join(format!("v{version}-{fmt:?}.wallet"));
            let header = old_build_header(1, fmt);
            let file = crypto::seal(&header, b"pw", &payload_of_version(version, fmt)).unwrap();
            std::fs::write(&path, &file).unwrap();

            let err = VaultFile::open(&path, "pw").unwrap_err();
            assert!(matches!(err, Error::LegacyVault(_)), "{err:?}");
            assert_eq!(std::fs::read(&path).unwrap(), file);
        }
    }

    /// An old positional-binary file (payload_fmt 1) is refused as legacy and
    /// left alone, whatever the container.
    #[test]
    fn a_file_in_the_old_binary_encoding_is_refused_on_open() {
        let dir = tempfile::tempdir().unwrap();
        for container in [1, 2] {
            let path = dir.path().join(format!("c{container}.wallet"));
            let mut header = old_build_header(container, SnapshotFormat::Json);
            header.payload_fmt = crypto::header::VaultHeader::FMT_BIN;
            let file = crypto::seal(&header, b"pw", b"whatever").unwrap();
            std::fs::write(&path, &file).unwrap();

            let err = VaultFile::open(&path, "pw").unwrap_err();
            assert_refused_as_legacy(err, "positional binary");
            assert_eq!(std::fs::read(&path).unwrap(), file);
        }
    }

    /// A file written now is container 2, payload 1, in both formats.
    #[test]
    fn a_new_file_is_container_2_payload_1() {
        let dir = tempfile::tempdir().unwrap();
        for fmt in [SnapshotFormat::Json, SnapshotFormat::Pack] {
            let path = dir.path().join(format!("{fmt:?}.wallet"));
            VaultFile::create(&path, "pw").unwrap();
            VaultFile::save(&path, "pw", &sample_doc(), fmt).unwrap();
            let bytes = std::fs::read(&path).unwrap();
            let (header, _) = crypto::parse_file(&bytes).unwrap();
            assert_eq!(header.version, 2);
            let (decoded, _) = VaultFile::open_versioned(&path, "pw").unwrap();
            assert_eq!(decoded.from_version, 1);
            assert_eq!(decoded.doc.version, 1);
            assert!(decoded.carry.is_none());
        }
        let plain = dir.path().join("plain.wallet");
        let doc = sample_doc();
        let file = crypto::seal_plain(
            SnapshotFormat::Json,
            &encode(&doc, SnapshotFormat::Json).unwrap(),
        );
        std::fs::write(&plain, file).unwrap();
        let (header, _) = crypto::parse_file(&std::fs::read(&plain).unwrap()).unwrap();
        assert_eq!(header.version, 2);
    }

    /// v2 carries field names: a field missing from an older v2 file takes its
    /// default instead of breaking the read.
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
    }

    #[test]
    fn corrupt_payload_is_an_error_not_a_panic() {
        let junk = vec![0xffu8; 32];
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
    /// nothing here follows the current constants. `container` is the container
    /// version written into it.
    fn old_build_header(container: u32, fmt: SnapshotFormat) -> crypto::header::VaultHeader {
        crypto::header::VaultHeader {
            version: container,
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
        for fmt in [SnapshotFormat::Json, SnapshotFormat::Pack] {
            let path = dir.path().join(format!("{fmt:?}.wallet"));
            let header = old_build_header(2, fmt);
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
            assert_eq!(level.unwrap(), crate::crypto::level::SecurityLevel::Crypto);
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

    // -- the committed v4 fixtures (vault reset) -----------------------------

    use super::fixtures_gen::{V4_PASSWORD, V4_VERSION};

    const V4_FIXTURES: [(&str, SnapshotFormat, &[u8]); 2] = [
        (
            "v4.json.wallet",
            SnapshotFormat::Json,
            include_bytes!("fixtures/v4.json.wallet"),
        ),
        (
            "v4.pack.wallet",
            SnapshotFormat::Pack,
            include_bytes!("fixtures/v4.pack.wallet"),
        ),
    ];

    /// The document the fixtures hold, as written by the generator.
    fn v4_expected() -> Json {
        serde_json::from_str(include_str!("fixtures/v4.expected.json")).unwrap()
    }

    /// Opens the committed fixture with the password written in the generator.
    fn open_v4_fixture(name: &str, bytes: &[u8]) -> (Decoded, SnapshotFormat) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        VaultFile::open_versioned(&path, V4_PASSWORD).unwrap()
    }

    #[test]
    fn v4_fixture_opens_in_both_formats_and_equals_the_expected_document() {
        let expected = v4_expected();
        for (name, fmt, bytes) in V4_FIXTURES {
            let (decoded, read) = open_v4_fixture(name, bytes);
            assert_eq!(read, fmt, "{name}");
            assert_eq!(decoded.from_version, V4_VERSION, "{name}");
            assert!(decoded.carry.is_some(), "{name}");
            // The transition renumbers the document; its content is the fixture's.
            let mut transitioned = serde_json::to_value(&decoded.doc).unwrap();
            assert_eq!(transitioned["version"], VAULT_FORMAT_VERSION);
            transitioned["version"] = expected["version"].clone();
            // The graphs are brought to the live version by the transition, so the fields
            // that hold them differ by design.
            let graph_fields = ["workflows", "workflow_waits", "workflow_queue"];
            // `settings.crypto` is dropped: whether the vault holds crypto is derived.
            let mut expected_settings = expected["settings"].clone();
            expected_settings.as_object_mut().unwrap().remove("crypto");
            assert_eq!(
                transitioned["settings"], expected_settings,
                "{name}: settings"
            );
            for (field, value) in expected.as_object().unwrap() {
                if graph_fields.contains(&field.as_str()) || field == "settings" {
                    continue;
                }
                assert_eq!(&transitioned[field], value, "{name}: {field}");
            }
        }
    }

    #[test]
    fn v4_fixture_is_refused_with_another_password() {
        for (name, _, bytes) in V4_FIXTURES {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join(name);
            std::fs::write(&path, bytes).unwrap();
            assert!(matches!(
                VaultFile::open(&path, "wrong"),
                Err(Error::Secret)
            ));
        }
    }

    #[test]
    fn v4_fixture_fills_every_collection() {
        let doc = v4_expected();
        let fields = doc.as_object().unwrap();
        for (field, value) in fields {
            let empty = match value {
                Json::Array(items) => items.is_empty(),
                Json::Object(entries) => entries.is_empty(),
                Json::Null => true,
                _ => false,
            };
            assert!(!empty, "the fixture leaves `{field}` empty");
        }
        for (field, value) in fields["sync"].as_object().unwrap() {
            let must_be_filled = [
                "stamps",
                "tombstones",
                "tombstones_seen",
                "peers",
                "batches",
            ];
            assert!(
                !must_be_filled.contains(&field.as_str())
                    || !value.as_object().map_or_else(
                        || value.as_array().is_some_and(Vec::is_empty),
                        |entries| entries.is_empty()
                    ),
                "the fixture's sync leaves `{field}` empty"
            );
        }
    }

    #[test]
    fn v4_fixture_carries_what_the_reset_has_to_transition() {
        let doc = v4_expected();
        assert_eq!(doc["settings"]["crypto"], true);
        assert!(!doc["pending_imports"].as_array().unwrap().is_empty());
        assert!(!doc["sync"]["tombstones"].as_object().unwrap().is_empty());
        let graph: Json =
            serde_json::from_str(doc["workflows"][0]["graph"].as_str().unwrap()).unwrap();
        assert_eq!(graph["version"], 2);
        let (decoded, _) = open_v4_fixture(V4_FIXTURES[0].0, V4_FIXTURES[0].2);
        assert!(decoded.carry.is_some(), "a v4 file hands on a carry");
        let bybit = &decoded.doc.exchanges.bybit;
        assert!(!bybit.is_empty());
        assert!(bybit.iter().all(|key| !key.api_secret.is_empty()));
    }
}
