//! `cargo xtask deps-prune <prefix> <keep>`: delete the older caches of this ref and lane, keeping
//! the key just saved (replaces deps-prune.sh). Needs `actions: write` for GITHUB_TOKEN.
//! API: https://docs.github.com/en/rest/actions/cache

use std::process::Command;

use anyhow::{Context, Result};
use serde::Deserialize;

use super::{capture_checked, run_checked};

#[derive(Deserialize)]
struct Listing {
    #[serde(default)]
    actions_caches: Vec<Entry>,
}

#[derive(Deserialize)]
struct Entry {
    id: u64,
    key: String,
}

/// The ids of every listed cache whose key is not `keep`.
pub fn stale_ids(json: &str, keep: &str) -> Result<Vec<u64>> {
    let listing: Listing = serde_json::from_str(json).context("parse the cache listing")?;
    Ok(listing
        .actions_caches
        .into_iter()
        .filter(|entry| entry.key != keep)
        .map(|entry| entry.id)
        .collect())
}

fn curl(token: &str) -> Command {
    let mut cmd = Command::new("curl");
    cmd.args(["-fsS", "-H"])
        .arg(format!("Authorization: Bearer {token}"))
        .args([
            "-H",
            "X-GitHub-Api-Version: 2022-11-28",
            "-H",
            "Accept: application/vnd.github+json",
        ]);
    cmd
}

pub fn run(prefix: &str, keep: &str) -> Result<()> {
    let repo = std::env::var("GITHUB_REPOSITORY").context("GITHUB_REPOSITORY is not set")?;
    let git_ref = std::env::var("GITHUB_REF").context("GITHUB_REF is not set")?;
    let token = std::env::var("GITHUB_TOKEN").context("GITHUB_TOKEN is not set")?;
    let api = format!("https://api.github.com/repos/{repo}/actions/caches");
    // `key` filters by prefix, `ref` by the ref the cache was saved under (refs/pull/N/merge for a PR).
    let listing = capture_checked(
        curl(&token)
            .args(["-G", &api])
            .args(["--data-urlencode", &format!("ref={git_ref}")])
            .args(["--data-urlencode", &format!("key={prefix}-")])
            .args(["--data-urlencode", "per_page=100"]),
    )?;
    for id in stale_ids(&listing, keep)? {
        println!("deleting the old cache {id} of {prefix} on {git_ref}");
        run_checked(
            curl(&token)
                .args(["-o", if cfg!(windows) { "NUL" } else { "/dev/null" }])
                .args(["-X", "DELETE", &format!("{api}/{id}")]),
        )?;
    }
    Ok(())
}
