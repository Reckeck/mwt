//! Storage layer: serializing the [`VaultDoc`] snapshot and persisting the
//! encrypted container.
//!
//! Snapshot encoding depends on the build profile:
//! - `debug` build  -> pretty JSON (readable, easy to inspect & diff)
//! - `release` build -> compact bincode binary chunks
//!
//! Both encodings live inside the SAME encrypted container (see `crypto`),
//! and a `payload_fmt` byte in the header makes any file self-describing.

use std::path::{Path, PathBuf};

use crate::VAULT_FORMAT_VERSION;
use crate::crypto;
use crate::error::{Error, Result};
use crate::model::vault::{VaultDoc, VaultDocV1};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotFormat {
    Json,
    Bin,
}

impl SnapshotFormat {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            crypto::header::VaultHeader::FMT_JSON => Some(SnapshotFormat::Json),
            crypto::header::VaultHeader::FMT_BIN => Some(SnapshotFormat::Bin),
            _ => None,
        }
    }

    pub fn as_u8(self) -> u8 {
        match self {
            SnapshotFormat::Json => crypto::header::VaultHeader::FMT_JSON,
            SnapshotFormat::Bin => crypto::header::VaultHeader::FMT_BIN,
        }
    }

    /// The concrete format for this build profile (dev vs prod).
    pub fn for_profile() -> Self {
        if cfg!(debug_assertions) {
            SnapshotFormat::Json
        } else {
            SnapshotFormat::Bin
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SnapshotFormat::Json => "json",
            SnapshotFormat::Bin => "bin",
        }
    }
}

/// Encode a snapshot into the raw payload bytes for a given format.
pub fn encode(doc: &VaultDoc, fmt: SnapshotFormat) -> Result<Vec<u8>> {
    match fmt {
        SnapshotFormat::Json => {
            serde_json::to_vec_pretty(doc).map_err(|e| Error::Serialize(e.to_string()))
        }
        SnapshotFormat::Bin => bincode::serde::encode_to_vec(doc, bincode::config::standard())
            .map_err(|e| Error::Serialize(e.to_string())),
    }
}

/// Decode raw payload bytes according to the format written in the file.
pub fn decode(bytes: &[u8], fmt: SnapshotFormat) -> Result<VaultDoc> {
    let mut doc = match fmt {
        // `bybit` is `#[serde(default)]`, so a v1 JSON payload already parses
        // into the current struct without any restructuring.
        SnapshotFormat::Json => serde_json::from_slice::<VaultDoc>(bytes)
            .map_err(|e| Error::Deserialize(e.to_string()))?,
        SnapshotFormat::Bin => decode_bin(bytes)?,
    };
    if doc.version > VAULT_FORMAT_VERSION {
        return Err(Error::Unsupported);
    }
    if doc.version < VAULT_FORMAT_VERSION {
        // Nothing to restructure (v2 only appended an optional field, which
        // both encodings already defaulted). Stamp the current version so the
        // next save writes a payload this build fully understands.
        doc.version = VAULT_FORMAT_VERSION;
    }
    Ok(doc)
}

