//! Encrypted vault container.
//!
//! File layout (all integers little-endian):
//!
//! ```text
//! 0         magic        "TRDWVLT\0"            (8 bytes)
//! 8         version      u32                     (1)
//! 12        kdf_id       u8                      (0 = Argon2id)
//! 13        kdf_mem_kib  u32                     (64 MiB by default)
//! 17        kdf_iters    u32                     (3)
//! 21        kdf_par      u8                      (1)
//! 22        salt[16]
//! 38        nonce[24]                            (XChaCha20-Poly1305)
//! 62        payload_fmt  u8                      (0 = JSON, 1 = bincode)
//! 63        payload ... ciphertext + 16-byte tag  (AAD = bytes[0..63])
//! ```
//!
//! A vault without a password has `kdf_id = 1` ([`VaultHeader::KDF_NONE`]):
//! after the header comes the payload in the clear and a 32-byte SHA-256 of
//! `header ‖ payload` (detects a damaged file; it protects nothing). Sync
//! between devices never depends on the file: it has its own encrypted
//! channel (Noise), so a plain file does not weaken it.
//!
//! Key derivation: `Argon2id(password, salt)` → 32-byte XChaCha20 key.
//! A wrong password = AEAD authentication failure = `Error::Secret`.
//! There is **no** stored password verifier or key material on disk.
//!
//! An open vault keeps its derived key in memory ([`SealKey`], zeroed on drop)
//! and seals every save with it under the same salt and a fresh random nonce:
//! Argon2 runs once per unlock, not once per save (it took ~80 ms on a desktop
//! and several times that on a phone, under the vault lock). The salt changes
//! with the password (`SealKey::fresh`).

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use rand::RngCore;
use zeroize::Zeroizing;

use crate::VAULT_MAGIC;
use crate::crypto::header::VaultHeader;
use crate::error::{Error, Result};
use crate::storage::SnapshotFormat;

pub mod header;

pub use crate::vault_io::local::{atomic_write, read_all};

pub const NONCE_LEN: usize = 24;
pub const TAG_LEN: usize = 16;
pub const SALT_LEN: usize = 16;
pub const KEY_LEN: usize = 32;

/// Default KDF parameters (memory-hard, GPU-resistant).
pub const KDF_MEM_KIB: u32 = 64 * 1024; // 64 MiB
pub const KDF_ITERS: u32 = 3;
pub const KDF_PAR: u8 = 1;

/// Derive a 32-byte key from the master password + salt.
pub fn derive_key(password: &[u8], salt: &[u8]) -> Result<[u8; KEY_LEN]> {
    let params = Params::new(KDF_MEM_KIB, KDF_ITERS, KDF_PAR as u32, Some(KEY_LEN))
        .map_err(|err| Error::Crypto(err.to_string()))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = [0u8; KEY_LEN];
    argon
        .hash_password_into(password, salt, &mut key)
        .map_err(|err| Error::Crypto(err.to_string()))?;
    Ok(key)
}

/// Compute a fresh random salt + nonce header.
pub fn random_header(
    payload_fmt: SnapshotFormat,
) -> (VaultHeader, [u8; SALT_LEN], [u8; NONCE_LEN]) {
    let mut salt = [0u8; SALT_LEN];
    let mut nonce = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut salt);
    rand::thread_rng().fill_bytes(&mut nonce);
    let header = VaultHeader::new(payload_fmt, salt, nonce);
    (header, salt, nonce)
}

/// The key of an open vault and the salt it was derived with. Zeroed on drop;
/// never written, logged or sent anywhere.
pub struct SealKey {
    salt: [u8; SALT_LEN],
    key: Zeroizing<[u8; KEY_LEN]>,
    /// A vault without a password: saves are written in the clear.
    plain: bool,
}

impl SealKey {
    /// Derive the key for `salt` (the one in the file being opened).
    pub fn derive(password: &[u8], salt: [u8; SALT_LEN]) -> Result<Self> {
        let key = Zeroizing::new(derive_key(password, &salt)?);
        Ok(Self {
            salt,
            key,
            plain: false,
        })
    }

    /// The "key" of a vault without a password: nothing to derive or keep.
    pub fn plain() -> Self {
        Self {
            salt: [0u8; SALT_LEN],
            key: Zeroizing::new([0u8; KEY_LEN]),
            plain: true,
        }
    }

    /// Saves under this key are not encrypted.
    pub fn is_plain(&self) -> bool {
        self.plain
    }

