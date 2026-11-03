//! `cargo xtask backmerge`: after a release from main bring main back into development.
//!
//! Without `--merge`: only a PR main -> development (a plain merge; with a squashed release it
//! conflicts on what development already has). With `--merge` (what release.yml runs):
//!
//! - after a `release-start` release: a branch `backmerge/vX.Y.Z` = the release tip (the tag
//!   `release-base/vX.Y.Z`) with main merged in by `-s ours`: main is the squash of exactly that
//!   tip, so the merge only records the squash commit as an ancestor, and development then
//!   reaches both `release-base/vX.Y.Z` and the release tag. Done only when the trees are equal.
//! - after a hotfix (no `release-base` tag for the version): a plain merge of main into
//!   development.
//!
//! Either way a PR into development with auto-merge (a merge commit). A conflict leaves the plain
//! PR main -> development open for a human and fails the job with the reason in the Job Summary.

use std::path::Path;

use anyhow::{Context, Result, bail};
use serde_json::Value;

use super::{BackmergeArgs, capture, check_version, has_token, repo, run_logged, shell_line};
use crate::jobsummary;
use crate::release::flow::{self, Step, gh, git};
use crate::release::start::base_tag;
use crate::version::release_remote;

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

/// How main goes into development.
#[derive(Debug, PartialEq, Eq)]
pub enum Mode {
    /// `-s ours` on top of the release tip: main is the squash of it.
    Ours,
    /// An ordinary merge of main into development (a hotfix).
    Plain,
}

/// `base_tree` is the tree of `release-base/vX.Y.Z` (none: no such tag), `main_tree` the tree of
/// main. A release made by release-start squashes exactly its tip into main, so the trees are
/// equal; main with other content (a hotfix landed meanwhile, the PR was edited) is not merged
/// away by guesswork.
pub fn decide(base_tree: Option<&str>, main_tree: &str) -> Result<Mode, String> {
    match base_tree {
        None => Ok(Mode::Plain),
        Some(tree) if tree == main_tree => Ok(Mode::Ours),
        Some(_) => Err(
            "the tree of main differs from the tip of the release branch (release-base): \
            main has content the release did not; merge it into development by hand"
                .into(),
        ),
    }
}

pub fn branch_name(version: &str) -> String {
    format!("backmerge/v{version}")
}

/// The steps up to the merge commit on the branch `backmerge/vX.Y.Z`.
pub fn merge_steps(mode: &Mode, remote: &str, version: &str) -> Vec<Step> {
    let branch = branch_name(version);
    let main = format!("{remote}/main");
    match mode {
        Mode::Ours => vec![
            git(&["switch", "-C", &branch, &base_tag(version)]),
            flow::merge(
                &["-s", "ours"],
                &format!("Merge main (the squash of release v{version}) into {branch}"),
                &main,
            ),
        ],
        Mode::Plain => vec![
            git(&["switch", "-C", &branch, &format!("{remote}/development")]),
            flow::merge(
                &[],
                &format!("Merge main into development after v{version}"),
                &main,
            ),
        ],
    }
}

/// Push the branch and open the PR (unless one is open), then auto-merge with a merge commit.
pub fn pr_steps(repo: &str, version: &str, remote: &str, pr_open: bool) -> Vec<Step> {
    let branch = branch_name(version);
    let mut steps = vec![git(&[
        "push",
        "--force",
        remote,
        &format!("refs/heads/{branch}"),
    ])];
    if !pr_open {
        let title = format!("chore: back-merge v{version} into development");
        let body = format!(
            "Brings main (release v{version}) back into development, made by `cargo xtask backmerge --merge`. Auto-merged with a merge commit when the checks are green."
        );
        steps.push(gh(&[
            "pr",
            "create",
            "--repo",
            repo,
            "--base",
            "development",
            "--head",
            &branch,
            "--title",
            &title,
            "--body",
            &body,
        ]));
    }
    steps.push(gh(&[
        "pr", "merge", &branch, "--repo", repo, "--auto", "--merge",
    ]));
    steps
}

/// A conflict: main stays unmerged, the plain PR main -> development is the place to resolve it.
fn conflict(root: &Path, repo: &str, error: &anyhow::Error, version: &str) -> Result<()> {
    let _ = flow::execute(root, &[git(&["merge", "--abort"])], false);
    let list = list_args(repo, "main", "development");
    let open = capture("gh", &list)
        .ok()
        .and_then(|text| serde_json::from_str::<Vec<Value>>(&text).ok())
        .is_some_and(|prs| !prs.is_empty());
    if !open {
        let _ = run_logged(
            "gh",
            &create_args(repo, "main", "development", version),
            false,
        );
    }
    let reason = format!("{error:#}");
    jobsummary::append(&format!(
        "### backmerge v{version}: needs a human\n\nMerging main into development failed: {reason}\n\nThe PR main -> development is open: resolve the conflict there (only version and CHANGELOG lines can differ).\n"
    ));
    bail!("main does not merge into development: {reason}")
}

fn run_merge(root: &Path, args: &BackmergeArgs) -> Result<()> {
    check_version(&args.version, false)?;
    let dry = args.dry_run;
    if !dry && !has_token() {
        bail!("GH_TOKEN is not set: the App token of release.yml is needed");
    }
    let remote = release_remote(root);
    let (main, development) = (format!("{remote}/main"), format!("{remote}/development"));
    if !dry {
        flow::execute(
            root,
            &[git(&[
                "fetch",
                "--tags",
                "--force",
                &remote,
                "main",
                "development",
            ])],
            false,
        )?;
    }
    if flow::succeeds(root, &["merge-base", "--is-ancestor", &main, &development]) {
        println!("development already has main: nothing to merge back");
        return Ok(());
    }
    let tag = base_tag(&args.version);
    let tag_ref = format!("refs/tags/{tag}");
    let base_tree = if flow::succeeds(root, &["rev-parse", "-q", "--verify", &tag_ref]) {
        Some(flow::query(
            root,
            "git",
            &["rev-parse", &format!("{tag_ref}^{{tree}}")],
        )?)
    } else {
        None
    };
    let main_tree = flow::query(root, "git", &["rev-parse", &format!("{main}^{{tree}}")])?;
    let mode = decide(base_tree.as_deref(), &main_tree).map_err(anyhow::Error::msg)?;
    println!("v{}: {mode:?} merge of main into development", args.version);

    let repo = repo();
    let branch = branch_name(&args.version);
    let pr_open = flow::query(
        root,
        "gh",
        &[
            "pr",
            "list",
            "--repo",
            &repo,
            "--state",
            "open",
            "--base",
            "development",
            "--head",
            &branch,
            "--json",
            "number",
        ],
    )
    .ok()
    .and_then(|text| serde_json::from_str::<Vec<Value>>(&text).ok())
    .is_some_and(|prs| !prs.is_empty());
    let steps = merge_steps(&mode, &remote, &args.version);
    let (switch, merge) = steps.split_at(1);
    flow::execute(root, switch, dry)?;
    if let Err(error) = flow::execute(root, merge, dry) {
        return conflict(root, &repo, &error, &args.version);
    }
    flow::execute(root, &pr_steps(&repo, &args.version, &remote, pr_open), dry)
}

pub fn run(root: &Path, args: &BackmergeArgs) -> Result<()> {
    if args.merge {
        return run_merge(root, args);
    }
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

#[cfg(test)]
mod tests;
