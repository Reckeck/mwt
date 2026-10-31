//! `cargo xtask prune-prereleases`: keep the newest pre-releases, delete the rest with their tags.

use anyhow::{Context, Result};
use serde::Deserialize;

use super::{PruneArgs, capture, has_token, is_prerelease_tag, repo, run_logged, shell_line};

const DEFAULT_KEPT: usize = 10;

/// The pre-release tags to delete: `tags` are newest first, `keep` pre-releases stay.
/// Only `vX.Y.Z-N` tags are ever picked, so a release `vX.Y.Z` cannot be deleted.
pub fn select(tags: &[String], keep: usize) -> Vec<String> {
    tags.iter()
        .filter(|tag| is_prerelease_tag(tag))
        .skip(keep)
        .cloned()
        .collect()
}

#[derive(Deserialize)]
struct Listed {
    #[serde(rename = "tagName")]
    tag_name: String,
}

pub fn list_args(repo: &str) -> Vec<String> {
    [
        "release",
        "list",
        "--repo",
        repo,
        "--limit",
        "200",
        "--order",
        "desc",
        "--json",
        "tagName,isPrerelease,createdAt",
    ]
    .map(String::from)
    .into()
}

pub fn delete_args(repo: &str, tag: &str) -> Vec<String> {
    [
        "release",
        "delete",
        tag,
        "--repo",
        repo,
        "--cleanup-tag",
        "--yes",
    ]
    .map(String::from)
    .into()
}

pub fn run(args: &PruneArgs) -> Result<()> {
    let keep = args
        .keep
        .or_else(|| {
            std::env::var("PRERELEASES_KEPT")
                .ok()
                .and_then(|value| value.parse().ok())
        })
        .unwrap_or(DEFAULT_KEPT);
    let repo = repo();
    let list = list_args(&repo);
    if !has_token() {
        eprintln!(
            "WARNING: GH_TOKEN is not set - old pre-releases stay (set it to keep only {keep})"
        );
        if args.dry_run {
            println!("$ {}", shell_line("gh", &list));
        }
        return Ok(());
    }
    // Reading the list is harmless, also in a dry run: it is what the deletions are chosen from.
    let json = capture("gh", &list)?;
    let listed: Vec<Listed> = serde_json::from_str(&json).context("parse gh release list")?;
    let tags: Vec<String> = listed.into_iter().map(|item| item.tag_name).collect();
    let old = select(&tags, keep);
    if old.is_empty() {
        println!("at most {keep} pre-releases: nothing to prune");
        return Ok(());
    }
    for tag in old {
        println!("pruning {tag}");
        if let Err(error) = run_logged("gh", &delete_args(&repo, &tag), args.dry_run) {
            eprintln!("  not deleted: {error:#}");
        }
    }
    Ok(())
}