    /// A new random salt and its key: a new vault or a new password.
    pub fn fresh(password: &[u8]) -> Result<Self> {
        let mut salt = [0u8; SALT_LEN];
        rand::thread_rng().fill_bytes(&mut salt);
        Self::derive(password, salt)
    }

    pub fn salt(&self) -> &[u8; SALT_LEN] {
        &self.salt
    }
}

impl std::fmt::Debug for SealKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SealKey(…)")
    }
}

/// Encrypt `plaintext` with an already derived key: same salt, fresh nonce.
pub fn seal_with(key: &SealKey, payload_fmt: SnapshotFormat, plaintext: &[u8]) -> Result<Vec<u8>> {
    if key.plain {
        return Ok(seal_plain(payload_fmt, plaintext));
    }
    let mut nonce = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce);
    let header = VaultHeader::new(payload_fmt, key.salt, nonce);
    encrypt(&header, &key.key, plaintext)
}

/// The file of a vault without a password: header, payload, checksum.
pub fn seal_plain(payload_fmt: SnapshotFormat, plaintext: &[u8]) -> Vec<u8> {
    let header = VaultHeader::plain(payload_fmt).bytes();
    let mut out = Vec::with_capacity(VAULT_MAGIC.len() + header.len() + plaintext.len() + 32);
    out.extend_from_slice(VAULT_MAGIC);
    out.extend_from_slice(&header);
    out.extend_from_slice(plaintext);
    out.extend_from_slice(&plain_checksum(&header, plaintext));
    out
}

fn plain_checksum(header: &[u8], payload: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(header);
    hasher.update(payload);
    hasher.finalize().into()
}

/// Whether the vault file at `path` has a password. Reads only the header;
/// a file that cannot be read or is not a vault counts as encrypted (the
/// safe answer: the unlock screen then asks for a password).
pub fn file_is_encrypted(path: &std::path::Path) -> bool {
    use std::io::Read;
    let mut head = [0u8; 8 + VaultHeader::LEN];
    let Ok(mut file) = std::fs::File::open(path) else {
        return true;
    };
    if file.read_exact(&mut head).is_err() || &head[..VAULT_MAGIC.len()] != VAULT_MAGIC {
        return true;
    }
    VaultHeader::from_bytes(&head[VAULT_MAGIC.len()..]).is_ok_and(|header| !header.is_plain())
}

/// Encrypt `plaintext` under a header+password. Returns the full file bytes.
pub fn seal(header: &VaultHeader, password: &[u8], plaintext: &[u8]) -> Result<Vec<u8>> {
    if header.is_plain() {
        return Ok(seal_plain(
            SnapshotFormat::from_u8(header.payload_fmt).ok_or(Error::Unsupported)?,
            plaintext,
        ));
    }
    let key = Zeroizing::new(derive_key(password, &header.salt)?);
    encrypt(header, &key, plaintext)
}

fn encrypt(header: &VaultHeader, key: &[u8; KEY_LEN], plaintext: &[u8]) -> Result<Vec<u8>> {
    let cipher = XChaCha20Poly1305::new(Key::from_slice(key));

    let aad = header.bytes();
    let nonce = XNonce::from_slice(&header.nonce);
    let ct = cipher
        .encrypt(
            nonce,
            Payload {
                msg: plaintext,
                aad: &aad,
            },
        )
        .map_err(|_| Error::Crypto("encryption failed".into()))?;

    let mut out = Vec::with_capacity(VAULT_MAGIC.len() + aad.len() + ct.len());
    out.extend_from_slice(VAULT_MAGIC);
    out.extend_from_slice(&aad);
    out.extend_from_slice(&ct);
    Ok(out)
}

/// Verify magic + version and return (header, ciphertext bytes after header).
pub fn parse_file(bytes: &[u8]) -> Result<(VaultHeader, &[u8])> {
    if bytes.len() < VAULT_MAGIC.len() {
        return Err(Error::Unsupported);
    }
    if &bytes[0..VAULT_MAGIC.len()] != VAULT_MAGIC {
        return Err(Error::Unsupported);
    }
    let header = VaultHeader::from_bytes(&bytes[VAULT_MAGIC.len()..])?;
    if header.version != crate::VAULT_CONTAINER_VERSION {
        return Err(Error::Unsupported);
    }
    let ct = &bytes[VAULT_MAGIC.len() + VaultHeader::LEN..];
    if ct.len() < TAG_LEN {
        return Err(Error::Unsupported);
    }
    Ok((header, ct))
}

