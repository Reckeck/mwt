//! Every copy of a vault next to it, and which key it is sealed under.
//!
//! The copies are this vault's backups ([`super::backups::list`]): the
//! `<vault>.bak` slot (a whole-document replace, a pull from the external
//! copy), the `<vault>.vN.bak` copies a migration leaves, the copies before a
//! sync (`<vault>.presync[.N].bak`), the copies taken by hand and by the
//! level wizard (`<vault>.<stamp>.bak`). The external copy (the mirror) is not
//! a file here: it receives the vault bytes after every save.
//!
//! [`inventory`] tells for each copy whether it is plain, sealed under the
//! session key, or under another key (an older password or an older salt),
//! from its header and one AEAD check with the session key: no password and
//! no KDF run. [`reseal`] moves one copy to another key, replacing it only
//! after the new bytes are read back from the disk and opened.

use std::path::Path;

use zeroize::Zeroizing;

use super::backups::{self, BackupFile};
use super::local::{
    create_synced, read_all, read_back, reencrypt_tmp_path, replace_with, still_the_written_file,
};
use crate::crypto::{self, SealKey};
use crate::error::{Error, Result};
use crate::storage::SnapshotFormat;

/// What made a copy, from its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopyKind {
    /// `<vault>.bak`: the slot a whole-document replace or a pull overwrites.
    Slot,
    /// `<vault>.vN.bak`: the file as it was before a schema migration.
    Migration,
    /// `<vault>.presync.bak`, `<vault>.presync.N.bak`: before a sync session.
    PreSync,
    /// `<vault>.<stamp>.bak` and anything else named like a backup.
    Other,
}

/// What key a copy is sealed under, as far as the session can tell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopySeal {
    /// Not encrypted: anyone with the file reads it.
    Plain,
    /// Sealed under the session key (same salt and cost, opens with it).
    CurrentKey,
    /// Encrypted under another key: an older password, or the same password
    /// with another salt. Only a password can tell which.
    OtherKey,
    /// Not a vault this build reads, or damaged: never touched.
    Unreadable,
}

/// One copy of the vault.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultCopy {
    pub file: BackupFile,
    pub kind: CopyKind,
    pub seal: CopySeal,
}

fn kind_of(vault_name: &str, file: &str) -> CopyKind {
    let middle = file
        .strip_prefix(vault_name)
        .and_then(|rest| rest.strip_suffix(".bak"))
        .unwrap_or_default();
    let middle = middle.strip_prefix('.').unwrap_or(middle);
    if middle.is_empty() {
        return CopyKind::Slot;
    }
    if middle == "presync"
        || middle.strip_prefix("presync.").is_some_and(|index| {
            !index.is_empty() && index.bytes().all(|byte| byte.is_ascii_digit())
        })
    {
        return CopyKind::PreSync;
    }
    if middle.strip_prefix('v').is_some_and(|version| {
        !version.is_empty() && version.bytes().all(|byte| byte.is_ascii_digit())
    }) {
        return CopyKind::Migration;
    }
    CopyKind::Other
}

/// The seal of the file bytes `bytes` relative to the session key `key`.
pub fn seal_of(bytes: &[u8], key: &SealKey) -> CopySeal {
    let Ok((header, _)) = crypto::parse_file(bytes) else {
        return CopySeal::Unreadable;
    };
    if header.is_plain() {
        return match crypto::open_bytes_keyed(bytes, b"") {
            Ok(_) => CopySeal::Plain,
            Err(_) => CopySeal::Unreadable,
        };
    }
    if !crypto::sealed_under(bytes, key) {
        return CopySeal::OtherKey;
    }
    match crypto::open_bytes_with(bytes, key) {
        Ok(_) => CopySeal::CurrentKey,
        // The session's salt and cost but the tag does not match: damaged.
        Err(_) => CopySeal::Unreadable,
    }
}

