//! The CI cache helpers: hidden top-level commands that replaced `.github/scripts/*.sh`
//! (lane keys, dependency cooking, cache pruning, job environment, cargo-chef install).

pub mod ci_env;
pub mod deps_cook;
pub mod deps_prune;
pub mod install_chef;
pub mod lane_key;

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

/// Runs a command with inherited output; an error when it cannot start or exits non-zero.
pub fn run_checked(cmd: &mut Command) -> Result<()> {
    let shown = format!("{cmd:?}");
    let status = cmd
        .status()
        .with_context(|| format!("cannot start {shown}"))?;
    if !status.success() {
        bail!("{shown} failed: {status}");
    }
    Ok(())
}

/// Runs a command and returns its trimmed-at-the-end stdout; stderr is inherited.
pub fn capture_checked(cmd: &mut Command) -> Result<String> {
    let shown = format!("{cmd:?}");
    let output = cmd
        .output()
        .with_context(|| format!("cannot start {shown}"))?;
    if !output.status.success() {
        bail!(
            "{shown} failed: {}\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// A scratch directory removed on drop.
pub struct Scratch(PathBuf);

impl Scratch {
    pub fn new(prefix: &str) -> Result<Self> {
        let path = std::env::temp_dir().join(format!("{prefix}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).with_context(|| format!("create {}", path.display()))?;
        Ok(Self(path))
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
