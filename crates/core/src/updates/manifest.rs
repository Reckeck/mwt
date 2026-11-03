//! The Tauri v2 updater static manifest (plus our `release_url` / `apk_url`)
//! and the pure decision "is there something to offer".

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::kind::{Channel, InstallKind, ManualHint};
use super::version::AppVersion;
use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlatformEntry {
    pub signature: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpdateManifest {
    pub version: String,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub pub_date: Option<String>,
    #[serde(default)]
    pub platforms: BTreeMap<String, PlatformEntry>,
    /// Release page, for installs that update by hand.
    #[serde(default)]
    pub release_url: Option<String>,
    /// Direct link to the Android package.
    #[serde(default)]
    pub apk_url: Option<String>,
}

/// What the UI shows when a newer version exists.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct UpdateOffer {
    pub version: String,
    pub notes: Option<String>,
    pub release_url: Option<String>,
    pub download_url: Option<String>,
    pub can_self_update: bool,
    pub hint: ManualHint,
}

/// `None` when the manifest is not newer than `current` for this channel.
pub fn offer_from_manifest(
    manifest: &UpdateManifest,
    current: &AppVersion,
    channel: Channel,
    kind: InstallKind,
) -> Result<Option<UpdateOffer>> {
    let offered = AppVersion::parse(&manifest.version)?;
    if !super::is_newer(current, &offered, channel) {
        return Ok(None);
    }
    let download_url = if kind == InstallKind::Android {
        manifest.apk_url.clone()
    } else {
        manifest.release_url.clone()
    };
    Ok(Some(UpdateOffer {
        version: offered.to_string(),
        notes: manifest.notes.clone(),
        release_url: manifest.release_url.clone(),
        download_url,
        can_self_update: kind.can_self_update(),
        hint: kind.manual_hint(),
    }))
}

/// Network only; the decision is [`offer_from_manifest`].
pub async fn fetch_manifest(client: &reqwest::Client, url: &str) -> Result<UpdateManifest> {
    let resp = client
        .get(url)
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await
        .map_err(|err| Error::Network(err.to_string()))?;
    if !resp.status().is_success() {
        return Err(Error::Network(format!("http {}", resp.status().as_u16())));
    }
    let bytes = resp
        .bytes()
        .await
        .map_err(|err| Error::Network(err.to_string()))?;
    serde_json::from_slice(&bytes).map_err(|err| Error::Deserialize(err.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
        "version": "1.3.0",
        "notes": "Fixes",
        "pub_date": "2026-11-03T10:00:00Z",
        "platforms": {"linux-x86_64": {"signature": "sig", "url": "https://x/app.AppImage"}}
    }"#;

    fn manifest(version: &str) -> UpdateManifest {
        UpdateManifest {
            version: version.into(),
            notes: Some("n".into()),
            pub_date: None,
            platforms: BTreeMap::new(),
            release_url: Some("https://x/release".into()),
            apk_url: Some("https://x/app.apk".into()),
        }
    }

    fn ver(text: &str) -> AppVersion {
        AppVersion::parse(text).expect("version")
    }

    #[test]
    fn deserializes_with_and_without_extension_fields() {
        let plain: UpdateManifest = serde_json::from_str(SAMPLE).expect("plain");
        assert_eq!(plain.release_url, None);
        assert_eq!(plain.platforms["linux-x86_64"].signature, "sig");
        let with = SAMPLE.replacen(
            '{',
            r#"{"release_url":"https://r","apk_url":"https://a","#,
            1,
        );
        let full: UpdateManifest = serde_json::from_str(&with).expect("full");
        assert_eq!(full.release_url.as_deref(), Some("https://r"));
        assert_eq!(full.apk_url.as_deref(), Some("https://a"));
        let minimal: UpdateManifest = serde_json::from_str(r#"{"version":"1.0.0"}"#).expect("min");
        assert!(minimal.notes.is_none() && minimal.platforms.is_empty());
    }

    #[test]
    fn appimage_self_updates() {
        let offer = offer_from_manifest(
            &manifest("1.3.0"),
            &ver("1.2.0"),
            Channel::Prod,
            InstallKind::AppImage,
        )
        .expect("ok")
        .expect("offer");
        assert!(offer.can_self_update);
        assert_eq!(offer.hint, ManualHint::None);
        assert_eq!(offer.version, "1.3.0");
    }

    #[test]
    fn deb_points_to_release_page() {
        let offer = offer_from_manifest(
            &manifest("1.3.0"),
            &ver("1.2.0"),
            Channel::Prod,
            InstallKind::Deb,
        )
        .expect("ok")
        .expect("offer");
        assert!(!offer.can_self_update);
        assert_eq!(offer.hint, ManualHint::ReleasePage);
        assert_eq!(offer.download_url.as_deref(), Some("https://x/release"));
    }

    #[test]
    fn android_gets_apk_url() {
        let offer = offer_from_manifest(
            &manifest("1.3.0"),
            &ver("1.2.0"),
            Channel::Prod,
            InstallKind::Android,
        )
        .expect("ok")
        .expect("offer");
        assert_eq!(offer.download_url.as_deref(), Some("https://x/app.apk"));
        assert_eq!(offer.hint, ManualHint::Apk);
    }

    #[test]
    fn not_newer_or_bad_version() {
        let same = offer_from_manifest(
            &manifest("1.2.0"),
            &ver("1.2.0"),
            Channel::Prod,
            InstallKind::Deb,
        );
        assert_eq!(same.expect("ok"), None);
        let pre = offer_from_manifest(
            &manifest("1.3.0-2"),
            &ver("1.2.0"),
            Channel::Prod,
            InstallKind::Deb,
        );
        assert_eq!(pre.expect("ok"), None);
        let dev = offer_from_manifest(
            &manifest("1.3.0-2"),
            &ver("1.2.0"),
            Channel::Dev,
            InstallKind::Deb,
        );
        assert!(dev.expect("ok").is_some());
        assert!(
            offer_from_manifest(
                &manifest("oops"),
                &ver("1.2.0"),
                Channel::Dev,
                InstallKind::Deb
            )
            .is_err()
        );
    }
}
