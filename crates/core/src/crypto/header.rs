//! Fixed-size on-disk header of a vault file. The serialized header bytes are
//! used as AEAD associated data, so any tampering is detected even if the
//! payload section is untouched.

use crate::VAULT_MAGIC;
use crate::crypto::kdf_profile::KdfParams;
use crate::crypto::{NONCE_LEN, SALT_LEN};
use crate::error::{Error, Result};
use crate::ipc_contract::security::SecurityLevel;
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
    /// Argon2id at the crypto level's cost. The algorithm is the same as
    /// [`Self::KDF_ARGON2ID`], the id is what differs: a build that predates
    /// the cost in the header derives with its own constants, and would call a
    /// correct password wrong. An unknown id makes it answer `Unsupported`.
    pub const KDF_ARGON2ID_HARDENED: u8 = 2;
    pub const FMT_JSON: u8 = 0;
    /// bincode: positional, written by v1 release builds; read-only now.
    pub const FMT_BIN: u8 = 1;
    /// MessagePack with field names: release builds from payload v2 on.
    pub const FMT_PACK: u8 = 2;

    pub fn new(fmt: SnapshotFormat, salt: [u8; SALT_LEN], nonce: [u8; NONCE_LEN]) -> Self {
        Self::with_kdf(fmt, KdfParams::STANDARD, salt, nonce)
    }

    /// The header of a file sealed with a key derived at the cost `kdf`.
    pub fn with_kdf(
        fmt: SnapshotFormat,
        kdf: KdfParams,
        salt: [u8; SALT_LEN],
        nonce: [u8; NONCE_LEN],
    ) -> Self {
        Self {
            version: crate::VAULT_CONTAINER_VERSION,
            kdf_id: kdf.id,
            kdf_mem_kib: kdf.mem_kib,
            kdf_iters: kdf.iters,
            kdf_par: kdf.par,
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

    /// The KDF fields as they stand in the file: not checked yet, see
    /// [`KdfParams::check`].
    pub fn kdf(&self) -> KdfParams {
        KdfParams {
            id: self.kdf_id,
            mem_kib: self.kdf_mem_kib,
            iters: self.kdf_iters,
            par: self.kdf_par,
        }
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

        if !matches!(
            kdf_id,
            Self::KDF_ARGON2ID | Self::KDF_NONE | Self::KDF_ARGON2ID_HARDENED
        ) {
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

/// The protection a container claims, from its first bytes alone (at least
/// the magic and the header; a whole file will do). The level is never stored
/// anywhere else. Nothing here is authenticated: a file saying `Crypto` proves
/// it only by opening.
pub fn level_of_container(head: &[u8]) -> Result<SecurityLevel> {
    let body = head
        .strip_prefix(VAULT_MAGIC.as_slice())
        .ok_or(Error::Unsupported)?;
    let header = VaultHeader::from_bytes(body)?;
    if header.version != crate::VAULT_CONTAINER_VERSION {
        return Err(Error::Unsupported);
    }
    Ok(match header.kdf_id {
        VaultHeader::KDF_NONE => SecurityLevel::Ledger,
        VaultHeader::KDF_ARGON2ID_HARDENED => SecurityLevel::Crypto,
        _ => SecurityLevel::Pin,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `VaultHeader::from_bytes` as builds before the crypto level had it.
    fn old_build_parses(bytes: &[u8]) -> Result<u8> {
        let kdf_id = bytes[4];
        if kdf_id != VaultHeader::KDF_ARGON2ID && kdf_id != VaultHeader::KDF_NONE {
            return Err(Error::Unsupported);
        }
        Ok(kdf_id)
    }

    fn container(header: &VaultHeader) -> Vec<u8> {
        let mut head = VAULT_MAGIC.to_vec();
        head.extend_from_slice(&header.bytes());
        head
    }

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

    #[test]
    fn a_hardened_header_roundtrips_with_its_cost() {
        let kdf = KdfParams::hardened(1024 * 1024, 3, 4);
        let header = VaultHeader::with_kdf(SnapshotFormat::Pack, kdf, [7u8; 16], [9u8; 24]);
        let parsed = VaultHeader::from_bytes(&header.bytes()).unwrap();
        assert_eq!(parsed.kdf(), kdf);
        assert_eq!(parsed.bytes(), header.bytes());
        assert!(!parsed.is_plain());
    }

    #[test]
    fn an_old_build_answers_unsupported_to_the_hardened_id() {
        let kdf = KdfParams::hardened(1024 * 1024, 3, 4);
        let header = VaultHeader::with_kdf(SnapshotFormat::Pack, kdf, [7u8; 16], [9u8; 24]);
        assert!(matches!(
            old_build_parses(&header.bytes()),
            Err(Error::Unsupported)
        ));
        // The ids old builds do read are not reused.
        for known in [VaultHeader::KDF_ARGON2ID, VaultHeader::KDF_NONE] {
            assert_ne!(VaultHeader::KDF_ARGON2ID_HARDENED, known);
        }
        assert_eq!(VaultHeader::KDF_ARGON2ID_HARDENED, 2);
    }

    #[test]
    fn the_level_follows_from_the_kdf_id() {
        let plain = VaultHeader::plain(SnapshotFormat::Pack);
        let pin = VaultHeader::new(SnapshotFormat::Pack, [7u8; 16], [9u8; 24]);
        let hardened = VaultHeader::with_kdf(
            SnapshotFormat::Pack,
            KdfParams::hardened(256 * 1024, 3, 1),
            [7u8; 16],
            [9u8; 24],
        );
        for (header, level) in [
            (plain, SecurityLevel::Ledger),
            (pin, SecurityLevel::Pin),
            (hardened, SecurityLevel::Crypto),
        ] {
            assert_eq!(level_of_container(&container(&header)).unwrap(), level);
        }
    }

    #[test]
    fn a_level_is_not_read_from_what_is_not_a_container() {
        let good = container(&VaultHeader::new(
            SnapshotFormat::Pack,
            [7u8; 16],
            [9u8; 24],
        ));
        let mut foreign = good.clone();
        foreign[0] ^= 1;
        let mut future = good.clone();
        future[VAULT_MAGIC.len()] ^= 2;
        let mut unknown_kdf = good.clone();
        unknown_kdf[VAULT_MAGIC.len() + 4] = 9;
        for bad in [&b""[..], &good[..20], &foreign, &future, &unknown_kdf] {
            assert!(matches!(level_of_container(bad), Err(Error::Unsupported)));
        }
    }
}
