//! Encrypted vault container.
//!
//! File layout (all integers little-endian):
//!
//! ```text
//! 0         magic        "TRDWVLT\0"            (8 bytes)
//! 8         version      u32                     (1)
//! 12        kdf_id       u8                      (0 = Argon2id, 2 = crypto level)
//! 13        kdf_mem_kib  u32                     (64 MiB below the crypto level)
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

use argon2::{Algorithm, Argon2, Block, Params, Version};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use rand::RngCore;
use zeroize::Zeroizing;

use crate::VAULT_MAGIC;
use crate::crypto::header::VaultHeader;
use crate::crypto::kdf_profile::KdfParams;
use crate::error::{Error, Result};
use crate::storage::SnapshotFormat;

pub mod header;
pub mod kdf_profile;

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
pub fn derive_key(password: &[u8], salt: &[u8], kdf: &KdfParams) -> Result<[u8; KEY_LEN]> {
    derive_key_within(password, salt, kdf, kdf_profile::kdf_memory_ceiling_kib())
}

/// [`derive_key`] under an explicit memory ceiling (tests pass their own and
/// leave the process-wide one alone).
fn derive_key_within(
    password: &[u8],
    salt: &[u8],
    kdf: &KdfParams,
    ceiling_kib: u32,
) -> Result<[u8; KEY_LEN]> {
    kdf.check()?;
    // Checked before anything is allocated: under overcommit (Linux, Android)
    // a reservation larger than the device "succeeds" and the kernel kills the
    // app once Argon2 writes to it, so `try_reserve` alone is no guard.
    if kdf.mem_kib > ceiling_kib {
        return Err(too_heavy(kdf.mem_kib, ceiling_kib));
    }
    let params = Params::new(kdf.mem_kib, kdf.iters, u32::from(kdf.par), Some(KEY_LEN))
        .map_err(|err| Error::Crypto(err.to_string()))?;
    // `hash_password_into` allocates with `vec!`, which aborts on failure;
    // here a refused reservation (no overcommit, or address space) is an error.
    let mut memory: Vec<Block> = Vec::new();
    memory
        .try_reserve_exact(params.block_count())
        .map_err(|_| too_heavy(kdf.mem_kib, ceiling_kib))?;
    memory.resize(params.block_count(), Block::default());
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = [0u8; KEY_LEN];
    argon
        .hash_password_into_with_memory(password, salt, &mut key, memory.as_mut_slice())
        .map_err(|err| Error::Crypto(err.to_string()))?;
    Ok(key)
}

fn too_heavy(mem_kib: u32, ceiling_kib: u32) -> Error {
    Error::VaultTooHeavy(format!(
        "its key needs {} MiB, this device can give {} MiB",
        mem_kib / 1024,
        ceiling_kib / 1024
    ))
}

/// Compute a fresh random salt + nonce header at the standard cost. Tests
/// only: outside them a header comes from a [`SealKey`], which carries the
/// vault's cost.
#[cfg(test)]
pub(crate) fn random_header(
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
    /// The cost the key was derived at: every save writes it into the header.
    kdf: KdfParams,
    /// A vault without a password: saves are written in the clear.
    plain: bool,
}

impl SealKey {
    /// Derive the key for `salt` and `kdf` (those of the file being opened).
    pub fn derive(password: &[u8], salt: [u8; SALT_LEN], kdf: KdfParams) -> Result<Self> {
        let key = Zeroizing::new(derive_key(password, &salt, &kdf)?);
        Ok(Self {
            salt,
            key,
            kdf,
            plain: false,
        })
    }

    /// The "key" of a vault without a password: nothing to derive or keep.
    pub fn plain() -> Self {
        Self {
            salt: [0u8; SALT_LEN],
            key: Zeroizing::new([0u8; KEY_LEN]),
            kdf: KdfParams::STANDARD,
            plain: true,
        }
    }

    /// Saves under this key are not encrypted.
    pub fn is_plain(&self) -> bool {
        self.plain
    }

