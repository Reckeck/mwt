//! Android APK + AAB for ANDROID_TARGETS (default: all three ABIs), signed with the release key
//! (or a throwaway one). Runs in the ci-android image, after setup-linux.sh and setup-android.sh.
//!
//! `MWT_TAURI_CONFIG` is the Android overlay (`cargo xtask tauri-config --android`),
//! `MWT_APP_ICONS` the channel's icon folder under crates/app (`Channel::env`).
//!
//! The release key comes from masked CI/CD variables: `ANDROID_KEYSTORE_BASE64` (the keystore
//! file, base64), `ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS`; the key password is the store
//! password. Without them a throwaway key signs the APK so it installs for testing; such builds
//! cannot update each other and must not be published.

mod icons;
mod sign;

use std::path::Path;

use anyhow::{Result, bail};

use super::LinuxArgs;
use crate::exec::{Cmd, run_inherited};

const DEFAULT_TARGETS: &str = "aarch64 armv7 x86_64";

/// A published build needs the real key: an APK signed with another key cannot update the app.
fn publish_key_error(publish: &str, keystore: &str) -> Option<&'static str> {
    (publish == "true" && keystore.is_empty()).then_some(
        "ANDROID_KEYSTORE_BASE64 is not set: an APK signed with another key cannot update the installed app",
    )
}

/// ABIs from ANDROID_TARGETS (space separated), all three when unset or blank.
fn targets(value: Option<&str>) -> Vec<String> {
    let list: Vec<String> = value
        .unwrap_or_default()
        .split_whitespace()
        .map(str::to_string)
        .collect();
    if list.is_empty() {
        DEFAULT_TARGETS.split(' ').map(str::to_string).collect()
    } else {
        list
    }
}

fn required(name: &str, what: &str) -> Result<String> {
    match std::env::var(name) {
        Ok(value) if !value.is_empty() => Ok(value),
        _ => bail!("{name}: {what}"),
    }
}

pub fn run(root: &Path, args: &LinuxArgs) -> Result<()> {
    let keystore = std::env::var("ANDROID_KEYSTORE_BASE64").unwrap_or_default();
    let publish = std::env::var("PUBLISH").unwrap_or_default();
    if let Some(message) = publish_key_error(&publish, &keystore) {
        // The format GitHub Actions shows as an annotation.
        bail!("::error::{message}");
    }
    // `tauri icon` writes the Android icons into the generated project, and rewrites (and adds
    // to) the committed desktop icons on the way: put the folder back as it was, also on a
    // failure. (cargo xtask build mounts a private copy and sets MWT_ICONS_ISOLATED: other
    // builds read the real folder meanwhile.)
    let _icons = if std::env::var("MWT_ICONS_ISOLATED").is_ok_and(|value| !value.is_empty()) {
        None
    } else {
        Some(icons::Backup::take(root)?)
    };
    let config = required(
        "MWT_TAURI_CONFIG",
        "the Android Tauri config overlay (cargo xtask tauri-config --android)",
    )?;
    let app_icons = required("MWT_APP_ICONS", "the channel icon folder, e.g. icons/dev")?;
    build(root, &config, &app_icons)?;
    sign::run(root, &args.version, &root.join(&args.out))
}

fn build(root: &Path, config: &str, app_icons: &str) -> Result<()> {
    // The Gradle project is generated, not committed (crates/app/gen/android). Its Java package
    // is the channel's identifier: a project left by the other channel would build both, so it
    // starts over.
    let generated = root.join("crates/app/gen/android");
    if generated.exists() {
        std::fs::remove_dir_all(&generated)?;
    }
    let tauri = |args: &[&str]| Cmd::new("pnpm").arg("tauri").args(args);
    run_inherited(
        root,
        &tauri(&["android", "init", "--ci", "--config", config]),
    )?;
    // The adaptive icon of the channel: background, foreground, monochrome.
    let manifest = format!("crates/app/{app_icons}/android/manifest.json");
    run_inherited(root, &tauri(&["icon", &manifest]))?;
    let abis = targets(std::env::var("ANDROID_TARGETS").ok().as_deref());
    run_inherited(
        root,
        &tauri(&["android", "build", "--ci", "--apk", "--aab", "--target"])
            .args(abis)
            .args(["--config", config]),
    )
}

#[cfg(test)]
mod tests;
