//! Device secrets: a file next to the vault, `<vault>.secrets`, that holds
//! what must never leave this device (exchange API keys).
//!
//! - The key is derived from the open vault's key with HKDF-SHA256 and its own
//!   `info`, so the vault key itself is never reused for a second file.
//! - The cipher is the vault's one (XChaCha20-Poly1305); the file header is
//!   the AAD, so a swapped header byte does not open.
//! - The file is written only through `atomic_write`; it is never part of the
//!   vault bytes, so the mirror, sync, transfers and backups never carry it.
//! - Any failure to open (wrong key, a changed byte, a cut file) is
//!   [`Error::Secret`], never a panic.
//!
//! A new vault key re-seals the file in two steps so a crash never loses it:
//! [`stage_rekey`] writes `<vault>.secrets.next` under the new key, the caller
//! writes the vault, then [`finish_rekey`] moves `.next` over the file. A load
//! settles a crash in between: the file opens → `.next` is stale and removed;
//! it does not open but `.next` does → `.next` is promoted.

use std::path::{Path, PathBuf};

use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use zeroize::Zeroizing;

use super::{KEY_LEN, SealKey, atomic_write, open_blob, read_all, seal_blob};
use crate::error::{Error, Result};
use crate::model::vault::ExchangeKeys;

const MAGIC: &[u8; 8] = b"MWTDSEC\0";
const VERSION: u8 = 1;
const HEADER_LEN: usize = MAGIC.len() + 1;
const INFO: &[u8] = b"mwt device secrets v1";

/// One exchange key kept on this device.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceKey {
    pub keys: ExchangeKeys,
    /// The read-only check passed; `false` = it could not run (no network)
    /// and the next import runs it again.
    #[serde(default)]
    pub verified: bool,
}

/// Everything in the secrets file.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceSecrets {
    #[serde(default)]
    pub bybit: Vec<DeviceKey>,
}

impl DeviceSecrets {
    pub fn is_empty(&self) -> bool {
        self.bybit.is_empty()
    }

    pub fn bybit_for(&self, store_id: &str) -> Option<&DeviceKey> {
        self.bybit
            .iter()
            .find(|entry| entry.keys.store_id == store_id)
    }
}

/// `<vault>.secrets`.
pub fn secrets_path(vault: &Path) -> PathBuf {
    suffixed(vault, ".secrets")
}

fn staged_path(vault: &Path) -> PathBuf {
    suffixed(vault, ".secrets.next")
}

