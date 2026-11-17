//! The snapshot payload encodings a vault container can carry.

use super::header::VaultHeader;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotFormat {
    Json,
    /// Legacy bincode (v1 release builds); read-only.
    Bin,
    /// MessagePack with field names (release builds from payload v2).
    Pack,
}

impl SnapshotFormat {
    pub fn from_u8(raw: u8) -> Option<Self> {
        match raw {
            VaultHeader::FMT_JSON => Some(SnapshotFormat::Json),
            VaultHeader::FMT_BIN => Some(SnapshotFormat::Bin),
            VaultHeader::FMT_PACK => Some(SnapshotFormat::Pack),
            _ => None,
        }
    }

    pub fn as_u8(self) -> u8 {
        match self {
            SnapshotFormat::Json => VaultHeader::FMT_JSON,
            SnapshotFormat::Bin => VaultHeader::FMT_BIN,
            SnapshotFormat::Pack => VaultHeader::FMT_PACK,
        }
    }

    /// The concrete format for this build profile (dev vs prod).
    pub fn for_profile() -> Self {
        if cfg!(debug_assertions) {
            SnapshotFormat::Json
        } else {
            SnapshotFormat::Pack
        }
    }

    /// The format a document read in `self` is written back in: bincode is
    /// never written again.
    pub fn for_writing(self) -> Self {
        match self {
            SnapshotFormat::Bin => SnapshotFormat::Pack,
            other => other,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SnapshotFormat::Json => "json",
            SnapshotFormat::Bin => "bin",
            SnapshotFormat::Pack => "pack",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_format_bytes_are_stable() {
        assert_eq!(SnapshotFormat::Json.as_u8(), 0);
        assert_eq!(SnapshotFormat::Bin.as_u8(), 1);
        assert_eq!(SnapshotFormat::Pack.as_u8(), 2);
        for raw in 0..=2u8 {
            assert_eq!(SnapshotFormat::from_u8(raw).unwrap().as_u8(), raw);
        }
        assert_eq!(SnapshotFormat::from_u8(3), None);
    }
}
