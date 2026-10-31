//! The Tauri config overlay of a build (`tauri build --config <path>`): the version, the
//! channel's identifier, name, binary and icons, the Android versionCode. A file, not inline
//! JSON: it survives every shell's quoting.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Serialize;

use crate::channel::{Channel, ICON_FILES};

/// Where the overlay goes by default, relative to the repository root.
pub const DEFAULT_OUT: &str = ".cache/tauri-ci.json";

#[derive(Serialize)]
struct Overlay {
    version: String,
    identifier: String,
    #[serde(rename = "productName")]
    product_name: String,
    #[serde(rename = "mainBinaryName")]
    main_binary_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    build: Option<Build>,
    bundle: Bundle,
}

#[derive(Serialize)]
struct Build {
    #[serde(rename = "beforeBuildCommand")]
    before_build_command: String,
}

#[derive(Serialize)]
struct Bundle {
    icon: Vec<String>,
    android: Android,
}

#[derive(Serialize)]
struct Android {
    #[serde(rename = "versionCode")]
    version_code: u32,
}

pub struct Params<'a> {
    pub channel: Channel,
    pub android: bool,
    pub version: &'a str,
    pub version_code: u32,
    /// `dist/` is already built: an empty beforeBuildCommand keeps tauri from running
    /// `pnpm build` again.
    pub prebuilt_frontend: bool,
}

/// The overlay as one line of JSON, newline-terminated.
pub fn render(params: &Params) -> Result<String> {
    let channel = params.channel;
    let overlay = Overlay {
        version: params.version.to_string(),
        identifier: if params.android {
            channel.android_id()
        } else {
            channel.id()
        }
        .to_string(),
        product_name: channel.name().to_string(),
        main_binary_name: channel.bin().to_string(),
        build: params.prebuilt_frontend.then(|| Build {
            before_build_command: String::new(),
        }),
        bundle: Bundle {
            icon: ICON_FILES
                .iter()
                .map(|file| format!("{}/{file}", channel.icons()))
                .collect(),
            android: Android {
                version_code: params.version_code,
            },
        },
    };
    Ok(serde_json::to_string(&overlay)? + "\n")
}

/// Writes the overlay (`out` is relative to the root unless absolute) and returns its absolute path.
pub fn write(root: &Path, out: &str, params: &Params) -> Result<PathBuf> {
    let path = root.join(out);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
    }
    std::fs::write(&path, render(params)?).with_context(|| format!("write {}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(channel: Channel, android: bool, prebuilt: bool) -> Params<'static> {
        Params {
            channel,
            android,
            version: "1.2.3-4",
            version_code: 4,
            prebuilt_frontend: prebuilt,
        }
    }

    #[test]
    fn dev_desktop_overlay() {
        let text = render(&params(Channel::Dev, false, false)).unwrap();
        assert!(text.starts_with(
            r#"{"version":"1.2.3-4","identifier":"dev.mwt.desktop","productName":"Multi Wallet Tracker Dev","mainBinaryName":"multi-wallet-tracker-dev","bundle":{"icon":["icons/dev/32x32.png","#
        ));
        assert!(text.ends_with("\"icons/dev/icon.png\"],\"android\":{\"versionCode\":4}}}\n"));
        assert!(text.ends_with('\n'));
    }

    #[test]
    fn prod_android_prebuilt_overlay() {
        let text = render(&params(Channel::Prod, true, true)).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["identifier"], "mwt.app");
        assert_eq!(value["productName"], "Multi Wallet Tracker");
        assert_eq!(value["build"]["beforeBuildCommand"], "");
        assert_eq!(value["bundle"]["icon"][4], "icons/prod/icon.ico");
        assert_eq!(value["bundle"]["android"]["versionCode"], 4);
        assert!(text.find("\"build\"").unwrap() < text.find("\"bundle\"").unwrap());
    }

    #[test]
    fn no_build_key_without_prebuilt() {
        let text = render(&params(Channel::Prod, false, false)).unwrap();
        assert!(!text.contains("\"build\""));
    }
}