/// Decode a bincode payload, transparently migrating v1.
///
/// bincode is positional and carries no field names, so there is no way to ask
/// "which version is this?" up front. Instead we decode the frozen v1 shape
/// first and use the leftover byte count as the discriminator:
///
/// * leftover `0` -> the payload ended exactly at the v1 boundary -> v1.
/// * leftover `> 0` -> the appended v2 `bybit` field is still there -> v2.
fn decode_bin(bytes: &[u8]) -> Result<VaultDoc> {
    let as_v1 =
        bincode::serde::decode_from_slice::<VaultDocV1, _>(bytes, bincode::config::standard());
    match as_v1 {
        // `decode_from_slice` reports bytes *read*, so a v1 payload is one that
        // ends exactly at the v1 boundary with nothing left over.
        Ok((v1, n)) if n == bytes.len() => return Ok(v1.upgrade()),
        _ => {}
    }
    bincode::serde::decode_from_slice::<VaultDoc, _>(bytes, bincode::config::standard())
        .map(|(doc, _)| doc)
        .map_err(|e| Error::Deserialize(e.to_string()))
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
        Self::save_impl(path, password, &doc, fmt, /*overwrite=*/ false)
    }

    /// Open an existing vault, decrypt and decode it.
    pub fn open(path: &Path, password: &str) -> Result<(VaultDoc, SnapshotFormat)> {
        let bytes = crypto::read_all(path)?;
        let (plaintext, fmt) = crypto::open_bytes(&bytes, password.as_bytes())?;
        let doc = decode(&plaintext, fmt)?;
        Ok((doc, fmt))
    }

    /// Re-encrypt & atomically persist the whole document.
    pub fn save(path: &Path, password: &str, doc: &VaultDoc, fmt: SnapshotFormat) -> Result<()> {
        Self::save_impl(path, password, doc, fmt, /*overwrite=*/ true).map(|_| ())
    }

    /// Export the decrypted snapshot as readable JSON (for debugging/backup).
    pub fn export_json(doc: &VaultDoc) -> Result<String> {
        serde_json::to_string_pretty(doc).map_err(|e| Error::Serialize(e.to_string()))
    }

    fn save_impl(
        path: &Path,
        password: &str,
        doc: &VaultDoc,
        fmt: SnapshotFormat,
        overwrite: bool,
    ) -> Result<PathBuf> {
        if path.exists() && !overwrite {
            return Err(Error::AlreadyExists(path.display().to_string()));
        }
        let plaintext = encode(doc, fmt)?;
        let (header, _, _) = crypto::random_header(fmt);
        let file = crypto::seal(&header, password.as_bytes(), &plaintext)?;
        crypto::atomic_write(path, &file)?;
        Ok(path.to_path_buf())
    }
}

/// The default data directory for the app.
pub fn default_data_dir(app_name: &str) -> PathBuf {
    let dir = std::env::var("TRADE_APP_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| directories_like(app_name));
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn directories_like(app_name: &str) -> PathBuf {
    // Prefer XDG on Linux, AppData on Windows, ~/Library on macOS.
    #[cfg(target_os = "linux")]
    {
        let base = std::env::var("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
                PathBuf::from(home).join(".local/share")
            });
        base.join(app_name)
    }
    #[cfg(target_os = "windows")]
    {
        let base = std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));
        base.join(app_name)
    }
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        PathBuf::from(home)
            .join("Library/Application Support")
            .join(app_name)
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        PathBuf::from(".")
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
    use crate::model::transaction::{Transaction, TxKind};

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
        let mut t = Transaction::new(
            TxKind::Income,
            btc.clone(),
            Amount::parse("0.5").unwrap(),
            &store.id,
            "first buy",
        );
        t.price_usd = Some(Amount::parse("65000").unwrap());
        doc.stores.push(store);
        doc.holdings.push(crate::model::store::Holding {
            id: uuid::Uuid::new_v4().to_string(),
            store_id: doc.stores[0].id.clone(),
            asset: btc,
            quantity: Amount::parse("0.5").unwrap(),
            avg_cost: None,
        });
        doc.transactions.push(t);
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
        assert_eq!(out.transactions[0].comment, "first buy");
        assert_eq!(out.transactions[0].amount.to_string(), "0.5");
        assert_eq!(out.transactions[0].price_usd.unwrap().to_string(), "65000");
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
        assert_eq!(out.transactions[0].kind, TxKind::Income);
        assert_eq!(out.id, doc.id);
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
}

#[cfg(test)]
mod migration_tests {
    use super::*;
    use crate::model::vault::{ExchangeKeys, SettingsV1, VaultDocV1};
    use serde_json::Value as Json;

    /// Project a current document back onto the frozen v1 shape, which is how
    /// a real pre-v2 payload would have looked on disk.
    fn to_v1(doc: &VaultDoc) -> VaultDocV1 {
        VaultDocV1 {
            version: 1,
            id: doc.id.clone(),
            created_at: doc.created_at.clone(),
            settings: SettingsV1 {
                base_currency: doc.settings.base_currency.clone(),
                theme: doc.settings.theme,
                locale: doc.settings.locale,
                display_decimals: doc.settings.display_decimals,
            },
            stores: doc.stores.clone(),
            holdings: doc.holdings.clone(),
            transactions: doc.transactions.clone(),
            categories: doc.categories.clone(),
            price_cache: doc.price_cache.clone(),
            fx_cache: doc.fx_cache.clone(),
            sim: doc.sim.clone(),
        }
    }

