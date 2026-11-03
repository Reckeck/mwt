//! The CI steps that used to be shell scripts: the release plan, the release file check, the CI
//! image build and the static checks of the CI scripts.

pub mod build_images;
pub mod cache_cleanup;
pub mod ci_ok;
pub mod container_env;
pub mod lint;
pub mod pnpm_install;
pub mod release_files;
pub mod release_plan;

use std::io::Write;

use anyhow::{Context, Result};

/// Appends `KEY=value` lines to `$GITHUB_OUTPUT`, or prints them when it is unset or empty.
pub fn write_outputs(lines: &[String]) -> Result<()> {
    let text = lines
        .iter()
        .map(|line| format!("{line}\n"))
        .collect::<String>();
    match std::env::var("GITHUB_OUTPUT")
        .ok()
        .filter(|path| !path.is_empty())
    {
        Some(path) => {
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .with_context(|| format!("open {path}"))?;
            file.write_all(text.as_bytes())?;
        }
        None => print!("{text}"),
    }
    Ok(())
}
