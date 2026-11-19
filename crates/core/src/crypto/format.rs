//! The snapshot payload encodings a vault container can carry.

use super::header::VaultHeader;
use crate::error::Error;

/// The last release that still reads the formats of before the reset; the
/// refusal names it. Fixed at release time.
pub(crate) const LAST_LEGACY_BUILD: &str = "Multi Wallet Tracker 0.X.Y";

/// The refusal of a file in a format this build no longer reads (D6): the
/// format number comes from the payload when it can be read.
pub(crate) fn legacy_refusal(version: Option<u32>, encoding: &str) -> Error {
    let number = version.map_or_else(
        || "an unknown format".to_owned(),
        |found| format!("format {found}"),
    );
    Error::LegacyVault(format!(
        "This vault file is in {number} ({encoding}), which this version no longer reads. \
         Open it once with {LAST_LEGACY_BUILD} (it converts the file to the format this \
         version reads), then update."
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotFormat {
    Json,
    /// MessagePack with field names (release builds).
    Pack,
}

impl SnapshotFormat {
    pub fn from_u8(raw: u8) -> Option<Self> {
        match raw {
            VaultHeader::FMT_JSON => Some(SnapshotFormat::Json),
            VaultHeader::FMT_PACK => Some(SnapshotFormat::Pack),
            _ => None,
        }
    }

    pub fn as_u8(self) -> u8 {
        match self {
            SnapshotFormat::Json => VaultHeader::FMT_JSON,
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

    pub fn label(self) -> &'static str {
        match self {
            SnapshotFormat::Json => "json",
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
        assert_eq!(SnapshotFormat::Pack.as_u8(), 2);
        for raw in [0, 2u8] {
            assert_eq!(SnapshotFormat::from_u8(raw).unwrap().as_u8(), raw);
        }
        // 1 was the old positional binary: refused, not a format.
        assert_eq!(SnapshotFormat::from_u8(1), None);
        assert_eq!(SnapshotFormat::from_u8(3), None);
    }
}