    fn legacy_json(doc: &VaultDoc) -> Vec<u8> {
        let mut v = serde_json::to_value(to_v1(doc)).unwrap();
        // v1 had no `bybit` field at all.
        v.as_object_mut().unwrap().remove("bybit");
        serde_json::to_vec_pretty(&v).unwrap()
    }

    fn legacy_bin(doc: &VaultDoc) -> Vec<u8> {
        bincode::serde::encode_to_vec(to_v1(doc), bincode::config::standard()).unwrap()
    }

    fn populated() -> VaultDoc {
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
        let mut tx = crate::model::transaction::Transaction::new(
            crate::model::transaction::TxKind::Income,
            btc.clone(),
            crate::Amount::parse("0.25").unwrap(),
            &store.id,
            "legacy entry",
        );
        tx.price_usd = Some(crate::Amount::parse("65000").unwrap());
        let store_id = store.id.clone();
        VaultDoc {
            stores: vec![store],
            holdings: vec![crate::model::store::Holding {
                id: uuid::Uuid::new_v4().to_string(),
                store_id,
                asset: btc,
                quantity: crate::Amount::parse("0.25").unwrap(),
                avg_cost: None,
            }],
            transactions: vec![tx],
            ..Default::default()
        }
    }

    /// The bincode path is the dangerous one: no field names, so a naive field
    /// addition would make every existing vault undecodable.
    #[test]
    fn bin_v1_payload_migrates_without_data_loss() {
        let doc = populated();
        let out = decode(&legacy_bin(&doc), SnapshotFormat::Bin).unwrap();

        assert_eq!(out.version, VAULT_FORMAT_VERSION);
        assert_eq!(out.bybit, None);
        assert_eq!(out.id, doc.id);
        assert_eq!(out.created_at, doc.created_at);
        assert_eq!(out.settings.base_currency, doc.settings.base_currency);
        assert_eq!(out.settings.display_decimals, doc.settings.display_decimals);
        assert_eq!(out.stores.len(), 1);
        assert_eq!(out.stores[0].name, "Binance");
        assert_eq!(out.holdings[0].quantity.to_string(), "0.25");
        assert_eq!(out.transactions.len(), 1);
        assert_eq!(out.transactions[0].comment, "legacy entry");
        assert_eq!(out.transactions[0].price_usd.unwrap().to_string(), "65000");
        assert_eq!(out.categories, doc.categories);
    }

    #[test]
    fn json_v1_payload_migrates_without_data_loss() {
        let doc = populated();
        let out = decode(&legacy_json(&doc), SnapshotFormat::Json).unwrap();

        assert_eq!(out.version, VAULT_FORMAT_VERSION);
        assert_eq!(out.bybit, None);
        assert_eq!(out.stores[0].name, "Binance");
        assert_eq!(out.transactions[0].comment, "legacy entry");
    }

    /// A migrated document must survive a save/reload cycle as a normal v2 file.
    #[test]
    fn migrated_doc_reencodes_as_v2() {
        let doc = populated();
        for fmt in [SnapshotFormat::Json, SnapshotFormat::Bin] {
            let migrated = decode(&legacy_bin(&doc), fmt)
                .unwrap_or_else(|_| decode(&legacy_json(&doc), fmt).unwrap());
            let re = decode(&encode(&migrated, fmt).unwrap(), fmt).unwrap();
            assert_eq!(re.version, VAULT_FORMAT_VERSION);
            assert_eq!(re.id, doc.id);
            assert_eq!(re.transactions.len(), 1);
            assert_eq!(re.bybit, None);
        }
    }

