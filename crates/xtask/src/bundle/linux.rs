//! Linux bundles (deb, rpm, AppImage) and the portable tarball, under release names.
//!
//! `multi-wallet-tracker-<v>-linux-x86_64.tar.gz` is the deb payload (usr/bin, .desktop, icons):
//! the "native binary" download, and the source of the AUR package.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use super::LinuxArgs;
use crate::exec::{Cmd, run_inherited};

const NAME: &str = "multi-wallet-tracker";

/// `X.Y.Z` or `X.Y.Z-N`.
#[derive(Debug, PartialEq)]
struct Split<'a> {
    base: &'a str,
    pre: Option<&'a str>,
}

fn split_version(version: &str) -> Split<'_> {
    match version.split_once('-') {
        Some((base, pre)) => Split {
            base,
            pre: Some(pre),
        },
        None => Split {
            base: version,
            pre: None,
        },
    }
}

/// The config overlay of the rpm of a pre-release: RPM forbids `-` in Version (tauri-cli 2.12.4
/// checks it), so the rpm is packed from the same binary as Version X.Y.Z, Release 0.N, which
/// sorts before the X.Y.Z release (Release 1).
fn rpm_config(split: &Split<'_>, pre: &str) -> String {
    serde_json::json!({
        "version": split.base,
        "bundle": { "linux": { "rpm": { "release": format!("0.{pre}") } } },
    })
    .to_string()
}

/// Names of the files in the output folder.
#[derive(Debug, PartialEq)]
struct Names {
    deb: String,
    rpm: String,
    appimage: String,
    sig: String,
    root_dir: String,
    tarball: String,
}

fn names(version: &str) -> Names {
    let root_dir = format!("{NAME}-{version}-linux-x86_64");
    Names {
        deb: format!("{NAME}_{version}_amd64.deb"),
        rpm: format!("{NAME}-{version}-1.x86_64.rpm"),
        appimage: format!("{NAME}-{version}-x86_64.AppImage"),
        sig: format!("{NAME}-{version}-x86_64.AppImage.sig"),
        tarball: format!("{root_dir}.tar.gz"),
        root_dir,
    }
}

/// The single file, or fail loudly (the names of a bundle may hold spaces).
fn exactly_one(mut matches: Vec<PathBuf>, pattern: &str) -> Result<PathBuf> {
    matches.sort();
    if let [only] = matches.as_slice() {
        return Ok(only.clone());
    }
    let listed: Vec<String> = matches
        .iter()
        .map(|path| path.display().to_string())
        .collect();
    bail!("expected one file for {pattern}, got: {}", listed.join(" "))
}

/// Files of `dir` whose name ends with `suffix` (the `<dir>/*<suffix>` glob).
fn with_suffix(dir: &Path, suffix: &str) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| !name.starts_with('.') && name.ends_with(suffix))
        })
        .collect()
}

fn one(dir: &Path, suffix: &str) -> Result<PathBuf> {
    exactly_one(
        with_suffix(dir, suffix),
        &format!("{}/*{suffix}", dir.display()),
    )
}

fn temp_json(label: &str, text: &str) -> Result<PathBuf> {
    let path = std::env::temp_dir().join(format!("xtask-{label}-{}.json", std::process::id()));
    std::fs::write(&path, text).with_context(|| format!("cannot write {}", path.display()))?;
    Ok(path)
}

pub fn run(root: &Path, args: &LinuxArgs) -> Result<()> {
    let config = std::env::var("MWT_TAURI_CONFIG")
        .ok()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            anyhow::anyhow!("MWT_TAURI_CONFIG: the Tauri config overlay (cargo xtask tauri-config)")
        })?;
    let target = std::env::var("CARGO_TARGET_DIR")
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| ".rust-target".into());
    let bundle = root.join(target).join("release/bundle");
    // Bundles of an earlier build (the other channel names them differently) would be
    // collected as well.
    if bundle.exists() {
        std::fs::remove_dir_all(&bundle)?;
    }
    build(root, &args.version, &config)?;
    collect(root, &args.version, &bundle, &root.join(&args.out))
}

fn tauri(args: &[&str]) -> Cmd {
    Cmd::new("pnpm").args(["tauri"]).args(args)
}

