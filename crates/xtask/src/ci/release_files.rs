//! `xtask check-release-files <dir> <version> [--macos]`: the files of a release (decision 12 of
//! the build rewrite plan) are all in the directory, and none is signed with the throwaway test key.
//!
//! Linux .deb .rpm .AppImage, linux-x86_64.tar.gz (the AUR source), .pkg.tar.zst (pacman), .flatpak;
//! Android universal APK and AAB; Windows NSIS setup.exe, an .msi per language, the portable zip;
//! Updater: the .sig files of the AppImage, the setup.exe and the .msi files, only when
//! TAURI_SIGNING_PRIVATE_KEY is set (the manifest is made later, by `xtask publish`); macOS:
//! universal .dmg and the .app tarball, only with `--macos`.

use std::path::Path;

use anyhow::Result;

const PREFIX: &str = "multi-wallet-tracker";

/// A name or a pattern with one `*` (the only wildcard the list needs).
fn matches(pattern: &str, name: &str) -> bool {
    match pattern.split_once('*') {
        None => pattern == name,
        Some((head, tail)) => {
            name.len() >= head.len() + tail.len() && name.starts_with(head) && name.ends_with(tail)
        }
    }
}

/// The names (or patterns) the release needs.
pub fn wanted(version: &str, sign: bool, macos: bool) -> Vec<String> {
    let mut want: Vec<String> = [
        format!("{PREFIX}_{version}_amd64.deb"),
        format!("{PREFIX}-{version}-1.x86_64.rpm"),
        format!("{PREFIX}-{version}-x86_64.AppImage"),
        format!("{PREFIX}-{version}-linux-x86_64.tar.gz"),
        format!("{PREFIX}-{version}-x86_64.flatpak"),
        "*.pkg.tar.zst".to_string(),
        format!("{PREFIX}-{version}-android-universal.apk"),
        format!("{PREFIX}-{version}-android-universal.aab"),
        format!("{PREFIX}-{version}-x64-setup.exe"),
        format!("{PREFIX}-{version}-x64-*.msi"),
        format!("{PREFIX}-{version}-windows-x64-portable.zip"),
    ]
    .into();
    if sign {
        want.push(format!("{PREFIX}-{version}-x86_64.AppImage.sig"));
        want.push(format!("{PREFIX}-{version}-x64-setup.exe.sig"));
        want.push(format!("{PREFIX}-{version}-x64-*.msi.sig"));
    }
    if macos {
        want.push(format!("{PREFIX}-{version}-macos-universal.dmg"));
        want.push(format!("{PREFIX}-{version}-macos-universal.app.tar.gz"));
    }
    want
}

/// The wanted entries no file name satisfies, in order.
pub fn missing(names: &[String], version: &str, sign: bool, macos: bool) -> Vec<String> {
    wanted(version, sign, macos)
        .into_iter()
        .filter(|pattern| !names.iter().any(|name| matches(pattern, name)))
        .collect()
}

/// The files signed with the throwaway Android test key (not hidden ones, as the shell glob).
pub fn testkeys(names: &[String]) -> Vec<&String> {
    names
        .iter()
        .filter(|name| !name.starts_with('.') && name.contains("testkey"))
        .collect()
}

pub fn run(dir: &Path, version: &str, macos: bool) -> Result<i32> {
    let sign = std::env::var("TAURI_SIGNING_PRIVATE_KEY").is_ok_and(|key| !key.is_empty());
    let mut files: Vec<(String, u64)> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let size = entry.metadata().map(|meta| meta.len()).unwrap_or(0);
            files.push((entry.file_name().to_string_lossy().into_owned(), size));
        }
    }
    files.sort();
    let names: Vec<String> = files.iter().map(|(name, _)| name.clone()).collect();
    let absent = missing(&names, version, sign, macos);
    if !absent.is_empty() {
        eprintln!("missing from {}: {}", dir.display(), absent.join(" "));
        return Ok(1);
    }
    let keyed = testkeys(&names);
    if !keyed.is_empty() {
        eprintln!("a build signed with the throwaway Android test key must not be published:");
        for name in keyed {
            eprintln!("{}", dir.join(name).display());
        }
        return Ok(1);
    }
    println!("release files ok:");
    for (name, size) in &files {
        println!("{size:>12} {name}");
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const VERSION: &str = "1.2.3";

    fn complete(sign: bool, macos: bool) -> Vec<String> {
        wanted(VERSION, sign, macos)
            .into_iter()
            .map(|name| {
                name.replace("*.pkg", "multi-wallet-tracker-1.2.3-1-x86_64.pkg")
                    .replace("x64-*.msi", "x64-en-US.msi")
            })
            .collect()
    }

    #[test]
    fn a_complete_set_is_ok() {
        assert!(missing(&complete(false, false), VERSION, false, false).is_empty());
        assert!(missing(&complete(true, true), VERSION, true, true).is_empty());
    }

    #[test]
    fn every_missing_kind_is_reported() {
        let all = complete(false, false);
        for dropped in &all {
            let rest: Vec<String> = all
                .iter()
                .filter(|name| *name != dropped)
                .cloned()
                .collect();
            let absent = missing(&rest, VERSION, false, false);
            assert_eq!(absent.len(), 1, "{dropped}");
            assert!(matches(&absent[0], dropped), "{dropped} vs {}", absent[0]);
        }
        assert_eq!(missing(&[], VERSION, false, false).len(), 11);
    }

    #[test]
    fn globs_need_their_head_and_tail() {
        let names = vec![
            "x.pkg.tar.zst".to_string(),
            "multi-wallet-tracker-1.2.3-x64-.msi".to_string(),
        ];
        assert!(!missing(&names, VERSION, false, false).contains(&"*.pkg.tar.zst".to_string()));
        assert!(!matches("a*b", "ab_"));
        assert!(matches("a*b", "ab"));
        assert!(!matches("ab*ba", "aba"));
    }

    #[test]
    fn signatures_are_required_only_with_the_signing_key() {
        let unsigned = complete(false, false);
        assert!(missing(&unsigned, VERSION, false, false).is_empty());
        let absent = missing(&unsigned, VERSION, true, false);
        assert_eq!(absent.len(), 3);
        assert!(absent.iter().all(|name| name.ends_with(".sig")));
    }

    #[test]
    fn macos_files_are_required_only_with_the_flag() {
        let linux = complete(false, false);
        let absent = missing(&linux, VERSION, false, true);
        assert_eq!(absent.len(), 2);
        assert!(absent[0].ends_with("macos-universal.dmg"));
    }

    #[test]
    fn a_test_key_build_is_found() {
        let names = vec![
            "a-testkey.apk".to_string(),
            ".testkey".to_string(),
            "b.apk".to_string(),
        ];
        assert_eq!(testkeys(&names), [&"a-testkey.apk".to_string()]);
    }

    #[test]
    fn run_reports_an_empty_directory_and_accepts_a_full_one() {
        let dir = std::env::temp_dir().join(format!("xtask-release-files-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(run(&dir, VERSION, false).unwrap(), 1);
        let sign = std::env::var("TAURI_SIGNING_PRIVATE_KEY").is_ok_and(|key| !key.is_empty());
        for name in complete(sign, false) {
            std::fs::write(dir.join(name), b"x").unwrap();
        }
        assert_eq!(run(&dir, VERSION, false).unwrap(), 0);
        std::fs::write(dir.join("app-testkey.apk"), b"x").unwrap();
        assert_eq!(run(&dir, VERSION, false).unwrap(), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
