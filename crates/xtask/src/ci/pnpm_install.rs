//! `xtask ci-pnpm-install`: `pnpm install` against the cached store in `.cache/pnpm`.

use std::process::Command;

use anyhow::{Context, Result};

use crate::cache::run_checked;

pub fn store_dir(workspace: &str) -> String {
    format!("{workspace}/.cache/pnpm")
}

pub const INSTALL_ARGS: [&str; 3] = ["install", "--frozen-lockfile", "--prefer-offline"];

pub fn run() -> Result<()> {
    let workspace = std::env::var("GITHUB_WORKSPACE").context("GITHUB_WORKSPACE is not set")?;
    run_checked(
        Command::new("pnpm")
            .args(["config", "set", "store-dir"])
            .arg(store_dir(&workspace)),
    )?;
    run_checked(Command::new("pnpm").args(INSTALL_ARGS))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_is_inside_the_cached_directory() {
        assert_eq!(store_dir("/work/app"), "/work/app/.cache/pnpm");
    }

    #[test]
    fn install_is_locked_and_offline_first() {
        assert!(INSTALL_ARGS.contains(&"--frozen-lockfile"));
        assert!(INSTALL_ARGS.contains(&"--prefer-offline"));
    }
}