/// Every copy of the vault at `vault` next to it, newest first, with the key
/// it is sealed under relative to the session key `key`. A copy that cannot
/// be read is listed as [`CopySeal::Unreadable`].
pub fn inventory(vault: &Path, key: &SealKey) -> Result<Vec<VaultCopy>> {
    let vault_name = vault
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or_else(|| Error::InvalidInput("the vault has no file name".into()))?;
    let mut copies = Vec::new();
    for file in backups::list(vault)? {
        let seal = match backups::resolve(vault, &file.name).and_then(|path| read_all(&path)) {
            Ok(bytes) => seal_of(&Zeroizing::new(bytes), key),
            Err(_) => CopySeal::Unreadable,
        };
        copies.push(VaultCopy {
            kind: kind_of(&vault_name, &file.name),
            file,
            seal,
        });
    }
    Ok(copies)
}

/// The points of [`reseal`] a test can break.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResealStep {
    /// The new bytes are on the disk in the temp file, the copy is untouched.
    Written,
}

/// Move the copy called `name` of the vault at `vault` onto `new_key`.
/// `open` turns the copy's bytes into its payload (it holds the old key or
/// password); the payload is sealed again as it is (its schema version and
/// format stay), written to a temp file next to the vault, read back and
/// opened with `new_key`, and only then renamed over the copy. On any
/// failure the copy is as it was; a temp file a crash leaves is swept when
/// the vault opens.
pub fn reseal(
    vault: &Path,
    name: &str,
    open: impl FnOnce(&[u8]) -> Result<(Vec<u8>, SnapshotFormat)>,
    new_key: &SealKey,
) -> Result<()> {
    reseal_with_faults(vault, name, open, new_key, |_| Ok(()))
}

