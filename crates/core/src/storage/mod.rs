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
use crate::model::vault::VaultDoc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotFormat {
    Json,
    Bin,
}

impl SnapshotFormat {
    pub fn from_u8(raw: u8) -> Option<Self> {
        match raw {
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
            serde_json::to_vec_pretty(doc).map_err(|err| Error::Serialize(err.to_string()))
        }
        SnapshotFormat::Bin => bincode::serde::encode_to_vec(doc, bincode::config::standard())
            .map_err(|err| Error::Serialize(err.to_string())),
    }
}

/// Decode raw payload bytes according to the format written in the file.
///
/// Only accepts payloads with `version == VAULT_FORMAT_VERSION` (currently 1).
/// No backward compatibility — files written at any other version are rejected
/// as [`Error::Unsupported`].
///
/// Do not "fix" that strictness by accepting a second version or by bumping
/// [`VAULT_FORMAT_VERSION`]: both require the user's explicit permission. See
/// the constant's documentation.
pub fn decode(bytes: &[u8], fmt: SnapshotFormat) -> Result<VaultDoc> {
    let decoded = match fmt {
        SnapshotFormat::Json => serde_json::from_slice::<VaultDoc>(bytes)
            .map_err(|err| Error::Deserialize(err.to_string())),
        SnapshotFormat::Bin => decode_bin(bytes),
    };
    // A file that says it is the current version but does not fit the current
    // schema was written before the schema break (plan §4.10). There is no
    // converter, so say so instead of reporting a parse error.
    let doc = match decoded {
        Ok(doc) => doc,
        Err(err) if stored_version(bytes, fmt) == Some(VAULT_FORMAT_VERSION) => {
            return Err(Error::IncompatibleVault(err.to_string()));
        }
        Err(err) => return Err(err),
    };
    if doc.version != VAULT_FORMAT_VERSION {
        return Err(Error::Unsupported);
    }
    Ok(doc)
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
    }
}

/// Decode a bincode payload directly as the current VaultDoc schema.
fn decode_bin(bytes: &[u8]) -> Result<VaultDoc> {
    let (doc, consumed) =
        bincode::serde::decode_from_slice::<VaultDoc, _>(bytes, bincode::config::standard())
            .map_err(|err| Error::Deserialize(err.to_string()))?;
    if consumed != bytes.len() {
        return Err(Error::Deserialize(
            "trailing bytes in bincode payload".into(),
        ));
    }
    Ok(doc)
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

/// Conventional backup slot for a vault file: `<name>.bak` appended to the
/// full file name, so `vault.wallet` becomes `vault.wallet.bak`.
pub fn backup_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".bak");
    path.with_file_name(name)
}

/// Keeps a vault open in one app instance at a time.
///
/// Every save rewrites the whole file, so two windows editing the same vault
/// would silently undo each other. The lock is `<vault>.lock` holding the
/// owner's PID; a lock whose process is gone (a crash) is taken over. The
/// owning process may open the vault again (re-unlock, tests).
#[derive(Debug)]
pub struct VaultLock {
    path: PathBuf,
    owned: bool,
}

impl VaultLock {
    pub fn acquire(vault: &Path) -> Result<Self> {
        let mut name = vault.file_name().unwrap_or_default().to_os_string();
        name.push(".lock");
        let path = vault.with_file_name(name);
        let own = std::process::id();
        if let Some(holder) = std::fs::read_to_string(&path)
            .ok()
            .and_then(|raw| raw.trim().parse::<u32>().ok())
        {
            if holder == own {
                return Ok(Self { path, owned: false });
            }
            if process_alive(holder) {
                return Err(Error::VaultBusy(vault.display().to_string()));
            }
        }
        std::fs::write(&path, own.to_string()).map_err(|err| Error::Io(err.to_string()))?;
        Ok(Self { path, owned: true })
    }
}

impl Drop for VaultLock {
    fn drop(&mut self) {
        if self.owned {
            std::fs::remove_file(&self.path).ok();
        }
    }
}

#[cfg(target_os = "linux")]
fn process_alive(pid: u32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

/// Without `/proc` a leftover lock cannot be told from a live one; it is
/// ignored rather than locking the user out after a crash.
#[cfg(not(target_os = "linux"))]
fn process_alive(_pid: u32) -> bool {
    false
}

/// File extension of a vault on disk.
pub const VAULT_EXT: &str = "wallet";

/// Append `.wallet` unless the name already ends with it (any case).
///
/// Native save dialogs on Linux return exactly what was typed, so `my-money`
/// would otherwise become an extension-less file the open dialog's filter hides.
pub fn ensure_wallet_ext(path: &Path) -> PathBuf {
    let has_ext = path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case(VAULT_EXT));
    if has_ext {
        return path.to_path_buf();
    }
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".");
    name.push(VAULT_EXT);
    path.with_file_name(name)
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
        for fmt in [SnapshotFormat::Json, SnapshotFormat::Bin] {
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
        for fmt in [SnapshotFormat::Json, SnapshotFormat::Bin] {
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

    /// Plan D11: every persisted type of phases 2–7 exists now. A document
    /// with every collection filled must survive both encodings unchanged —
    /// the bincode half is what catches a type serde can write but not read
    /// back (internal tags, `flatten`, `Value`).
    #[test]
    fn a_full_document_round_trips_in_both_formats() {
        let doc = crate::model::fixtures::full_doc();
        for fmt in [SnapshotFormat::Json, SnapshotFormat::Bin] {
            let out = decode(&encode(&doc, fmt).unwrap(), fmt).unwrap();
            assert_eq!(out, doc, "{fmt:?}");
        }
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

    #[test]
    fn ensure_wallet_ext_appends_only_when_missing() {
        let cases = [
            ("/tmp/a", "/tmp/a.wallet"),
            ("/tmp/a.wallet", "/tmp/a.wallet"),
            ("/tmp/a.WALLET", "/tmp/a.WALLET"),
            ("/tmp/a.json", "/tmp/a.json.wallet"),
            ("/tmp/my.money", "/tmp/my.money.wallet"),
        ];
        for (input, expected) in cases {
            assert_eq!(ensure_wallet_ext(Path::new(input)), PathBuf::from(expected));
        }
    }

    #[test]
    fn a_vault_lock_keeps_other_processes_out() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().join("v.wallet");
        let lock_file = dir.path().join("v.wallet.lock");
        {
            let _held = VaultLock::acquire(&vault).unwrap();
            assert!(lock_file.exists());
            // The same process may open it again.
            assert!(VaultLock::acquire(&vault).is_ok());
            assert!(
                lock_file.exists(),
                "a re-entrant lock does not release the first"
            );
        }
        assert!(!lock_file.exists());
        // PID 1 is always alive on Linux: another process holds it.
        std::fs::write(&lock_file, "1").unwrap();
        if cfg!(target_os = "linux") {
            assert!(matches!(
                VaultLock::acquire(&vault),
                Err(Error::VaultBusy(_))
            ));
        }
        // A dead holder is taken over.
        std::fs::write(&lock_file, "4294967000").unwrap();
        assert!(VaultLock::acquire(&vault).is_ok());
    }
}
