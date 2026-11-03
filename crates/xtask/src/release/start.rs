//! `cargo xtask release-start`: the first half of a release (docs/workflow.md, "Release").
//!
//! From development: a branch `release/vX.Y.Z` with the bump and the CHANGELOG section, the
//! commit `chore(release): vX.Y.Z`, the tag `release-base/vX.Y.Z` on its tip, a pull request into
//! `main` with auto-merge (squash). When green, GitHub squashes it into main and `release.yml`
//! publishes `vX.Y.Z`; `backmerge --merge` brings it back into development.
//!
//! `--dry-run` prints the steps and writes nothing (it only reads: git, `gh pr list`). A real run
//! is refused outside GitHub Actions: it switches branches and pushes.

use std::path::Path;

use anyhow::{Context, Result, bail};
use clap::Args;
use serde_json::Value;

use super::flow::{self, Step, gh, git};
use super::{BASE_TAG_PREFIX, next_for, require_git};
use crate::jobsummary;
use crate::publish::{has_token, repo};
use crate::version::release_remote;

const MAIN: &str = "main";
const DEVELOPMENT: &str = "development";
const RELEASE_BRANCH_PREFIX: &str = "release/v";

#[derive(Args)]
pub struct StartArgs {
    /// Print the steps, change nothing (no fetch, no push, no PR, no file written).
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub struct OpenPr {
    pub number: u64,
    pub head: String,
}

/// What the plan depends on; all of it is read before anything is written.
pub struct Facts<'a> {
    pub repo: &'a str,
    pub remote: &'a str,
    /// `X.Y.Z`.
    pub version: &'a str,
    /// The tip of development (a sha), where the release branch starts and `main` is created.
    pub base: &'a str,
    pub main_exists: bool,
    /// Open pull requests into main whose branch is `release/v*`.
    pub open_prs: Vec<OpenPr>,
    /// `release/vX.Y.Z` is on the remote.
    pub branch_exists: bool,
    /// `release-base/vX.Y.Z` is on the remote.
    pub tag_exists: bool,
}

pub fn branch_name(version: &str) -> String {
    format!("{RELEASE_BRANCH_PREFIX}{version}")
}

pub fn base_tag(version: &str) -> String {
    format!("{BASE_TAG_PREFIX}{version}")
}

fn title(version: &str) -> String {
    format!("chore(release): v{version}")
}

fn body(version: &str) -> String {
    format!(
        "Release v{version}, made by `cargo xtask release-start`. When the checks are green GitHub \
         squashes it into {MAIN} and release.yml publishes v{version}; the back-merge PR into \
         {DEVELOPMENT} follows. Red checks: the PR stays open and nothing is released."
    )
}

/// `gh pr list --json number,headRefName` output, only the release branches.
pub fn parse_open_prs(json: &str) -> Result<Vec<OpenPr>> {
    let items: Vec<Value> = serde_json::from_str(json).context("parse gh pr list")?;
    Ok(items
        .iter()
        .filter_map(|item| {
            let head = item["headRefName"].as_str()?;
            head.starts_with(RELEASE_BRANCH_PREFIX).then(|| OpenPr {
                number: item["number"].as_u64().unwrap_or_default(),
                head: head.to_string(),
            })
        })
        .collect())
}

/// The steps, in order. A pure function of the facts.
pub fn plan(facts: &Facts) -> Result<Vec<Step>> {
    let version = facts.version;
    let branch = branch_name(version);
    let tag = base_tag(version);
    let (remote, repo) = (facts.remote, facts.repo);
    if let Some(other) = facts.open_prs.iter().find(|pr| pr.head != branch) {
        bail!(
            "PR #{} ({}) is another open release, this run makes {branch}: close it first",
            other.number,
            other.head
        );
    }
    let mut steps = Vec::new();
    if !facts.main_exists {
        // The first release: main starts as development, the PR then carries the bump only.
        steps.push(git(&[
            "push",
            remote,
            &format!("{}:refs/heads/{MAIN}", facts.base),
        ]));
    }
    // The same version again: the old attempt goes, this one starts from the current development.
    for pr in &facts.open_prs {
        steps.push(gh(&[
            "pr",
            "close",
            &pr.number.to_string(),
            "--repo",
            repo,
            "--comment",
            "Superseded: the release is made again from the current development.",
        ]));
    }
    if facts.branch_exists {
        steps.push(git(&["push", remote, "--delete", &branch]));
    }
    if facts.tag_exists {
        steps.push(git(&[
            "push",
            remote,
            "--delete",
            &format!("refs/tags/{tag}"),
        ]));
    }
    steps.push(git(&["switch", "-C", &branch, facts.base]));
    steps.push(Step::Bump(version.to_string()));
    steps.push(flow::commit(&["--all"], &title(version)));
    steps.push(git(&["tag", "--force", &tag]));
    steps.push(git(&[
        "push",
        remote,
        &format!("refs/heads/{branch}"),
        &format!("refs/tags/{tag}"),
    ]));
    steps.push(gh(&[
        "pr",
        "create",
        "--repo",
        repo,
        "--base",
        MAIN,
        "--head",
        &branch,
        "--title",
        &title(version),
        "--body",
        &body(version),
    ]));
    steps.push(gh(&[
        "pr", "merge", &branch, "--repo", repo, "--auto", "--squash",
    ]));
    Ok(steps)
}

