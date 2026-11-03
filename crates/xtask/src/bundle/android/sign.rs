//! Sign the release APK (zipalign + apksigner) and AAB (jarsigner). Passwords reach the tools
//! through the environment (`env:NAME`), never the command line.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use base64::Engine;

use crate::exec::{Cmd, output, run_inherited};

const TEST_PASSWORD: &str = "test-only-password";
const TEST_ALIAS: &str = "test";

/// `-testkey` marks a build signed with the throwaway key.
fn file_names(version: &str, testkey: bool) -> (String, String) {
    let suffix = if testkey { "-testkey" } else { "" };
    let base = format!("multi-wallet-tracker-{version}-android-universal{suffix}");
    (format!("{base}.apk"), format!("{base}.aab"))
}

/// The single file under `dir` whose name has `marker` and ends with `extension`.
fn find(dir: &Path, marker: &str, extension: &str) -> Option<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(next) = stack.pop() {
        for entry in std::fs::read_dir(&next).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.contains(marker) && name.ends_with(extension))
            {
                found.push(path);
            }
        }
    }
    found.sort();
    found.into_iter().next()
}

/// The keystore file in a temporary folder, removed with this value.
struct Keystore {
    dir: PathBuf,
    file: PathBuf,
}

impl Keystore {
    fn new() -> Result<Self> {
        let dir = std::env::temp_dir().join(format!("xtask-keystore-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir)?;
        Ok(Self {
            file: dir.join("release.keystore"),
            dir,
        })
    }
}

impl Drop for Keystore {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn variable(name: &str) -> Result<String> {
    match std::env::var(name) {
        Ok(value) if !value.is_empty() => Ok(value),
        _ => bail!("{name} is required with ANDROID_KEYSTORE_BASE64"),
    }
}

pub fn run(root: &Path, version: &str, out: &Path) -> Result<()> {
    let outputs = root.join("crates/app/gen/android/app/build/outputs");
    let (Some(apk_in), Some(aab_in)) = (
        find(&outputs.join("apk"), "universal-release", ".apk"),
        find(&outputs.join("bundle"), "universal-release", ".aab"),
    ) else {
        bail!("release APK/AAB not found under {}", outputs.display());
    };
    let keystore = Keystore::new()?;
    let encoded = std::env::var("ANDROID_KEYSTORE_BASE64").unwrap_or_default();
    let (alias, store_password, testkey) = if encoded.is_empty() {
        eprintln!("WARNING: no ANDROID_KEYSTORE_BASE64 - signing with a throwaway test key");
        let keytool = Cmd::new("keytool")
            .args(["-genkeypair", "-keystore"])
            .arg(keystore.file.display().to_string())
            .args(["-storepass", TEST_PASSWORD, "-alias", TEST_ALIAS])
            .args(["-keyalg", "RSA", "-keysize", "2048", "-validity", "365"])
            .args(["-dname", "CN=Multi Wallet Tracker test build"]);
        output(root, &keytool)?;
        (TEST_ALIAS.to_string(), TEST_PASSWORD.to_string(), true)
    } else {
        let compact: String = encoded.split_whitespace().collect();
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(compact)
            .context("ANDROID_KEYSTORE_BASE64 is not base64")?;
        std::fs::write(&keystore.file, bytes)?;
        (
            variable("ANDROID_KEY_ALIAS")?,
            variable("ANDROID_KEYSTORE_PASSWORD")?,
            false,
        )
    };
    let (apk_name, aab_name) = file_names(version, testkey);
    std::fs::create_dir_all(out)?;
    let apk_out = out.join(&apk_name);
    let aab_out = out.join(&aab_name);
    let aligned = out.join(format!("{apk_name}.aligned"));
    let idsig = out.join(format!("{apk_name}.idsig"));
    let sign_env = |cmd: Cmd| {
        cmd.env("ANDROID_KEYSTORE_PASSWORD", &store_password)
            .env("ANDROID_KEY_PASSWORD", &store_password)
    };
    let result = (|| {
        run_inherited(
            root,
            &Cmd::new("zipalign")
                .args(["-p", "-f", "4"])
                .arg(apk_in.display().to_string())
                .arg(aligned.display().to_string()),
        )?;
        let apksigner = Cmd::new("apksigner")
            .arg("sign")
            .args(["--ks", &keystore.file.display().to_string()])
            .args(["--ks-key-alias", &alias])
            .args(["--ks-pass", "env:ANDROID_KEYSTORE_PASSWORD"])
            .args(["--key-pass", "env:ANDROID_KEY_PASSWORD"])
            .args(["--out", &apk_out.display().to_string()])
            .arg(aligned.display().to_string());
        run_inherited(root, &sign_env(apksigner))?;
        let _ = std::fs::remove_file(&aligned);
        let _ = std::fs::remove_file(&idsig);
        run_inherited(
            root,
            &Cmd::new("apksigner")
                .arg("verify")
                .arg(apk_out.display().to_string()),
        )?;
        std::fs::copy(&aab_in, &aab_out)?;
        let jarsigner = Cmd::new("jarsigner")
            .args(["-sigalg", "SHA256withRSA", "-digestalg", "SHA-256"])
            .args(["-keystore", &keystore.file.display().to_string()])
            .args(["-storepass:env", "ANDROID_KEYSTORE_PASSWORD"])
            .args(["-keypass:env", "ANDROID_KEY_PASSWORD"])
            .arg(aab_out.display().to_string())
            .arg(&alias);
        output(root, &sign_env(jarsigner)).map(|_| ())
    })();
    let _ = std::fs::remove_file(&aligned);
    result?;
    run_inherited(
        root,
        &Cmd::new("ls").arg("-l").arg(out.display().to_string()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_builds_are_marked() {
        assert_eq!(
            file_names("0.1.0-3", true),
            (
                "multi-wallet-tracker-0.1.0-3-android-universal-testkey.apk".to_string(),
                "multi-wallet-tracker-0.1.0-3-android-universal-testkey.aab".to_string()
            )
        );
        assert_eq!(
            file_names("0.1.0", false).0,
            "multi-wallet-tracker-0.1.0-android-universal.apk"
        );
    }

    #[test]
    fn outputs_are_found_by_marker_and_extension() {
        let dir = std::env::temp_dir().join(format!("xtask-find-{}", std::process::id()));
        let deep = dir.join("universal/release");
        std::fs::create_dir_all(&deep).unwrap();
        std::fs::write(deep.join("app-universal-release-unsigned.apk"), "").unwrap();
        std::fs::write(deep.join("app-universal-debug.apk"), "").unwrap();
        let found = find(&dir, "universal-release", ".apk").unwrap();
        assert!(found.ends_with("app-universal-release-unsigned.apk"));
        assert!(find(&dir, "universal-release", ".aab").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
