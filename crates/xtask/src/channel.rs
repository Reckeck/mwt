//! The names a build ships under, by release channel. prod (main) is the release; dev
//! (development), none and local builds are the dev app, which installs next to it with its own
//! data. These are application identifiers: changing one loses the users' data.
//! `crates/app/tauri.conf.json` holds the dev values (a test below checks).

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    Prod,
    Dev,
}

/// The files of an icon folder that go into the bundle, in the order the config lists them.
pub const ICON_FILES: [&str; 6] = [
    "32x32.png",
    "128x128.png",
    "128x128@2x.png",
    "icon.icns",
    "icon.ico",
    "icon.png",
];

impl Channel {
    pub fn label(self) -> &'static str {
        match self {
            Self::Prod => "prod",
            Self::Dev => "dev",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Prod => "Multi Wallet Tracker",
            Self::Dev => "Multi Wallet Tracker Dev",
        }
    }

    pub fn bin(self) -> &'static str {
        match self {
            Self::Prod => "multi-wallet-tracker",
            Self::Dev => "multi-wallet-tracker-dev",
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::Prod => "mwt.desktop",
            Self::Dev => "dev.mwt.desktop",
        }
    }

    pub fn android_id(self) -> &'static str {
        match self {
            Self::Prod => "mwt.app",
            Self::Dev => "dev.mwt.app",
        }
    }

    pub fn flatpak_id(self) -> &'static str {
        match self {
            Self::Prod => "mwt.app.MultiWalletTracker",
            Self::Dev => "dev.mwt.MultiWalletTracker",
        }
    }

    /// The icon folder under `crates/app`.
    pub fn icons(self) -> &'static str {
        match self {
            Self::Prod => "icons/prod",
            Self::Dev => "icons/dev",
        }
    }

    /// Release-profile switches of the channel, for every build (local and CI alike). dev ships
    /// pre-releases and local builds: no LTO and 16 codegen units, the optimizer runs in parallel.
    /// prod sets nothing, so `[profile.release]` (full LTO, one unit) applies. A name for which
    /// `is_set` says the caller already chose a value is left out.
    pub fn build_env(self, is_set: impl Fn(&str) -> bool) -> Vec<(&'static str, &'static str)> {
        let all: &[(&str, &str)] = match self {
            Self::Prod => &[],
            Self::Dev => &[
                ("CARGO_PROFILE_RELEASE_LTO", "off"),
                ("CARGO_PROFILE_RELEASE_CODEGEN_UNITS", "16"),
            ],
        };
        all.iter()
            .copied()
            .filter(|(name, _)| !is_set(name))
            .collect()
    }

    /// `build_env` against the environment of this process.
    pub fn build_env_here(self) -> Vec<(&'static str, &'static str)> {
        self.build_env(|name| std::env::var_os(name).is_some_and(|value| !value.is_empty()))
    }

    /// The names the build scripts read (they do not call xtask: the pacman and flatpak
    /// containers have no cargo).
    pub fn env(self) -> [(&'static str, &'static str); 6] {
        [
            ("MWT_APP_NAME", self.name()),
            ("MWT_APP_BIN", self.bin()),
            ("MWT_APP_ID", self.id()),
            ("MWT_APP_ANDROID_ID", self.android_id()),
            ("MWT_APP_FLATPAK_ID", self.flatpak_id()),
            ("MWT_APP_ICONS", self.icons()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    fn read(path: &str) -> String {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        std::fs::read_to_string(root.join(path)).unwrap_or_else(|error| panic!("{path}: {error}"))
    }

    fn json(path: &str) -> serde_json::Value {
        serde_json::from_str(&read(path)).unwrap()
    }

    #[test]
    fn none_has_the_dev_names() {
        use crate::version::Kind;
        assert_eq!(Kind::None.names(), Channel::Dev);
        assert_eq!(Kind::Prod.names(), Channel::Prod);
        let error = Kind::None.publishing("build").unwrap_err().to_string();
        assert!(error.contains("--channel none"), "{error}");
        assert_eq!(Kind::Dev.publishing("build").unwrap(), Channel::Dev);
    }

    #[test]
    fn dev_builds_without_lto_and_prod_keeps_the_profile() {
        let unset = |_: &str| false;
        assert_eq!(
            Channel::Dev.build_env(unset),
            [
                ("CARGO_PROFILE_RELEASE_LTO", "off"),
                ("CARGO_PROFILE_RELEASE_CODEGEN_UNITS", "16")
            ]
        );
        assert!(Channel::Prod.build_env(unset).is_empty());
        // A value the caller set is respected: only the other name is added.
        let lto_set = |name: &str| name == "CARGO_PROFILE_RELEASE_LTO";
        assert_eq!(
            Channel::Dev.build_env(lto_set),
            [("CARGO_PROFILE_RELEASE_CODEGEN_UNITS", "16")]
        );
        assert!(Channel::Dev.build_env(|_| true).is_empty());
    }

    #[test]
    fn identifiers_are_frozen() {
        assert_eq!(Channel::Prod.id(), "mwt.desktop");
        assert_eq!(Channel::Prod.android_id(), "mwt.app");
        assert_eq!(Channel::Prod.flatpak_id(), "mwt.app.MultiWalletTracker");
        assert_eq!(Channel::Dev.id(), "dev.mwt.desktop");
        assert_eq!(Channel::Dev.android_id(), "dev.mwt.app");
        assert_eq!(Channel::Dev.flatpak_id(), "dev.mwt.MultiWalletTracker");
    }

    #[test]
    fn tauri_configs_hold_the_dev_names() {
        let dev = Channel::Dev;
        let config = json("crates/app/tauri.conf.json");
        assert_eq!(config["identifier"], dev.id());
        assert_eq!(config["productName"], dev.name());
        assert_eq!(config["mainBinaryName"], dev.bin());
        let icon = format!("{}/icon.png", dev.icons());
        let icons = config["bundle"]["icon"].as_array().unwrap();
        assert!(icons.iter().any(|item| *item == icon.as_str()), "{icons:?}");
        let android = json("crates/app/tauri.android.conf.json");
        assert_eq!(android["identifier"], dev.android_id());
    }
}
