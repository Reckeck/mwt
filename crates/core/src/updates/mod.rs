//! Auto-updater logic without Tauri: channels, install kinds, versions, the
//! update manifest and the offer decision. Network (`fetch_manifest`) is kept
//! apart from the pure parts (rule 4).

mod kind;
mod manifest;
mod version;

pub use kind::{Channel, InstallKind, ManualHint};
pub use manifest::{
    PlatformEntry, UpdateManifest, UpdateOffer, fetch_manifest, offer_from_manifest,
};
pub use version::AppVersion;

pub fn manifest_url(channel: Channel) -> &'static str {
    match channel {
        Channel::Prod => "https://github.com/Reckeck/mwt/releases/latest/download/latest.json",
        Channel::Dev => {
            "https://github.com/Reckeck/mwt/releases/download/updater-dev/latest-dev.json"
        }
    }
}

/// Never offers an equal or older version; the prod channel skips pre-releases.
pub fn is_newer(current: &AppVersion, offered: &AppVersion, channel: Channel) -> bool {
    if channel == Channel::Prod && offered.is_prerelease() {
        return false;
    }
    offered > current
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ver(text: &str) -> AppVersion {
        AppVersion::parse(text).expect("version")
    }

    #[test]
    fn prod_ignores_prereleases_dev_accepts() {
        let current = ver("1.2.0");
        assert!(!is_newer(&current, &ver("1.3.0-1"), Channel::Prod));
        assert!(is_newer(&current, &ver("1.3.0-1"), Channel::Dev));
        assert!(is_newer(&current, &ver("1.3.0"), Channel::Prod));
        assert!(is_newer(&current, &ver("1.3.0"), Channel::Dev));
    }

    #[test]
    fn never_offers_equal_or_older() {
        assert!(!is_newer(&ver("1.2.0"), &ver("1.2.0"), Channel::Dev));
        assert!(!is_newer(&ver("1.2.0"), &ver("1.1.9"), Channel::Prod));
        assert!(!is_newer(&ver("1.2.0"), &ver("1.2.0-5"), Channel::Dev));
        assert!(is_newer(&ver("1.2.0-2"), &ver("1.2.0"), Channel::Prod));
        assert!(is_newer(&ver("1.2.0-2"), &ver("1.2.0-10"), Channel::Dev));
    }

    #[test]
    fn manifest_urls() {
        assert!(manifest_url(Channel::Prod).ends_with("/latest/download/latest.json"));
        assert!(manifest_url(Channel::Dev).ends_with("/updater-dev/latest-dev.json"));
    }
}
