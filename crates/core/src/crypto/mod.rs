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
//! Key derivation: `Argon2id(password, salt)` → 32-byte XChaCha20 key.
//! A wrong password = AEAD authentication failure = `Error::Secret`.
//! There is **no** stored password verifier or key material on disk.

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use rand::RngCore;
use zeroize::Zeroize;

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

/// Encrypt `plaintext` under a header+password. Returns the full file bytes.
pub fn seal(header: &VaultHeader, password: &[u8], plaintext: &[u8]) -> Result<Vec<u8>> {
    let mut key = derive_key(password, &header.salt)?;
    let cipher = XChaCha20Poly1305::new(Key::from_slice(&key));
    key.zeroize();

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
    let (header, ct) = parse_file(bytes)?;
    let mut key = derive_key(password, &header.salt)?;
    let cipher = XChaCha20Poly1305::new(Key::from_slice(&key));
    key.zeroize();

    let aad = header.bytes();
    let nonce = XNonce::from_slice(&header.nonce);
    let pt = cipher
        .decrypt(nonce, Payload { msg: ct, aad: &aad })
        .map_err(|_| Error::Secret)?;
    let fmt = SnapshotFormat::from_u8(header.payload_fmt).ok_or(Error::Unsupported)?;
    Ok((pt, fmt))
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