pub(crate) fn reseal_with_faults(
    vault: &Path,
    name: &str,
    open: impl FnOnce(&[u8]) -> Result<(Vec<u8>, SnapshotFormat)>,
    new_key: &SealKey,
    mut fault: impl FnMut(ResealStep) -> Result<()>,
) -> Result<()> {
    let path = backups::resolve(vault, name)?;
    let bytes = Zeroizing::new(read_all(&path)?);
    let (payload, fmt) = open(&bytes)?;
    let payload = Zeroizing::new(payload);
    let sealed = crypto::seal_with(new_key, fmt, &payload)?;
    let tmp = reencrypt_tmp_path(vault);
    let mut file = create_synced(&tmp, &sealed)?;
    let result = fault(ResealStep::Written)
        .and_then(|()| {
            let written = read_back(&mut file)?;
            let (reopened, reopened_fmt) = crypto::open_bytes_with(&written, new_key)?;
            if written != sealed || *Zeroizing::new(reopened) != *payload || reopened_fmt != fmt {
                return Err(Error::Crypto(format!(
                    "the re-encrypted copy {name} is not what was written"
                )));
            }
            Ok(written)
        })
        .and_then(|written| {
            still_the_written_file(&file, &tmp, &written)?;
            replace_with(&tmp, &path)
        });
    drop(file);
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::kdf_profile::{KDF_MEM_KIB_MIN, KdfParams};

    fn cheap_key(password: &str) -> SealKey {
        SealKey::fresh_with(
            password.as_bytes(),
            KdfParams::hardened(KDF_MEM_KIB_MIN, 1, 1),
        )
        .unwrap()
    }

    fn sealed(key: &SealKey, payload: &[u8]) -> Vec<u8> {
        crypto::seal_with(key, SnapshotFormat::Json, payload).unwrap()
    }

    fn setup() -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().join("v.wallet");
        std::fs::write(&vault, b"vault").unwrap();
        (dir, vault)
    }

    #[test]
    fn kinds_come_from_the_names() {
        for (file, kind) in [
            ("v.wallet.bak", CopyKind::Slot),
            ("v.wallet.v2.bak", CopyKind::Migration),
            ("v.wallet.presync.bak", CopyKind::PreSync),
            ("v.wallet.presync.2.bak", CopyKind::PreSync),
            ("v.wallet.20261022-094000.bak", CopyKind::Other),
            ("v.wallet.presync.x.bak", CopyKind::Other),
            ("v.wallet.vx.bak", CopyKind::Other),
        ] {
            assert_eq!(kind_of("v.wallet", file), kind, "{file}");
        }
    }

    #[test]
    fn the_inventory_tells_every_seal_without_a_password() {
        let (dir, vault) = setup();
        let current = cheap_key("now");
        let older = cheap_key("before");
        let folder = dir.path();
        std::fs::write(
            folder.join("v.wallet.bak"),
            crypto::seal_plain(SnapshotFormat::Json, b"{}"),
        )
        .unwrap();
        std::fs::write(folder.join("v.wallet.v2.bak"), sealed(&older, b"{}")).unwrap();
        std::fs::write(folder.join("v.wallet.presync.bak"), sealed(&current, b"{}")).unwrap();
        std::fs::write(folder.join("v.wallet.junk.bak"), b"not a vault").unwrap();
        // The session's salt with a changed byte: damaged, not "another key".
        let mut damaged = sealed(&current, b"{}");
        let last = damaged.len() - 1;
        damaged[last] ^= 1;
        std::fs::write(folder.join("v.wallet.cut.bak"), damaged).unwrap();

        let mut found: Vec<(String, CopyKind, CopySeal)> = inventory(&vault, &current)
            .unwrap()
            .into_iter()
            .map(|copy| (copy.file.name, copy.kind, copy.seal))
            .collect();
        found.sort_by(|left, right| left.0.cmp(&right.0));
        assert_eq!(
            found,
            [
                ("v.wallet.bak".into(), CopyKind::Slot, CopySeal::Plain),
                (
                    "v.wallet.cut.bak".into(),
                    CopyKind::Other,
                    CopySeal::Unreadable
                ),
                (
                    "v.wallet.junk.bak".into(),
                    CopyKind::Other,
                    CopySeal::Unreadable
                ),
                (
                    "v.wallet.presync.bak".into(),
                    CopyKind::PreSync,
                    CopySeal::CurrentKey
                ),
                (
                    "v.wallet.v2.bak".into(),
                    CopyKind::Migration,
                    CopySeal::OtherKey
                ),
            ]
        );
        // A plain session key sees every encrypted copy as another key's.
        let plain = inventory(&vault, &SealKey::plain()).unwrap();
        assert!(
            plain
                .iter()
                .find(|copy| copy.file.name == "v.wallet.presync.bak")
                .is_some_and(|copy| copy.seal == CopySeal::OtherKey)
        );
    }

    #[test]
    fn a_resealed_copy_opens_only_with_the_new_key() {
        let (dir, vault) = setup();
        let old = cheap_key("old");
        let new = cheap_key("new");
        let copy = dir.path().join("v.wallet.v1.bak");
        std::fs::write(&copy, sealed(&old, b"payload")).unwrap();
        reseal(
            &vault,
            "v.wallet.v1.bak",
            |bytes| crypto::open_bytes_with(bytes, &old),
            &new,
        )
        .unwrap();
        let bytes = std::fs::read(&copy).unwrap();
        assert_eq!(crypto::open_bytes_with(&bytes, &new).unwrap().0, b"payload");
        assert!(matches!(
            crypto::open_bytes_with(&bytes, &old),
            Err(Error::Secret)
        ));
        assert!(matches!(
            crypto::open_bytes(&bytes, b"old"),
            Err(Error::Secret)
        ));
        assert_eq!(crypto::open_bytes(&bytes, b"new").unwrap().0, b"payload");
    }

    #[test]
    fn a_failed_reseal_leaves_the_copy_and_no_temp_file() {
        let (dir, vault) = setup();
        let old = cheap_key("old");
        let new = cheap_key("new");
        let copy = dir.path().join("v.wallet.bak");
        let before = sealed(&old, b"payload");
        std::fs::write(&copy, &before).unwrap();
        // A crash right after the temp file is written: the copy is untouched.
        let err = reseal_with_faults(
            &vault,
            "v.wallet.bak",
            |bytes| crypto::open_bytes_with(bytes, &old),
            &new,
            |_| Err(Error::Io("injected".into())),
        );
        assert!(matches!(err, Err(Error::Io(_))));
        assert_eq!(std::fs::read(&copy).unwrap(), before);
        // A copy the opener refuses (a wrong key) is not touched either.
        assert!(matches!(
            reseal(
                &vault,
                "v.wallet.bak",
                |bytes| crypto::open_bytes_with(bytes, &new),
                &new
            ),
            Err(Error::Secret)
        ));
        assert_eq!(std::fs::read(&copy).unwrap(), before);
        let names: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert!(
            names.iter().all(|name| !name.contains(".reencrypt.")),
            "{names:?}"
        );
    }
}