/// Decrypt a vault file into its plaintext snapshot bytes.
pub fn open_bytes(bytes: &[u8], password: &[u8]) -> Result<(Vec<u8>, SnapshotFormat)> {
    open_bytes_keyed(bytes, password).map(|(plaintext, fmt, _)| (plaintext, fmt))
}

/// [`open_bytes`], also handing back the derived key for the session's saves.
pub fn open_bytes_keyed(
    bytes: &[u8],
    password: &[u8],
) -> Result<(Vec<u8>, SnapshotFormat, SealKey)> {
    let (header, ct) = parse_file(bytes)?;
    if header.is_plain() {
        // No password: the one given (if any) is ignored.
        let fmt = SnapshotFormat::from_u8(header.payload_fmt).ok_or(Error::Unsupported)?;
        let split = ct.len().checked_sub(32).ok_or(Error::Unsupported)?;
        let (payload, checksum) = ct.split_at(split);
        if plain_checksum(&header.bytes(), payload).as_slice() != checksum {
            return Err(Error::Deserialize("the vault file is damaged".into()));
        }
        return Ok((payload.to_vec(), fmt, SealKey::plain()));
    }
    let key = SealKey::derive(password, header.salt)?;
    let cipher = XChaCha20Poly1305::new(Key::from_slice(&*key.key));

    let aad = header.bytes();
    let nonce = XNonce::from_slice(&header.nonce);
    let pt = cipher
        .decrypt(nonce, Payload { msg: ct, aad: &aad })
        .map_err(|_| Error::Secret)?;
    let fmt = SnapshotFormat::from_u8(header.payload_fmt).ok_or(Error::Unsupported)?;
    Ok((pt, fmt, key))
}

/// A fresh random key for one blob (an attachment's file).
pub fn random_blob_key() -> Zeroizing<[u8; KEY_LEN]> {
    let mut key = Zeroizing::new([0u8; KEY_LEN]);
    rand::thread_rng().fill_bytes(&mut *key);
    key
}

/// Seal a blob: `nonce ‖ ciphertext`, bound to `aad` (the blob's id), so a
/// file cannot be swapped for another one sealed with the same key.
pub fn seal_blob(key: &[u8; KEY_LEN], aad: &[u8], plaintext: &[u8]) -> Result<Vec<u8>> {
    let mut nonce = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce);
    let cipher = XChaCha20Poly1305::new(Key::from_slice(key));
    let ct = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| Error::Crypto("encryption failed".into()))?;
    let mut out = Vec::with_capacity(NONCE_LEN + ct.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ct);
    Ok(out)
}

