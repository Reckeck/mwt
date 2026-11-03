//! `cargo xtask install-chef`: cargo-chef for the `deps` job (replaces install-chef.sh). Linux
//! x86_64: the release binary, checked against its published sha256; elsewhere (no release
//! binary) it is built from source.

use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};

use super::{Scratch, run_checked};

pub const VERSION: &str = "0.1.78";

/// The hash of a `.sha256` file: its first whitespace-separated word, 64 hex digits, lowercased.
pub fn parse_checksum(text: &str) -> Option<String> {
    let word = text.split_whitespace().next()?;
    (word.len() == 64 && word.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| word.to_ascii_lowercase())
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(unix)]
fn install(binary: &Path, target: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::copy(binary, target).with_context(|| format!("install {}", target.display()))?;
    std::fs::set_permissions(target, std::fs::Permissions::from_mode(0o755))?;
    Ok(())
}

#[cfg(not(unix))]
fn install(_binary: &Path, _target: &Path) -> Result<()> {
    bail!("the release binary is installed on Linux only")
}

fn from_release() -> Result<()> {
    let name = "cargo-chef-x86_64-unknown-linux-musl";
    let base = format!("https://github.com/LukeMathWalker/cargo-chef/releases/download/v{VERSION}");
    let dir = Scratch::new("cargo-chef")?;
    let tarball = dir.path().join(format!("{name}.tar.xz"));
    let sum_file = dir.path().join(format!("{name}.sha256"));
    for (url, path) in [
        (format!("{base}/{name}.tar.xz"), &tarball),
        (format!("{base}/{name}.tar.xz.sha256"), &sum_file),
    ] {
        run_checked(
            Command::new("curl")
                .args(["-fsSL", "-o"])
                .arg(path)
                .arg(url),
        )?;
    }
    let expected = parse_checksum(&std::fs::read_to_string(&sum_file)?)
        .context("cannot read the published sha256")?;
    let actual = sha256_hex(&std::fs::read(&tarball)?);
    if expected != actual {
        bail!("cargo-chef checksum mismatch: {expected} != {actual}");
    }
    run_checked(
        Command::new("tar")
            .arg("-xJ")
            .arg("-C")
            .arg(dir.path())
            .arg("-f")
            .arg(&tarball),
    )?;
    install(
        &dir.path().join(name).join("cargo-chef"),
        Path::new("/usr/local/bin/cargo-chef"),
    )
}

pub fn run() -> Result<()> {
    if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        from_release()?;
    } else {
        run_checked(Command::new("cargo").args([
            "install",
            "--locked",
            "--version",
            VERSION,
            "cargo-chef",
        ]))?;
    }
    run_checked(Command::new("cargo").args(["chef", "--version"]))
}
