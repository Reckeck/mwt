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

use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use super::local::{atomic_write, read_all};
use crate::crypto::{KEY_LEN, SealKey, open_blob, seal_blob};
use crate::error::{Error, Result};
use crate::model::chain::Chain;
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

/// The optional Blockscout key of one network (decision R8, spec E17). A
/// device secret like the Bybit keys: never in the document, never synced,
/// never shown again after it is saved. `Debug` hides the value, it is wiped
/// from memory on drop, and it is sent only in an `x-api-key` header.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BlockscoutKey(String);

impl BlockscoutKey {
    /// A key a header can carry: printable ASCII without spaces, at most 256
    /// characters. The error never echoes the value.
    pub fn parse(raw: &str) -> Result<Self> {
        let text = raw.trim();
        if text.is_empty() || text.len() > 256 || !text.chars().all(|ch| ch.is_ascii_graphic()) {
            return Err(Error::InvalidInput(
                "the Blockscout key must be 1-256 visible ASCII characters without spaces".into(),
            ));
        }
        Ok(Self(text.to_owned()))
    }

    /// The value, for one request. Do not log it.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for BlockscoutKey {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str("BlockscoutKey(<hidden>)")
    }
}

impl Drop for BlockscoutKey {
    fn drop(&mut self) {
        zeroize::Zeroize::zeroize(&mut self.0);
    }
}

/// What the user set for one network on this device: their own node, their
/// Blockscout instance and explorer, and the optional Blockscout key. An
/// endpoint left `None` is the preset of `cryptotrack::chains`. They live in
/// the secrets file because a node URL often carries a provider key in its
/// path, and a localhost node is meaningful on this device only.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkEndpoints {
    pub chain: Chain,
    #[serde(default)]
    pub rpc: Option<String>,
    #[serde(default)]
    pub blockscout: Option<String>,
    #[serde(default)]
    pub explorer: Option<String>,
    #[serde(default)]
    pub blockscout_key: Option<BlockscoutKey>,
}

impl NetworkEndpoints {
    /// Nothing set: the entry carries no information and is dropped.
    pub fn is_blank(&self) -> bool {
        self.rpc.is_none()
            && self.blockscout.is_none()
            && self.explorer.is_none()
            && self.blockscout_key.is_none()
    }
}

/// Everything in the secrets file.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceSecrets {
    #[serde(default)]
    pub bybit: Vec<DeviceKey>,
    /// Per-network endpoints and the Blockscout key (spec E17).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub crypto_networks: Vec<NetworkEndpoints>,
}

impl DeviceSecrets {
    pub fn is_empty(&self) -> bool {
        self.bybit.is_empty() && self.crypto_networks.is_empty()
    }

    pub fn network_for(&self, chain: Chain) -> Option<&NetworkEndpoints> {
        self.crypto_networks
            .iter()
            .find(|entry| entry.chain == chain)
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

fn file_key(key: &SealKey) -> Zeroizing<[u8; KEY_LEN]> {
    key.derive_subkey(INFO)
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
            crypto_networks: Vec::new(),
        }
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
    fn the_blockscout_key_lives_only_in_the_sealed_file_and_never_prints() {
        // Built at run time: no key-shaped literal sits in the source.
        let key_text = ["blk", "-", "0123456789abcdef"].concat();
        let dir = tempfile::tempdir().expect("dir");
        let vault = dir.path().join("v.wallet");
        let key = level3_key("pw");
        let mut secrets = DeviceSecrets::default();
        secrets.crypto_networks.push(NetworkEndpoints {
            chain: Chain::Base,
            rpc: Some("https://node.example.org".into()),
            blockscout: None,
            explorer: None,
            blockscout_key: Some(BlockscoutKey::parse(&key_text).expect("key")),
        });
        save(&vault, &key, &secrets).expect("save");
        let raw = std::fs::read(secrets_path(&vault)).expect("raw");
        assert!(
            !raw.windows(key_text.len())
                .any(|win| win == key_text.as_bytes())
        );
        assert!(!raw.windows(11).any(|win| win == b"node.example"));
        assert_eq!(load(&vault, &key).expect("load"), secrets);
        assert!(!format!("{secrets:?}").contains(&key_text));
        assert!(!format!("{:?}", secrets.crypto_networks[0]).contains(&key_text));
    }

    #[test]
    fn a_key_a_header_cannot_carry_is_refused_without_echo() {
        for bad in ["", "   ", "has space", "tab\there", "line\nbreak", "ключ"] {
            let err = BlockscoutKey::parse(bad).expect_err(bad);
            assert!(!err.to_string().contains("ключ"));
        }
        assert!(BlockscoutKey::parse(&"a".repeat(257)).is_err());
        assert!(BlockscoutKey::parse(" proapi_abc ").is_ok());
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