/// Open what [`seal_blob`] wrote; a changed byte or another id is an error.
pub fn open_blob(key: &[u8; KEY_LEN], aad: &[u8], sealed: &[u8]) -> Result<Vec<u8>> {
    if sealed.len() < NONCE_LEN + TAG_LEN {
        return Err(Error::Crypto("sealed file is cut short".into()));
    }
    let (nonce, ct) = sealed.split_at(NONCE_LEN);
    let cipher = XChaCha20Poly1305::new(Key::from_slice(key));
    cipher
        .decrypt(XNonce::from_slice(nonce), Payload { msg: ct, aad })
        .map_err(|_| Error::Crypto("sealed file does not open".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blob_opens_only_with_its_key_and_id() {
        let key = random_blob_key();
        let sealed = seal_blob(&key, b"a1", b"receipt").unwrap();
        assert_eq!(open_blob(&key, b"a1", &sealed).unwrap(), b"receipt");
        assert!(open_blob(&key, b"a2", &sealed).is_err());
        assert!(open_blob(&random_blob_key(), b"a1", &sealed).is_err());
        let mut broken = sealed.clone();
        *broken.last_mut().unwrap() ^= 1;
        assert!(open_blob(&key, b"a1", &broken).is_err());
    }

    #[test]
    fn a_plain_file_roundtrips_ignores_the_password_and_notices_damage() {
        let file = seal_plain(SnapshotFormat::Pack, b"payload");
        let (header, _) = parse_file(&file).unwrap();
        assert!(header.is_plain());
        for typed in [&b""[..], b"x"] {
            let (plaintext, fmt, key) = open_bytes_keyed(&file, typed).unwrap();
            assert_eq!(
                (plaintext.as_slice(), fmt),
                (&b"payload"[..], SnapshotFormat::Pack)
            );
            assert!(key.is_plain());
        }
        let resaved = seal_with(&SealKey::plain(), SnapshotFormat::Pack, b"payload").unwrap();
        assert_eq!(resaved, file);
        let mut broken = file.clone();
        broken[VAULT_MAGIC.len() + VaultHeader::LEN] ^= 1;
        assert!(matches!(
            open_bytes(&broken, b""),
            Err(Error::Deserialize(_))
        ));
        // The header is covered too.
        let mut other_format = file;
        other_format[VAULT_MAGIC.len() + 54] = SnapshotFormat::Json.as_u8();
        assert!(open_bytes(&other_format, b"").is_err());
    }

    #[test]
    fn roundtrip_encrypt_decrypt() {
        let password = b"correct horse battery staple";
        let plaintext = b"{ \"hello\": \"firefly\" }";
        let (header, salt, nonce) = random_header(SnapshotFormat::Json);
        let file = seal(&header, password, plaintext).unwrap();
        assert!(file.len() >= VAULT_MAGIC.len() + VaultHeader::LEN);

        let (out, fmt) = open_bytes(&file, password).unwrap();
        assert_eq!(out, plaintext);
        assert_eq!(fmt, SnapshotFormat::Json);
        assert_eq!(header.salt, salt);
        assert_eq!(header.nonce, nonce);
    }

    #[test]
    fn wrong_password_fails() {
        let (header, _, _) = random_header(SnapshotFormat::Json);
        let file = seal(&header, b"right", b"data").unwrap();
        assert!(matches!(open_bytes(&file, b"wrong"), Err(Error::Secret)));
    }

    #[test]
    fn tampered_file_uleis() {
        let (header, _, _) = random_header(SnapshotFormat::Json);
        let mut file = seal(&header, b"pw", b"data").unwrap();
        let last = file.len() - 1;
        file[last] ^= 0xFF;
        assert!(matches!(open_bytes(&file, b"pw"), Err(Error::Secret)));
    }

    #[test]
    fn nonce_and_salt_are_random() {
        let (h1, _, _) = random_header(SnapshotFormat::Json);
        let (h2, _, _) = random_header(SnapshotFormat::Json);
        assert_ne!(h1.salt, h2.salt);
        assert_ne!(h1.nonce, h2.nonce);
    }

    #[test]
    fn a_save_with_the_session_key_opens_with_the_password() {
        let password = b"correct horse battery staple";
        let key = SealKey::fresh(password).unwrap();
        let file = seal_with(&key, SnapshotFormat::Json, b"payload").unwrap();
        let (plaintext, fmt) = open_bytes(&file, password).unwrap();
        assert_eq!(plaintext, b"payload");
        assert_eq!(fmt, SnapshotFormat::Json);
    }

    #[test]
    fn saves_with_one_key_share_the_salt_but_never_the_nonce() {
        let key = SealKey::fresh(b"pw").unwrap();
        let first = seal_with(&key, SnapshotFormat::Pack, b"one").unwrap();
        let second = seal_with(&key, SnapshotFormat::Pack, b"one").unwrap();
        let (one, _) = parse_file(&first).unwrap();
        let (two, _) = parse_file(&second).unwrap();
        assert_eq!(one.salt, two.salt);
        assert_eq!(&one.salt, key.salt());
        assert_ne!(one.nonce, two.nonce);
    }

    #[test]
    fn opening_hands_back_the_key_of_the_file() {
        let key = SealKey::fresh(b"pw").unwrap();
        let file = seal_with(&key, SnapshotFormat::Json, b"x").unwrap();
        let (_, _, reopened) = open_bytes_keyed(&file, b"pw").unwrap();
        assert_eq!(reopened.salt(), key.salt());
        // The reopened key seals a file the password still opens.
        let again = seal_with(&reopened, SnapshotFormat::Json, b"y").unwrap();
        assert_eq!(open_bytes(&again, b"pw").unwrap().0, b"y");
    }

    #[test]
    fn a_keyed_save_still_rejects_a_wrong_password_and_tampering() {
        let key = SealKey::fresh(b"right").unwrap();
        let mut file = seal_with(&key, SnapshotFormat::Json, b"secret").unwrap();
        assert!(matches!(open_bytes(&file, b"wrong"), Err(Error::Secret)));
        let last = file.len() - 1;
        file[last] ^= 0x01;
        assert!(matches!(open_bytes(&file, b"right"), Err(Error::Secret)));
    }

    #[test]
    fn the_key_never_shows_in_debug_output() {
        let key = SealKey::fresh(b"pw").unwrap();
        assert_eq!(format!("{key:?}"), "SealKey(…)");
    }
}
