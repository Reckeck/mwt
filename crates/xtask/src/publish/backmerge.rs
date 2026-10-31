//! `cargo xtask backmerge`: after a release from main open the pull request main -> development.

use anyhow::{Context, Result};
use serde_json::Value;

use super::{BackmergeArgs, capture, has_token, repo, run_logged, shell_line};

fn branch(name: &str, default: &str) -> String {
    std::env::var(name)
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default.to_string())
}

pub fn list_args(repo: &str, source: &str, target: &str) -> Vec<String> {
    [
        "pr", "list", "--repo", repo, "--state", "open", "--base", target, "--head", source,
        "--json", "number",
    ]
    .map(String::from)
    .into()
}

pub fn create_args(repo: &str, source: &str, target: &str, version: &str) -> Vec<String> {
    let title = format!("Back-merge {source} into {target} after v{version}");
    let body = format!(
        "Release v{version} is out. Merge it back so {target} carries it, then raise the version on {target} to the next one."
    );
    [
        "pr", "create", "--repo", repo, "--base", target, "--head", source, "--title", &title,
        "--body", &body,
    ]
    .map(String::from)
    .into()
}

pub fn run(args: &BackmergeArgs) -> Result<()> {
    let source = branch("PROD_BRANCH", "main");
    let target = branch("DEV_BRANCH", "development");
    let repo = repo();
    let list = list_args(&repo, &source, &target);
    let create = create_args(&repo, &source, &target, &args.version);
    if args.dry_run {
        println!("$ {}", shell_line("gh", &list));
        println!("(only when it lists no open PR)");
        return run_logged("gh", &create, true);
    }
    if !has_token() {
        eprintln!("WARNING: GH_TOKEN is not set - open the PR {source} -> {target} by hand");
        return Ok(());
    }
    let open: Vec<Value> =
        serde_json::from_str(&capture("gh", &list)?).context("parse gh pr list")?;
    if !open.is_empty() {
        println!("a PR {source} -> {target} is already open");
        return Ok(());
    }
    if let Err(error) = run_logged("gh", &create, false) {
        eprintln!(
            "could not open the PR (nothing to merge back?) - check {source} -> {target} by hand: {error:#}"
        );
    }
    Ok(())
}
