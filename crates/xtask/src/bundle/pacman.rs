//! The pacman package (multi-wallet-tracker-bin) from the Linux tarball, in an
//! archlinux:base-devel container. makepkg refuses root, so a user builds it.

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

use super::LinuxArgs;
use crate::exec::{Cmd, output, run_inherited};

/// The shell line `su builder -c` runs. `-d`: runtime deps (webkit2gtk-4.1, ...) are not
/// needed to package a binary.
fn makepkg_line(work: &Path) -> String {
    format!("cd '{}' && makepkg -d --noconfirm", work.display())
}

fn packages(dir: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".pkg.tar.zst"))
        })
        .collect();
    found.sort();
    found
}

pub fn run(root: &Path, args: &LinuxArgs) -> Result<()> {
    let release = std::fs::canonicalize(root.join(&args.out))?;
    let tarball = release.join(format!(
        "multi-wallet-tracker-{}-linux-x86_64.tar.gz",
        args.version
    ));
    if !tarball.is_file() {
        bail!("no {}: build the Linux tarball first", tarball.display());
    }
    run_inherited(
        root,
        &Cmd::new("pacman").args(["-Sy", "--noconfirm", "--needed", "base-devel"]),
    )?;
    if run_inherited(root, &Cmd::new("id").arg("builder")).is_err() {
        run_inherited(root, &Cmd::new("useradd").args(["-m", "builder"]))?;
    }
    let work = std::env::temp_dir().join(format!("xtask-pacman-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work)?;
    let result = package(
        root,
        &version_args(&args.version, &tarball),
        &work,
        &release,
    );
    let _ = std::fs::remove_dir_all(&work);
    result
}

/// `render.sh <version> <tarball>`.
fn version_args(version: &str, tarball: &Path) -> Vec<String> {
    vec![version.to_string(), tarball.display().to_string()]
}

fn package(root: &Path, render: &[String], work: &Path, release: &Path) -> Result<()> {
    if let Some(tarball) = render.get(1) {
        std::fs::copy(
            tarball,
            work.join(Path::new(tarball).file_name().unwrap_or_default()),
        )?;
    }
    let pkgbuild = output(root, &Cmd::new("packaging/aur/render.sh").args(render))?;
    std::fs::write(work.join("PKGBUILD"), pkgbuild)?;
    std::fs::copy(
        root.join("packaging/aur/multi-wallet-tracker-bin.install"),
        work.join("multi-wallet-tracker-bin.install"),
    )?;
    run_inherited(
        root,
        &Cmd::new("chown")
            .args(["-R", "builder"])
            .arg(work.display().to_string()),
    )?;
    run_inherited(
        root,
        &Cmd::new("su").args(["builder", "-c", &makepkg_line(work)]),
    )?;
    for package in packages(work) {
        std::fs::copy(
            &package,
            release.join(package.file_name().unwrap_or_default()),
        )?;
    }
    for package in packages(release) {
        run_inherited(
            root,
            &Cmd::new("ls").arg("-l").arg(package.display().to_string()),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn makepkg_runs_in_the_work_folder_without_runtime_deps() {
        assert_eq!(
            makepkg_line(Path::new("/tmp/w")),
            "cd '/tmp/w' && makepkg -d --noconfirm"
        );
    }

    #[test]
    fn render_gets_the_version_and_the_tarball() {
        assert_eq!(
            version_args("0.1.0", Path::new("/r/t.tar.gz")),
            ["0.1.0", "/r/t.tar.gz"]
        );
    }

    #[test]
    fn only_packages_are_collected() {
        let dir = std::env::temp_dir().join(format!("xtask-pkgs-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for file in ["a.pkg.tar.zst", "a.pkg.tar.zst.sig", "PKGBUILD"] {
            std::fs::write(dir.join(file), "").unwrap();
        }
        assert_eq!(packages(&dir), [dir.join("a.pkg.tar.zst")]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
