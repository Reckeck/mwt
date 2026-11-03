//! `cargo xtask ci-env`: the job environment shared by every lane (replaces ci-env.sh).

use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};

/// The `KEY=value` lines appended to `$GITHUB_ENV`. sccache lives in a directory of the checkout
/// (what the lane cache saves and restores).
pub fn env_lines(workspace: &str) -> [String; 4] {
    [
        format!("SCCACHE_DIR={workspace}/.cache/sccache"),
        "RUSTC_WRAPPER=sccache".to_string(),
        // For C/C++ only: sccache does not apply it to rustc, see the target dir in deps_cook.rs.
        format!("SCCACHE_BASEDIRS={workspace}"),
        "RUST_LOG=info".to_string(),
    ]
}

pub fn run() -> Result<()> {
    let workspace = std::env::var("GITHUB_WORKSPACE").context("GITHUB_WORKSPACE is not set")?;
    let env_file = std::env::var("GITHUB_ENV").context("GITHUB_ENV is not set")?;
    // tauri::generate_context! wants dist/ to exist (the UI is not built for a compile check).
    for dir in [".cache/sccache", "dist"] {
        let path = Path::new(&workspace).join(dir);
        std::fs::create_dir_all(&path).with_context(|| format!("create {}", path.display()))?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&env_file)
        .with_context(|| format!("open {env_file}"))?;
    for line in env_lines(&workspace) {
        writeln!(file, "{line}")?;
    }
    Ok(())
}
