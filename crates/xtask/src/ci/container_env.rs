//! `xtask ci-container-env [--cargo]`: the toolchain of a job container. The image keeps mise
//! shims in `/opt/mise/shims` (and cargo in `/opt/cargo/bin`); later steps need them on PATH, and
//! mise has to trust the checkout's `mise.toml`.

use std::io::Write;
use std::process::Command;

use anyhow::{Context, Result};

use crate::cache::run_checked;

/// The directories appended to `$GITHUB_PATH`, in order.
pub fn path_entries(cargo: bool) -> Vec<&'static str> {
    let mut entries = vec!["/opt/mise/shims"];
    if cargo {
        entries.push("/opt/cargo/bin");
    }
    entries
}

pub fn run(cargo: bool) -> Result<()> {
    let path_file = std::env::var("GITHUB_PATH").context("GITHUB_PATH is not set")?;
    let workspace = std::env::var("GITHUB_WORKSPACE").context("GITHUB_WORKSPACE is not set")?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path_file)
        .with_context(|| format!("open {path_file}"))?;
    for entry in path_entries(cargo) {
        writeln!(file, "{entry}")?;
    }
    run_checked(
        Command::new("mise")
            .arg("trust")
            .arg(format!("{workspace}/mise.toml")),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shims_only_by_default() {
        assert_eq!(path_entries(false), ["/opt/mise/shims"]);
    }

    #[test]
    fn cargo_comes_after_the_shims() {
        assert_eq!(path_entries(true), ["/opt/mise/shims", "/opt/cargo/bin"]);
    }
}