fn build(root: &Path, version: &str, config: &str) -> Result<()> {
    // Updater artifacts (<bundle>.sig) need TAURI_SIGNING_PRIVATE_KEY: tauri fails the build when
    // createUpdaterArtifacts is on and the key is missing, so a build without the key (local, a
    // fork) simply makes no .sig files, and the release has no updater entry for Linux.
    let mut sign = Vec::new();
    let signing = std::env::var("TAURI_SIGNING_PRIVATE_KEY").is_ok_and(|key| !key.is_empty());
    let sign_config = if signing {
        let path = temp_json("sign", r#"{"bundle":{"createUpdaterArtifacts":true}}"#)?;
        sign = vec!["--config".to_string(), path.display().to_string()];
        Some(path)
    } else {
        None
    };
    let split = split_version(version);
    let result = match split.pre {
        Some(pre) => {
            // A pre-release X.Y.Z-N: the rpm is packed from the same binary as Version X.Y.Z,
            // Release 0.N (see `rpm_config`). deb and AppImage take the semver as it is.
            let first = tauri(&[
                "build",
                "--ci",
                "--bundles",
                "deb,appimage",
                "--config",
                config,
            ])
            .args(&sign);
            run_inherited(root, &first).and_then(|()| {
                let rpm = temp_json("rpm", &rpm_config(&split, pre))?;
                let second = tauri(&["bundle", "--ci", "--bundles", "rpm", "--config", config])
                    .args(["--config", &rpm.display().to_string()]);
                let done = run_inherited(root, &second);
                let _ = std::fs::remove_file(rpm);
                done
            })
        }
        None => {
            let all = tauri(&[
                "build",
                "--ci",
                "--bundles",
                "deb,rpm,appimage",
                "--config",
                config,
            ])
            .args(&sign);
            run_inherited(root, &all)
        }
    };
    if let Some(path) = sign_config {
        let _ = std::fs::remove_file(path);
    }
    result
}

fn collect(root: &Path, version: &str, bundle: &Path, out: &Path) -> Result<()> {
    let name = names(version);
    std::fs::create_dir_all(out)?;
    let deb = one(&bundle.join("deb"), ".deb")?;
    std::fs::copy(&deb, out.join(&name.deb))?;
    std::fs::copy(one(&bundle.join("rpm"), ".rpm")?, out.join(&name.rpm))?;
    let appimage = out.join(&name.appimage);
    std::fs::copy(one(&bundle.join("appimage"), ".AppImage")?, &appimage)?;
    make_executable(&appimage)?;
    // The updater signature, present when the build had TAURI_SIGNING_PRIVATE_KEY.
    if !with_suffix(&bundle.join("appimage"), ".AppImage.sig").is_empty() {
        let sig = one(&bundle.join("appimage"), ".AppImage.sig")?;
        std::fs::copy(sig, out.join(&name.sig))?;
    }
    tarball(
        root,
        &std::fs::canonicalize(&deb)?,
        &name,
        &std::fs::canonicalize(out)?,
    )?;
    run_inherited(
        root,
        &Cmd::new("ls").args(["-l"]).arg(out.display().to_string()),
    )
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut mode = std::fs::metadata(path)?.permissions();
    mode.set_mode(mode.mode() | 0o111);
    std::fs::set_permissions(path, mode)?;
    Ok(())
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<()> {
    Ok(())
}

/// The deb payload as a tarball: `ar x` the deb, unpack its data.tar.*, pack the root again.
fn tarball(root: &Path, deb: &Path, name: &Names, out: &Path) -> Result<()> {
    let work = std::env::temp_dir().join(format!("xtask-linux-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(work.join("root"))?;
    let result = (|| {
        run_inherited(
            root,
            &Cmd::new("ar")
                .arg("x")
                .arg(deb.display().to_string())
                .dir(&work),
        )?;
        let data = exactly_one(data_files(&work), "data.tar.*")?;
        run_inherited(
            root,
            &Cmd::new("tar")
                .args(["-xf"])
                .arg(data.display().to_string())
                .args(["-C", "root"])
                .dir(&work),
        )?;
        std::fs::rename(work.join("root"), work.join(&name.root_dir))?;
        run_inherited(
            root,
            &Cmd::new("tar")
                .arg("-czf")
                .arg(out.join(&name.tarball).display().to_string())
                .arg(&name.root_dir)
                .dir(&work),
        )
    })();
    let _ = std::fs::remove_dir_all(&work);
    result
}

fn data_files(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("data.tar."))
        })
        .collect()
}

#[cfg(test)]
mod tests;