fn suffixed(vault: &Path, suffix: &str) -> PathBuf {
    let mut name = vault.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

/// HKDF-SHA256 (RFC 5869) for one 32-byte output block.
fn hkdf_sha256(salt: &[u8], ikm: &[u8], info: &[u8]) -> Zeroizing<[u8; KEY_LEN]> {
    type HmacSha = Hmac<Sha256>;
    let mut extract = <HmacSha as Mac>::new_from_slice(salt)
        .unwrap_or_else(|_| unreachable!("HMAC takes any key length"));
    extract.update(ikm);
    let mut prk = Zeroizing::new([0u8; KEY_LEN]);
    prk.copy_from_slice(&extract.finalize().into_bytes());
    let mut expand = <HmacSha as Mac>::new_from_slice(&prk[..])
        .unwrap_or_else(|_| unreachable!("HMAC takes any key length"));
    expand.update(info);
    expand.update(&[1u8]);
    let mut out = Zeroizing::new([0u8; KEY_LEN]);
    out.copy_from_slice(&expand.finalize().into_bytes());
    out
}

fn file_key(key: &SealKey) -> Zeroizing<[u8; KEY_LEN]> {
    hkdf_sha256(&key.salt, &key.key[..], INFO)
}

fn header() -> [u8; HEADER_LEN] {
    let mut out = [0u8; HEADER_LEN];
    out[..MAGIC.len()].copy_from_slice(MAGIC);
    out[MAGIC.len()] = VERSION;
    out
}

/// Seal the secrets under the file key of `key`.
pub fn seal(key: &SealKey, secrets: &DeviceSecrets) -> Result<Vec<u8>> {
    let plain = Zeroizing::new(
        serde_json::to_vec(secrets).map_err(|err| Error::Serialize(err.to_string()))?,
    );
    let head = header();
    let sealed = seal_blob(&file_key(key), &head, &plain)?;
    let mut out = Vec::with_capacity(HEADER_LEN + sealed.len());
    out.extend_from_slice(&head);
    out.extend_from_slice(&sealed);
    Ok(out)
}

/// Open what [`seal`] wrote. Anything else is [`Error::Secret`].
pub fn open(key: &SealKey, bytes: &[u8]) -> Result<DeviceSecrets> {
    let head = header();
    let body = bytes.strip_prefix(&head[..]).ok_or(Error::Secret)?;
    let plain = Zeroizing::new(open_blob(&file_key(key), &head, body).map_err(|_| Error::Secret)?);
    serde_json::from_slice(&plain).map_err(|_| Error::Secret)
}

fn read_sealed(path: &Path, key: &SealKey) -> Result<DeviceSecrets> {
    let bytes = Zeroizing::new(read_all(path)?);
    open(key, &bytes)
}

/// The secrets of the vault at `vault`, or none when there is no file.
/// Settles an interrupted rekey (see the module docs).
pub fn load(vault: &Path, key: &SealKey) -> Result<DeviceSecrets> {
    let main = secrets_path(vault);
    let staged = staged_path(vault);
    if main.exists() {
        match read_sealed(&main, key) {
            Ok(secrets) => {
                if staged.exists() {
                    let _ = std::fs::remove_file(&staged);
                }
                return Ok(secrets);
            }
            Err(err) if !staged.exists() => return Err(err),
            Err(_) => {}
        }
    }
    if staged.exists() {
        let secrets = read_sealed(&staged, key)?;
        std::fs::rename(&staged, &main)?;
        return Ok(secrets);
    }
    Ok(DeviceSecrets::default())
}

/// Write the secrets; none at all removes the file.
pub fn save(vault: &Path, key: &SealKey, secrets: &DeviceSecrets) -> Result<()> {
    let main = secrets_path(vault);
    if secrets.is_empty() {
        remove(vault)?;
        return Ok(());
    }
    atomic_write(&main, &seal(key, secrets)?)
}

/// Remove the secrets file and a staged one.
pub fn remove(vault: &Path) -> Result<()> {
    for path in [secrets_path(vault), staged_path(vault)] {
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(err.into()),
        }
    }
    Ok(())
}

/// Step one of a rekey: the secrets re-sealed under `new` in `.next`; the
/// file under `old` stays until [`finish_rekey`].
pub fn stage_rekey(vault: &Path, old: &SealKey, new: &SealKey) -> Result<()> {
    let secrets = load(vault, old)?;
    if secrets.is_empty() {
        return Ok(());
    }
    atomic_write(&staged_path(vault), &seal(new, &secrets)?)
}

/// Step two of a rekey, after the vault is written under the new key.
pub fn finish_rekey(vault: &Path) -> Result<()> {
    let staged = staged_path(vault);
    if staged.exists() {
        std::fs::rename(&staged, secrets_path(vault))?;
    }
    Ok(())
}