    /// A new random salt and its key: a new vault or a new password.
    pub fn fresh(password: &[u8]) -> Result<Self> {
        Self::fresh_with(password, KdfParams::STANDARD)
    }

    /// [`Self::fresh`] at the cost `kdf`: the move to the crypto level.
    pub fn fresh_with(password: &[u8], kdf: KdfParams) -> Result<Self> {
        let mut salt = [0u8; SALT_LEN];
        rand::thread_rng().fill_bytes(&mut salt);
        Self::derive(password, salt, kdf)
    }

    /// A new password for a vault under this key: new salt, the same cost.
    /// [`Self::fresh`] here would quietly take a vault off the crypto level.
    pub fn with_new_password(&self, password: &[u8]) -> Result<Self> {
        Self::fresh_with(password, self.kdf)
    }

    /// The cost the key was derived at.
    pub fn kdf(&self) -> KdfParams {
        self.kdf
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
    let header = VaultHeader::with_kdf(payload_fmt, key.kdf, key.salt, nonce);
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
/// a file that cannot be read, is not a vault or carries a header this build
/// does not know (a future container version or kdf id) counts as encrypted:
/// the safe answer, the unlock screen then asks for a password and nothing
/// treats the file as a plain one it may remove or rewrite.
pub fn file_is_encrypted(path: &std::path::Path) -> bool {
    use std::io::Read;
    let mut head = [0u8; 8 + VaultHeader::LEN];
    let Ok(mut file) = std::fs::File::open(path) else {
        return true;
    };
    if file.read_exact(&mut head).is_err() || &head[..VAULT_MAGIC.len()] != VAULT_MAGIC {
        return true;
    }
    VaultHeader::from_bytes(&head[VAULT_MAGIC.len()..]).map_or(true, |header| {
        header.version != crate::VAULT_CONTAINER_VERSION || !header.is_plain()
    })
}

/// Encrypt `plaintext` under a header+password. Returns the full file bytes.
pub fn seal(header: &VaultHeader, password: &[u8], plaintext: &[u8]) -> Result<Vec<u8>> {
    if header.is_plain() {
        return Ok(seal_plain(
            SnapshotFormat::from_u8(header.payload_fmt).ok_or(Error::Unsupported)?,
            plaintext,
        ));
    }
    let key = Zeroizing::new(derive_key(password, &header.salt, &header.kdf())?);
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
        // A plain file has no KDF fields. Having them means an encrypted file
        // whose id was changed: not a damaged plain one, nothing to read.
        if header.kdf_mem_kib != 0 || header.kdf_iters != 0 || header.kdf_par != 0 {
            return Err(Error::Secret);
        }
        // No password: the one given (if any) is ignored.
        let fmt = SnapshotFormat::from_u8(header.payload_fmt).ok_or(Error::Unsupported)?;
        let split = ct.len().checked_sub(32).ok_or(Error::Unsupported)?;
        let (payload, checksum) = ct.split_at(split);
        if plain_checksum(&header.bytes(), payload).as_slice() != checksum {
            return Err(Error::Deserialize("the vault file is damaged".into()));
        }
        return Ok((payload.to_vec(), fmt, SealKey::plain()));
    }
    let key = SealKey::derive(password, header.salt, header.kdf())?;
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

    /// Sealed by the build before the KDF cost was read from the header:
    /// password "golden password", payload "golden payload", salt 0x5A…,
    /// nonce 0xA5…. Never regenerate them: they stand for files on real disks.
    const GOLDEN_JSON: &str = "54524457564c540001000000000000010003000000015a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5aa5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5006302c36b7b3408b8090eb9ef1522bd2847e6b58f5746ce3f99e739c20eab";
    const GOLDEN_BIN: &str = "54524457564c540001000000000000010003000000015a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5aa5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5016302c36b7b3408b8090eb9ef1522f839fa18b4512c5e1c06b2fada42cb81";
    const GOLDEN_PACK: &str = "54524457564c540001000000000000010003000000015a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5aa5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5026302c36b7b3408b8090eb9ef1522334bad4bb21301766accca0d7cc38758";
    const GOLDEN_PLAIN: &str = "54524457564c540001000000010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000002676f6c64656e207061796c6f6164f495226b932519f51b537cd7f7cb9bcdb3e58074d9d022b8be9ba79d4101b289";

    fn unhex(text: &str) -> Vec<u8> {
        (0..text.len())
            .step_by(2)
            .map(|at| u8::from_str_radix(&text[at..at + 2], 16).unwrap())
            .collect()
    }

    /// Offset of a header field in the file.
    fn at(header_offset: usize) -> usize {
        VAULT_MAGIC.len() + header_offset
    }

    const HEADER_KDF_ID: usize = 4;
    const HEADER_MEM: usize = 5;
    const HEADER_ITERS: usize = 9;
    const HEADER_PAR: usize = 13;

    fn hardened_file(kdf: KdfParams) -> Vec<u8> {
        let key = SealKey::fresh_with(b"pw", kdf).unwrap();
        seal_with(&key, SnapshotFormat::Pack, b"payload").unwrap()
    }

    #[test]
    fn every_cost_field_changes_the_key() {
        let salt = [3u8; SALT_LEN];
        let key = |kdf: KdfParams| derive_key(b"pw", &salt, &kdf).unwrap();
        let base = KdfParams::hardened(64 * 1024, 1, 1);
        assert_ne!(
            key(base),
            key(KdfParams {
                mem_kib: 65 * 1024,
                ..base
            })
        );
        assert_ne!(key(base), key(KdfParams { iters: 2, ..base }));
        assert_ne!(key(base), key(KdfParams { par: 2, ..base }));
    }

    #[test]
    fn files_of_the_old_builds_still_open() {
        for (golden, fmt) in [
            (GOLDEN_JSON, SnapshotFormat::Json),
            (GOLDEN_BIN, SnapshotFormat::Bin),
            (GOLDEN_PACK, SnapshotFormat::Pack),
        ] {
            let file = unhex(golden);
            let (plaintext, read, key) = open_bytes_keyed(&file, b"golden password").unwrap();
            assert_eq!((plaintext.as_slice(), read), (&b"golden payload"[..], fmt));
            assert_eq!(key.kdf(), KdfParams::STANDARD);
            assert!(matches!(open_bytes(&file, b"wrong"), Err(Error::Secret)));
        }
        let (plaintext, fmt) = open_bytes(&unhex(GOLDEN_PLAIN), b"").unwrap();
        assert_eq!(
            (plaintext.as_slice(), fmt),
            (&b"golden payload"[..], SnapshotFormat::Pack)
        );
    }

    #[test]
    fn a_file_opens_with_the_cost_in_its_header_not_the_defaults() {
        let kdf = KdfParams::hardened(70 * 1024, 2, 2);
        let file = hardened_file(kdf);
        let (plaintext, _, key) = open_bytes_keyed(&file, b"pw").unwrap();
        assert_eq!(plaintext, b"payload");
        assert_eq!(key.kdf(), kdf);
        assert!(matches!(open_bytes(&file, b"wrong"), Err(Error::Secret)));
    }

    #[test]
    fn a_session_key_keeps_the_cost_of_the_file_in_every_save() {
        let kdf = KdfParams::hardened(70 * 1024, 2, 2);
        let (_, _, reopened) = open_bytes_keyed(&hardened_file(kdf), b"pw").unwrap();
        let saved = seal_with(&reopened, SnapshotFormat::Pack, b"edited").unwrap();
        assert_eq!(parse_file(&saved).unwrap().0.kdf(), kdf);
        assert_eq!(open_bytes(&saved, b"pw").unwrap().0, b"edited");
    }

    #[test]
    fn a_new_password_keeps_the_cost_under_a_new_salt() {
        let kdf = KdfParams::hardened(70 * 1024, 2, 2);
        let key = SealKey::fresh_with(b"one", kdf).unwrap();
        let next = key.with_new_password(b"two").unwrap();
        assert_eq!(next.kdf(), kdf);
        assert_ne!(next.salt(), key.salt());
        let file = seal_with(&next, SnapshotFormat::Pack, b"x").unwrap();
        assert_eq!(open_bytes(&file, b"two").unwrap().0, b"x");
        assert!(matches!(open_bytes(&file, b"one"), Err(Error::Secret)));
    }

    #[test]
    fn a_header_outside_the_limits_is_refused_before_any_allocation() {
        let good = seal_with(&SealKey::fresh(b"pw").unwrap(), SnapshotFormat::Json, b"x").unwrap();
        let patch = |offset: usize, value: &[u8]| {
            let mut file = good.clone();
            file[at(offset)..at(offset) + value.len()].copy_from_slice(value);
            file
        };
        // Unchecked, the first would ask for 4 TiB and abort the process.
        for (offset, value) in [
            (HEADER_MEM, u32::MAX.to_le_bytes()),
            (HEADER_MEM, (1024 * 1024 + 1u32).to_le_bytes()),
            (HEADER_MEM, (64 * 1024 - 1u32).to_le_bytes()),
            (HEADER_MEM, 0u32.to_le_bytes()),
            (HEADER_ITERS, u32::MAX.to_le_bytes()),
            (HEADER_ITERS, 5u32.to_le_bytes()),
            (HEADER_ITERS, 0u32.to_le_bytes()),
        ] {
            assert!(matches!(
                open_bytes(&patch(offset, &value), b"pw"),
                Err(Error::Secret)
            ));
        }
        for par in [0u8, 5, 255] {
            assert!(matches!(
                open_bytes(&patch(HEADER_PAR, &[par]), b"pw"),
                Err(Error::Secret)
            ));
        }
    }

    #[test]
    fn a_vault_above_the_memory_ceiling_is_too_heavy_before_any_allocation() {
        // 1 GiB asked, 64 MiB allowed: an allocation would take seconds and,
        // under overcommit, could get the test process killed.
        let heavy = KdfParams::hardened(kdf_profile::KDF_MEM_KIB_MAX, 4, 4);
        let err = derive_key_within(b"pw", &[0; SALT_LEN], &heavy, kdf_profile::KDF_MEM_KIB_MIN)
            .unwrap_err();
        assert!(matches!(err, Error::VaultTooHeavy(_)), "{err:?}");
        assert_eq!(err.code(), "VAULT_TOO_HEAVY");
        let just_over = KdfParams::hardened(kdf_profile::KDF_MEM_KIB_MIN + 1024, 1, 1);
        assert!(matches!(
            derive_key_within(
                b"pw",
                &[0; SALT_LEN],
                &just_over,
                kdf_profile::KDF_MEM_KIB_MIN
            ),
            Err(Error::VaultTooHeavy(_))
        ));
    }

    #[test]
    fn a_vault_exactly_at_the_memory_ceiling_derives_its_key() {
        let at = KdfParams::hardened(kdf_profile::KDF_MEM_KIB_MIN, 1, 1);
        let within =
            derive_key_within(b"pw", &[0; SALT_LEN], &at, kdf_profile::KDF_MEM_KIB_MIN).unwrap();
        let unbounded =
            derive_key_within(b"pw", &[0; SALT_LEN], &at, kdf_profile::KDF_MEM_KIB_MAX).unwrap();
        assert_eq!(within, unbounded);
    }

    #[test]
    fn a_header_outside_the_limits_is_a_secret_error_even_above_the_ceiling() {
        // The bounds come first: a changed header must look like any other
        // file that does not open, not like a heavy vault.
        let broken = KdfParams::hardened(u32::MAX, 3, 1);
        assert!(matches!(
            derive_key_within(b"pw", &[0; SALT_LEN], &broken, kdf_profile::KDF_MEM_KIB_MIN),
            Err(Error::Secret)
        ));
    }

    /// Everything but the version, which a file from the future changes too:
    /// that must read as "update the app", not as a wrong password.
    fn every_changed_byte_is_a_secret_error(file: &[u8]) {
        for offset in 4..VaultHeader::LEN {
            if offset == HEADER_KDF_ID {
                continue;
            }
            let mut tampered = file.to_vec();
            tampered[at(offset)] ^= 0xFF;
            assert!(
                matches!(open_bytes(&tampered, b"pw"), Err(Error::Secret)),
                "header byte {offset}"
            );
        }
    }

    #[test]
    fn any_changed_header_byte_of_a_standard_file_is_a_secret_error() {
        every_changed_byte_is_a_secret_error(
            &seal_with(&SealKey::fresh(b"pw").unwrap(), SnapshotFormat::Json, b"x").unwrap(),
        );
    }

    #[test]
    fn any_changed_header_byte_of_a_hardened_file_is_a_secret_error() {
        every_changed_byte_is_a_secret_error(&hardened_file(KdfParams::hardened(64 * 1024, 1, 1)));
    }

    #[test]
    fn a_kdf_id_changed_to_a_known_one_is_a_secret_error() {
        let standard = seal_with(&SealKey::fresh(b"pw").unwrap(), SnapshotFormat::Pack, b"x");
        let hardened = hardened_file(KdfParams::hardened(64 * 1024, 3, 1));
        for (file, kdf_id) in [
            (
                standard.as_ref().unwrap(),
                VaultHeader::KDF_ARGON2ID_HARDENED,
            ),
            (standard.as_ref().unwrap(), VaultHeader::KDF_NONE),
            (&hardened, VaultHeader::KDF_ARGON2ID),
            (&hardened, VaultHeader::KDF_NONE),
        ] {
            let mut tampered = file.clone();
            tampered[at(HEADER_KDF_ID)] = kdf_id;
            assert!(matches!(open_bytes(&tampered, b"pw"), Err(Error::Secret)));
        }
    }

    #[test]
    fn a_container_version_or_kdf_from_the_future_is_unsupported_not_a_wrong_password() {
        let good = seal_with(&SealKey::fresh(b"pw").unwrap(), SnapshotFormat::Json, b"x").unwrap();
        for offset in (0..4).chain([HEADER_KDF_ID]) {
            let mut future = good.clone();
            future[at(offset)] ^= 0xFF;
            assert!(matches!(
                open_bytes(&future, b"pw"),
                Err(Error::Unsupported)
            ));
        }
    }

    #[test]
    fn a_plain_file_with_any_changed_header_byte_is_refused_without_a_panic() {
        let file = seal_plain(SnapshotFormat::Pack, b"payload");
        for offset in 0..VaultHeader::LEN {
            let mut tampered = file.clone();
            tampered[at(offset)] ^= 0xFF;
            assert!(open_bytes(&tampered, b"").is_err(), "header byte {offset}");
        }
    }

    #[test]
    fn a_file_with_a_broken_cost_still_asks_for_a_password() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v.wallet");
        let mut file = hardened_file(KdfParams::hardened(64 * 1024, 3, 1));
        std::fs::write(&path, &file).unwrap();
        assert!(file_is_encrypted(&path));
        file[at(HEADER_MEM + 3)] = 0xFF;
        std::fs::write(&path, &file).unwrap();
        assert!(file_is_encrypted(&path));
    }

    #[test]
    fn a_header_this_build_does_not_know_counts_as_encrypted() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v.wallet");
        let plain = seal_plain(SnapshotFormat::Pack, b"payload");
        std::fs::write(&path, &plain).unwrap();
        assert!(!file_is_encrypted(&path));
        let mut unknown_kdf = plain.clone();
        unknown_kdf[at(HEADER_KDF_ID)] = 9;
        std::fs::write(&path, &unknown_kdf).unwrap();
        assert!(file_is_encrypted(&path));
        let mut future = plain;
        future[at(0)..at(4)].copy_from_slice(&(crate::VAULT_CONTAINER_VERSION + 1).to_le_bytes());
        std::fs::write(&path, &future).unwrap();
        assert!(file_is_encrypted(&path));
    }
}
