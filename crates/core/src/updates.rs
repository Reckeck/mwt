//! Auto-updater logic without Tauri: the facade of `mwt-net`.

pub use mwt_net::{
    AppVersion, Channel, InstallKind, ManualHint, PlatformEntry, UpdateManifest, UpdateOffer,
    fetch_manifest, is_newer, manifest_url, offer_from_manifest,
};