/// Re-seal the secrets of `vault` from `old` to `new`. A caller that also
/// rewrites the vault calls [`stage_rekey`], writes the vault and then
/// [`finish_rekey`], so a crash at any point leaves a file one of the keys opens.
pub fn rekey_device_secrets(vault: &Path, old: &SealKey, new: &SealKey) -> Result<()> {
    stage_rekey(vault, old, new)?;
    finish_rekey(vault)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::kdf_profile::KdfParams;

    fn level3_key(password: &str) -> SealKey {
        SealKey::fresh_with(password.as_bytes(), KdfParams::hardened(64 * 1024, 3, 1)).expect("key")
    }

    fn sample() -> DeviceSecrets {
        DeviceSecrets {
            bybit: vec![DeviceKey {
                keys: ExchangeKeys {
                    store_id: "s1".into(),
                    api_key: "KEYKEYKEYKEY".into(),
                    api_secret: "SECRETSECRET".into(),
                    spot_store_id: None,
                    futures_store_id: Some("s2".into()),
                },
                verified: true,
            }],
        }
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    #[test]
    fn hkdf_matches_rfc5869_case_one() {
        let ikm = [0x0bu8; 22];
        let salt: Vec<u8> = (0x00u8..=0x0c).collect();
        let info: Vec<u8> = (0xf0u8..=0xf9).collect();
        let okm = hkdf_sha256(&salt, &ikm, &info);
        assert_eq!(
            hex(&okm[..]),
            "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf"
        );
    }

    #[test]
    fn the_file_key_differs_from_the_vault_key() {
        let key = level3_key("pw");
        assert_ne!(&file_key(&key)[..], &key.key[..]);
    }

    #[test]
    fn secrets_round_trip_through_the_file() {
        let dir = tempfile::tempdir().expect("dir");
        let vault = dir.path().join("v.wallet");
        let key = level3_key("pw");
        save(&vault, &key, &sample()).expect("save");
        assert!(secrets_path(&vault).exists());
        let raw = std::fs::read(secrets_path(&vault)).expect("raw");
        assert!(!raw.windows(6).any(|win| win == b"SECRET"));
        assert_eq!(load(&vault, &key).expect("load"), sample());
    }

    #[test]
    fn no_file_is_no_secrets_and_empty_removes_the_file() {
        let dir = tempfile::tempdir().expect("dir");
        let vault = dir.path().join("v.wallet");
        let key = level3_key("pw");
        assert!(load(&vault, &key).expect("load").is_empty());
        save(&vault, &key, &sample()).expect("save");
        save(&vault, &key, &DeviceSecrets::default()).expect("clear");
        assert!(!secrets_path(&vault).exists());
    }

    #[test]
    fn a_wrong_key_or_a_changed_file_is_a_secret_error() {
        let dir = tempfile::tempdir().expect("dir");
        let vault = dir.path().join("v.wallet");
        let key = level3_key("pw");
        save(&vault, &key, &sample()).expect("save");
        let good = std::fs::read(secrets_path(&vault)).expect("raw");

        assert!(matches!(
            load(&vault, &level3_key("other")),
            Err(Error::Secret)
        ));

        let cases: Vec<Vec<u8>> = vec![
            {
                let mut bytes = good.clone();
                let last = bytes.len() - 1;
                bytes[last] ^= 1;
                bytes
            },
            {
                let mut bytes = good.clone();
                bytes[HEADER_LEN - 1] ^= 1;
                bytes
            },
            {
                let mut bytes = good.clone();
                bytes[0] = b'X';
                bytes
            },
            good[..good.len() / 2].to_vec(),
            good[..HEADER_LEN + 3].to_vec(),
            Vec::new(),
        ];
        for bytes in cases {
            std::fs::write(secrets_path(&vault), &bytes).expect("tamper");
            assert!(matches!(load(&vault, &key), Err(Error::Secret)));
        }
    }

    #[test]
    fn rekey_moves_the_secrets_to_the_new_key() {
        let dir = tempfile::tempdir().expect("dir");
        let vault = dir.path().join("v.wallet");
        let old = level3_key("old");
        let new = level3_key("new");
        save(&vault, &old, &sample()).expect("save");
        rekey_device_secrets(&vault, &old, &new).expect("rekey");
        assert_eq!(load(&vault, &new).expect("new"), sample());
        assert!(matches!(load(&vault, &old), Err(Error::Secret)));
        assert!(!staged_path(&vault).exists());
    }

    #[test]
    fn a_rekey_cut_before_the_vault_write_keeps_the_old_key() {
        let dir = tempfile::tempdir().expect("dir");
        let vault = dir.path().join("v.wallet");
        let old = level3_key("old");
        let new = level3_key("new");
        save(&vault, &old, &sample()).expect("save");
        stage_rekey(&vault, &old, &new).expect("stage");
        // The vault was never rewritten: it still opens with the old key.
        assert_eq!(load(&vault, &old).expect("old"), sample());
        assert!(!staged_path(&vault).exists());
    }

    #[test]
    fn a_rekey_cut_after_the_vault_write_promotes_the_staged_file() {
        let dir = tempfile::tempdir().expect("dir");
        let vault = dir.path().join("v.wallet");
        let old = level3_key("old");
        let new = level3_key("new");
        save(&vault, &old, &sample()).expect("save");
        stage_rekey(&vault, &old, &new).expect("stage");
        // The vault is now under the new key, `finish_rekey` never ran.
        assert_eq!(load(&vault, &new).expect("new"), sample());
        assert!(!staged_path(&vault).exists());
        assert_eq!(load(&vault, &new).expect("again"), sample());
    }
}