    /// v2 payloads must NOT be mistaken for v1 (the leftover-bytes probe).
    #[test]
    fn bin_v2_payload_with_bybit_roundtrips() {
        let doc = VaultDoc {
            bybit: Some(ExchangeKeys {
                api_key: "abcdef123456".into(),
                api_secret: "s3cr3t".into(),
            }),
            ..populated()
        };
        let bytes = encode(&doc, SnapshotFormat::Bin).unwrap();
        let out = decode(&bytes, SnapshotFormat::Bin).unwrap();

        let keys = out.bybit.expect("bybit keys must survive v2 decode");
        assert_eq!(keys.api_key, "abcdef123456");
        assert_eq!(keys.api_secret, "s3cr3t");
        assert_eq!(out.transactions[0].comment, "legacy entry");
    }

    #[test]
    fn bin_v2_payload_without_bybit_is_not_read_as_v1() {
        let doc = populated();
        let out = decode(
            &encode(&doc, SnapshotFormat::Bin).unwrap(),
            SnapshotFormat::Bin,
        )
        .unwrap();
        assert_eq!(out.version, VAULT_FORMAT_VERSION);
        assert_eq!(out.bybit, None);
        assert_eq!(out.transactions.len(), 1);
    }

    /// End-to-end guard for the container/payload version split: a file whose
    /// header is v1 must still open even though the payload schema is now v2.
    #[test]
    fn v1_container_header_still_opens_v1_payload() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("legacy.wallet");
        let doc = populated();

        // Build the envelope by hand from a genuine v1 payload.
        let (header, _, _) = crypto::random_header(SnapshotFormat::Bin);
        assert_eq!(header.version, crate::VAULT_CONTAINER_VERSION);
        let sealed = crypto::seal(&header, b"pw", &legacy_bin(&doc)).unwrap();
        crypto::atomic_write(&path, &sealed).unwrap();

        let (out, fmt) = VaultFile::open(&path, "pw").unwrap();
        assert_eq!(fmt, SnapshotFormat::Bin);
        assert_eq!(out.stores[0].name, "Binance");
        assert_eq!(out.transactions[0].comment, "legacy entry");
        assert_eq!(out.bybit, None);
        assert_eq!(out.version, VAULT_FORMAT_VERSION);
    }

    /// A payload from a newer build must still be refused rather than guessed at.
    #[test]
    fn future_payload_version_is_rejected() {
        let doc = VaultDoc {
            version: VAULT_FORMAT_VERSION + 1,
            ..populated()
        };
        let err = decode(
            &encode(&doc, SnapshotFormat::Bin).unwrap(),
            SnapshotFormat::Bin,
        )
        .expect_err("newer payload must be rejected");
        assert!(matches!(err, Error::Unsupported), "got {err:?}");

        let mut json: Json = serde_json::to_value(&doc).unwrap();
        json["version"] = Json::from(VAULT_FORMAT_VERSION + 1);
        let err = decode(&serde_json::to_vec(&json).unwrap(), SnapshotFormat::Json)
            .expect_err("newer payload must be rejected");
        assert!(matches!(err, Error::Unsupported), "got {err:?}");
    }

    #[test]
    fn corrupt_payload_is_an_error_not_a_panic() {
        let junk = vec![0xffu8; 32];
        assert!(decode(&junk, SnapshotFormat::Bin).is_err());
        assert!(decode(b"{not json", SnapshotFormat::Json).is_err());
    }

    /// The vault password now also guards an exchange secret, so the secret must
    /// never be reachable through logging or debugging output.
    #[test]
    fn exchange_keys_debug_redacts_the_secret() {
        let keys = ExchangeKeys {
            api_key: "abcdef123456".into(),
            api_secret: "s3cr3t".into(),
        };
        let shown = format!("{keys:?}");
        assert!(!shown.contains("s3cr3t"), "secret leaked: {shown}");
        assert!(shown.contains("<redacted>"), "{shown}");
        assert_eq!(keys.hint(), "abcd…3456");

        // Nor through the enclosing document.
        let doc = VaultDoc {
            bybit: Some(keys),
            ..Default::default()
        };
        assert!(!format!("{doc:?}").contains("s3cr3t"));
    }

    /// `hint()` must stay non-reversible even for very short keys.
    #[test]
    fn exchange_key_hint_never_reveals_short_keys() {
        assert_eq!(
            ExchangeKeys {
                api_key: "abcd".into(),
                api_secret: "x".into()
            }
            .hint(),
            "****"
        );
    }
}
