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
//! the schema of that version and migrated up step by step (`v1` → current).
//! A version from the future is refused. To change the schema in a way an
//! older file cannot be read with: freeze the current shape in a `vN` module,
//! bump [`VAULT_FORMAT_VERSION`], add the step and its tests.

use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;

use crate::VAULT_FORMAT_VERSION;
use crate::crypto;
use crate::error::{Error, Result};
use crate::model::vault::VaultDoc;

pub mod v1;

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
    /// The version on disk; below [`VAULT_FORMAT_VERSION`] when migrated.
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
    let doc = match version {
        VAULT_FORMAT_VERSION => parse::<VaultDoc>(bytes, fmt)
            .map_err(|err| Error::IncompatibleVault(err.to_string()))?,
        1 => VaultDoc::from(
            parse::<v1::VaultDocV1>(bytes, fmt)
                .map_err(|err| Error::IncompatibleVault(err.to_string()))?,
        ),
        _ => return Err(Error::Unsupported),
    };
    if doc.version != VAULT_FORMAT_VERSION {
        return Err(Error::Unsupported);
    }
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

impl VaultFile {
    /// Create a brand new encrypted vault at `path` (refuses to overwrite).
    pub fn create(path: &Path, password: &str) -> Result<PathBuf> {
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
        let bytes = crypto::read_all(path)?;
        let (plaintext, fmt) = crypto::open_bytes(&bytes, password.as_bytes())?;
        let decoded = decode_versioned(&plaintext, fmt)?;
        Ok((decoded, fmt))
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

    /// Re-encrypt & atomically persist the whole document.
    pub fn save(path: &Path, password: &str, doc: &VaultDoc, fmt: SnapshotFormat) -> Result<()> {
        Self::save_sealed(path, password, doc, fmt).map(|_| ())
    }

    /// [`Self::save`], handing back the exact bytes written (for a mirror copy).
    pub fn save_sealed(
        path: &Path,
        password: &str,
        doc: &VaultDoc,
        fmt: SnapshotFormat,
    ) -> Result<Vec<u8>> {
        Self::save_impl(path, password, doc, fmt, /*overwrite=*/ true)
    }

    /// Export the decrypted snapshot as readable JSON (for debugging/backup).
    pub fn export_json(doc: &VaultDoc) -> Result<String> {
        serde_json::to_string_pretty(doc).map_err(|err| Error::Serialize(err.to_string()))
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
        expected.notification_log.clear();
        expected.sync = crate::sync::SyncState::default();
        expected.settings.notifications = crate::model::notify::NotificationSettings::default();
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
}
