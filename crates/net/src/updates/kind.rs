//! Release channel and how the app was installed.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    Prod,
    Dev,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum InstallKind {
    AppImage,
    WindowsNsis,
    WindowsMsi,
    Deb,
    Rpm,
    Pacman,
    Flatpak,
    Android,
    MacApp,
    Unknown,
}

/// What to tell the user when the app cannot update itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum ManualHint {
    None,
    ReleasePage,
    PackageManager,
    Apk,
}

impl InstallKind {
    pub fn can_self_update(&self) -> bool {
        matches!(
            self,
            Self::AppImage | Self::WindowsNsis | Self::WindowsMsi | Self::MacApp
        )
    }

    pub fn manual_hint(&self) -> ManualHint {
        match self {
            Self::AppImage | Self::WindowsNsis | Self::WindowsMsi | Self::MacApp => {
                ManualHint::None
            }
            Self::Deb | Self::Rpm | Self::Unknown => ManualHint::ReleasePage,
            Self::Pacman | Self::Flatpak => ManualHint::PackageManager,
            Self::Android => ManualHint::Apk,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn self_update_and_hints() {
        assert!(InstallKind::AppImage.can_self_update());
        assert!(InstallKind::MacApp.can_self_update());
        assert!(!InstallKind::Deb.can_self_update());
        assert_eq!(InstallKind::WindowsMsi.manual_hint(), ManualHint::None);
        assert_eq!(InstallKind::Rpm.manual_hint(), ManualHint::ReleasePage);
        assert_eq!(InstallKind::Unknown.manual_hint(), ManualHint::ReleasePage);
        assert_eq!(
            InstallKind::Pacman.manual_hint(),
            ManualHint::PackageManager
        );
        assert_eq!(
            InstallKind::Flatpak.manual_hint(),
            ManualHint::PackageManager
        );
        assert_eq!(InstallKind::Android.manual_hint(), ManualHint::Apk);
    }

    #[test]
    fn serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&InstallKind::WindowsNsis).expect("json"),
            "\"windows_nsis\""
        );
        assert_eq!(
            serde_json::to_string(&ManualHint::ReleasePage).expect("json"),
            "\"release_page\""
        );
    }
}