/// A read that may fail in a dry run (offline, no `gh` login): the failure is a warning and the
/// default is assumed. A real run stops.
fn read_or<T>(dry: bool, what: &str, fallback: T, read: impl FnOnce() -> Result<T>) -> Result<T> {
    match read() {
        Ok(value) => Ok(value),
        Err(error) if dry => {
            eprintln!("WARNING: {what}: {error:#} (dry run: assuming the default)");
            Ok(fallback)
        }
        Err(error) => Err(error.context(what.to_string())),
    }
}

pub fn run(root: &Path, args: &StartArgs) -> Result<()> {
    let dry = args.dry_run;
    require_git(root)?;
    if !dry {
        if std::env::var("GITHUB_ACTIONS").as_deref() != Ok("true") {
            bail!(
                "release-start switches branches and pushes: it runs in GitHub Actions only (release-start.yml); use --dry-run here"
            );
        }
        if !has_token() {
            bail!("GH_TOKEN is not set: the App token of release-start.yml is needed");
        }
    }
    let remote = release_remote(root);
    let development = format!("{remote}/{DEVELOPMENT}");
    if dry {
        println!("dry run: nothing is fetched, written, pushed or opened");
    } else {
        flow::execute(
            root,
            &[git(&["fetch", "--tags", "--force", &remote, DEVELOPMENT])],
            false,
        )?;
    }
    let base = match flow::query(root, "git", &["rev-parse", &development]) {
        Ok(sha) => sha,
        Err(error) if dry => {
            eprintln!("WARNING: no {development} ({error:#}); the plan starts from HEAD");
            flow::query(root, "git", &["rev-parse", "HEAD"])?
        }
        Err(error) => return Err(error),
    };
    if !dry && flow::query(root, "git", &["rev-parse", "HEAD"])? != base {
        bail!(
            "the checkout is not the tip of {development}: dispatch release-start.yml on development"
        );
    }
    let (version, note) = next_for(root, "auto")?;
    println!("release v{version}: {note}");

    let repo = repo();
    let branch = branch_name(&version);
    let tag = base_tag(&version);
    let main_ref = format!("refs/heads/{MAIN}");
    let facts = Facts {
        repo: &repo,
        remote: &remote,
        version: &version,
        base: &base,
        main_exists: read_or(dry, "cannot look for main on the remote", true, || {
            flow::remote_has(root, &remote, &main_ref)
        })?,
        open_prs: read_or(dry, "cannot list the open release PRs", Vec::new(), || {
            let json = flow::query(
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
                    MAIN,
                    "--json",
                    "number,headRefName",
                ],
            )?;
            parse_open_prs(&json)
        })?,
        branch_exists: read_or(dry, "cannot look for the release branch", false, || {
            flow::remote_has(root, &remote, &format!("refs/heads/{branch}"))
        })?,
        tag_exists: read_or(dry, "cannot look for the base tag", false, || {
            flow::remote_has(root, &remote, &format!("refs/tags/{tag}"))
        })?,
    };
    let steps = plan(&facts)?;
    flow::execute(root, &steps, dry)?;
    if !dry {
        jobsummary::append(&format!(
            "### release-start\n\nv{version}: PR `{branch}` -> `{MAIN}` is open with auto-merge (squash). Base tag `{tag}`.\n"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
