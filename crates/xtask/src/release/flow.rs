//! The plumbing `release-start` and `backmerge --merge` share: a list of steps that is either
//! printed (`--dry-run`) or run, and the read-only git/gh queries that decide the list.

use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

use crate::publish::shell_line;

/// The identity of the commits CI makes (the App token only authenticates the push).
pub const BOT_NAME: &str = "github-actions[bot]";
pub const BOT_EMAIL: &str = "41898282+github-actions[bot]@users.noreply.github.com";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// An external command, run in the repository root.
    Run {
        program: &'static str,
        args: Vec<String>,
    },
    /// The version files and CHANGELOG.md of the working tree become `X.Y.Z`.
    Bump(String),
}

fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|arg| (*arg).to_string()).collect()
}

pub fn git(args: &[&str]) -> Step {
    Step::Run {
        program: "git",
        args: strings(args),
    }
}

pub fn gh(args: &[&str]) -> Step {
    Step::Run {
        program: "gh",
        args: strings(args),
    }
}

/// `git commit` with the bot identity (a CI checkout has none configured).
pub fn commit(extra: &[&str], message: &str) -> Step {
    let mut args = strings(&[
        "-c",
        &format!("user.name={BOT_NAME}"),
        "-c",
        &format!("user.email={BOT_EMAIL}"),
    ]);
    args.push("commit".into());
    args.extend(strings(extra));
    args.extend(strings(&["-m", message]));
    Step::Run {
        program: "git",
        args,
    }
}

/// `git merge` with the bot identity.
pub fn merge(extra: &[&str], message: &str, reference: &str) -> Step {
    let mut args = strings(&[
        "-c",
        &format!("user.name={BOT_NAME}"),
        "-c",
        &format!("user.email={BOT_EMAIL}"),
    ]);
    args.push("merge".into());
    args.extend(strings(extra));
    args.extend(strings(&["-m", message, reference]));
    Step::Run {
        program: "git",
        args,
    }
}

pub fn describe(step: &Step) -> String {
    match step {
        Step::Run { program, args } => format!("$ {}", shell_line(program, args)),
        Step::Bump(version) => format!("(edit) version files and CHANGELOG.md -> {version}"),
    }
}

/// Prints every step and, unless `dry`, runs it; the first failure stops the list.
pub fn execute(root: &Path, steps: &[Step], dry: bool) -> Result<()> {
    for step in steps {
        println!("{}", describe(step));
        if dry {
            continue;
        }
        match step {
            Step::Bump(version) => super::write_bump(root, version)?,
            Step::Run { program, args } => {
                let status = Command::new(program)
                    .args(args)
                    .current_dir(root)
                    .status()
                    .with_context(|| format!("cannot start `{program}`"))?;
                if !status.success() {
                    bail!("`{}` exited with {status}", shell_line(program, args));
                }
            }
        }
    }
    Ok(())
}

/// Stdout of a read-only command; a failure is an error with its stderr.
pub fn query(root: &Path, program: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(program)
        .args(args)
        .current_dir(root)
        .stdin(Stdio::null())
        .output()
        .with_context(|| format!("cannot start `{program}`"))?;
    if !output.status.success() {
        bail!(
            "`{program} {}` failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Whether a git command succeeds (`merge-base --is-ancestor`, `rev-parse --verify`).
pub fn succeeds(root: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .args(args)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// Whether `reference` (`refs/heads/main`, `refs/tags/...`) exists on the remote. An unreachable
/// remote is an error: guessing "no" could make a second `main` or a second release.
pub fn remote_has(root: &Path, remote: &str, reference: &str) -> Result<bool> {
    Ok(!query(root, "git", &["ls-remote", remote, reference])?.is_empty())
}

/// The published tag closest to `rev`: among the tags matching `patterns` that are ancestors of
/// `rev` (and are not `skip`ped), the one with the fewest commits in `tag..rev`, because that is
/// the list a release note would have to show. `git describe` cannot be used for this: it ranks
/// candidates by commit date, so a squash tag created after the release tip (always the case for
/// `vX.Y.Z` against `release-base/vX.Y.Z`) can win although it is farther away. A tie goes to a
/// `release-base/` tag, then to the smaller name, so the result depends on neither dates nor the
/// user's `tag.sort`. Costs one `git tag` plus one `git rev-list --count` per candidate tag.
pub fn nearest_tag(
    root: &Path,
    rev: &str,
    patterns: &[&str],
    skip: impl Fn(&str) -> bool,
) -> Option<String> {
    let listing = query(
        root,
        "git",
        &[&["tag", "--merged", rev, "--list"], patterns].concat(),
    )
    .ok()?;
    listing
        .lines()
        .map(str::trim)
        .filter(|tag| !tag.is_empty() && !skip(tag))
        .filter_map(|tag| {
            let range = format!("{tag}..{rev}");
            let count: u64 = query(root, "git", &["rev-list", "--count", &range])
                .ok()?
                .parse()
                .ok()?;
            Some((count, !tag.starts_with("release-base/"), tag))
        })
        .min()
        .map(|(_, _, tag)| tag.to_string())
}
