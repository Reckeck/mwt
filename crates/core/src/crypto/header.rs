//! Fixed-size on-disk header of a vault file. The serialized header bytes are
//! used as AEAD associated data, so any tampering is detected even if the
//! payload section is untouched.

use crate::crypto::{NONCE_LEN, SALT_LEN};
use crate::error::{Error, Result};
use crate::storage::SnapshotFormat;

/// Fixed header length excluding the 8-byte magic prefix.
/// 4(version)+1(kdf)+4(mem)+4(iters)+1(par)+16(salt)+24(nonce)+1(fmt) = 55
pub const HEADER_LEN: usize = 55;

#[derive(Debug, Clone)]
pub struct VaultHeader {
    pub version: u32,
    pub kdf_id: u8,
    pub kdf_mem_kib: u32,
    pub kdf_iters: u32,
    pub kdf_par: u8,
    pub salt: [u8; SALT_LEN],
    pub nonce: [u8; NONCE_LEN],
    pub payload_fmt: u8,
}

impl VaultHeader {
    pub const LEN: usize = HEADER_LEN;

    pub const KDF_ARGON2ID: u8 = 0;
    /// No password: the payload follows the header in the clear, closed by a
    /// SHA-256 checksum (corruption check, not protection). Older builds read
    /// an unknown `kdf_id` as `Unsupported`, so they refuse such a file
    /// instead of mistaking it for a wrong password.
    pub const KDF_NONE: u8 = 1;
    pub const FMT_JSON: u8 = 0;
    /// bincode: positional, written by v1 release builds; read-only now.
    pub const FMT_BIN: u8 = 1;
    /// MessagePack with field names: release builds from payload v2 on.
    pub const FMT_PACK: u8 = 2;

    pub fn new(fmt: SnapshotFormat, salt: [u8; SALT_LEN], nonce: [u8; NONCE_LEN]) -> Self {
        Self {
            version: crate::VAULT_CONTAINER_VERSION,
            kdf_id: Self::KDF_ARGON2ID,
            kdf_mem_kib: crate::crypto::KDF_MEM_KIB,
            kdf_iters: crate::crypto::KDF_ITERS,
            kdf_par: crate::crypto::KDF_PAR,
            salt,
            nonce,
            payload_fmt: fmt.as_u8(),
        }
    }

    /// The header of a vault without a password: no salt, nonce or KDF cost.
    pub fn plain(fmt: SnapshotFormat) -> Self {
        Self {
            version: crate::VAULT_CONTAINER_VERSION,
            kdf_id: Self::KDF_NONE,
            kdf_mem_kib: 0,
            kdf_iters: 0,
            kdf_par: 0,
            salt: [0u8; SALT_LEN],
            nonce: [0u8; NONCE_LEN],
            payload_fmt: fmt.as_u8(),
        }
    }

    /// The file is not encrypted (see [`Self::KDF_NONE`]).
    pub fn is_plain(&self) -> bool {
        self.kdf_id == Self::KDF_NONE
    }

    pub fn payload_format(&self) -> Option<SnapshotFormat> {
        SnapshotFormat::from_u8(self.payload_fmt)
    }

    pub fn bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(Self::LEN);
        bytes.extend_from_slice(&self.version.to_le_bytes());
        bytes.push(self.kdf_id);
        bytes.extend_from_slice(&self.kdf_mem_kib.to_le_bytes());
        bytes.extend_from_slice(&self.kdf_iters.to_le_bytes());
        bytes.push(self.kdf_par);
        bytes.extend_from_slice(&self.salt);
        bytes.extend_from_slice(&self.nonce);
        bytes.push(self.payload_fmt);
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < Self::LEN {
            return Err(Error::Unsupported);
        }
        let version = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
        let kdf_id = bytes[4];
        let kdf_mem_kib = u32::from_le_bytes(bytes[5..9].try_into().unwrap());
        let kdf_iters = u32::from_le_bytes(bytes[9..13].try_into().unwrap());
        let kdf_par = bytes[13];
        let mut salt = [0u8; SALT_LEN];
        salt.copy_from_slice(&bytes[14..14 + SALT_LEN]);
        let mut nonce = [0u8; NONCE_LEN];
        nonce.copy_from_slice(&bytes[30..30 + NONCE_LEN]);
        let payload_fmt = bytes[54];

        if kdf_id != Self::KDF_ARGON2ID && kdf_id != Self::KDF_NONE {
            return Err(Error::Unsupported);
        }

        Ok(Self {
            version,
            kdf_id,
            kdf_mem_kib,
            kdf_iters,
            kdf_par,
            salt,
            nonce,
            payload_fmt,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_roundtrip() {
        let (_, _, _) = (0, [0u8; 16], [0u8; 24]);
        let header = VaultHeader::new(SnapshotFormat::Bin, [7u8; 16], [9u8; 24]);
        let bytes = header.bytes();
        assert_eq!(bytes.len(), HEADER_LEN);
        let parsed = VaultHeader::from_bytes(&bytes).unwrap();
        assert_eq!(header.version, parsed.version);
        assert_eq!(header.salt, parsed.salt);
        assert_eq!(header.nonce, parsed.nonce);
        assert_eq!(header.payload_fmt, parsed.payload_fmt);
        assert!(!parsed.is_plain());
    }

    #[test]
    fn a_plain_header_roundtrips_and_an_unknown_kdf_is_refused() {
        let header = VaultHeader::plain(SnapshotFormat::Json);
        let bytes = header.bytes();
        assert_eq!(bytes.len(), HEADER_LEN);
        assert!(VaultHeader::from_bytes(&bytes).unwrap().is_plain());
        let mut unknown = bytes;
        unknown[4] = 9;
        assert!(matches!(
            VaultHeader::from_bytes(&unknown),
            Err(Error::Unsupported)
        ));
    }
}
