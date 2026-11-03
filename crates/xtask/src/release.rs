//! `cargo xtask changelog` and `cargo xtask bump [auto|patch|minor|major|X.Y.Z]`: the commit
//! list of CHANGELOG.md and the next version from the Conventional Commits (git-cliff,
//! cliff.toml). See docs/workflow.md. Needs a git checkout with the history (a jj side
//! workspace has no `.git`).
//!
//! What counts as "since the last release": a release is squashed into `main`, so its tag
//! `vX.Y.Z` is not an ancestor of development. The boundary is the tag `release-base/vX.Y.Z`
//! on the tip of the release branch (`start`), which development reaches through the back-merge.
//! On a hotfix branch (from main) the nearest `vX.Y.Z` is the boundary.

use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

use crate::changelog::{self, EMPTY_LIST};

pub mod flow;
pub mod start;

/// The tag on the tip of a release branch: the boundary of "since the last release".
pub const BASE_TAG_PREFIX: &str = "release-base/v";

const METAINFO: &str = "packaging/flatpak/dev.mwt.MultiWalletTracker.metainfo.xml";

type Version = [u64; 3];

/// `X.Y.Z` of digits only; `None` for anything else (a tag `v1.2.3-4`, a `+meta`).
fn parse(text: &str) -> Option<Version> {
    let parts: Vec<&str> = text.split('.').collect();
    let [major, minor, patch] = parts.as_slice() else {
        return None;
    };
    let number = |part: &str| {
        (!part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
            .then(|| part.parse().ok())
            .flatten()
    };
    Some([number(major)?, number(minor)?, number(patch)?])
}

fn show(version: Version) -> String {
    format!("{}.{}.{}", version[0], version[1], version[2])
}

/// The newest `vX.Y.Z` among the tag names.
fn last_release(tags: &str) -> Option<Version> {
    tags.lines()
        .filter_map(|tag| parse(tag.strip_prefix('v')?))
        .max()
}

/// The version in a git-cliff tag name: `v1.2.3` and `release-base/v1.2.3` both give 1.2.3.
fn tag_version(text: &str) -> Option<Version> {
    let name = text.trim().rsplit('/').next()?;
    parse(name.strip_prefix('v').unwrap_or(name))
}

/// The version `level` asks for. `released` is the last release, `bumped` is what git-cliff
/// says for `auto` (only asked when needed).
fn next_version(
    level: &str,
    current: Version,
    released: Option<Version>,
    bumped: impl FnOnce() -> Result<Version>,
) -> Result<Version> {
    if let Some(explicit) = parse(level) {
        return Ok(explicit);
    }
    if level == "auto" {
        // The first release ships what the files say.
        if released.is_none() {
            return Ok(current);
        }
        let bumped = bumped()?;
        return Ok(if current > bumped { current } else { bumped });
    }
    let [major, minor, patch] = released.unwrap_or(current);
    match level {
        "major" => Ok([major + 1, 0, 0]),
        "minor" => Ok([major, minor + 1, 0]),
        "patch" => Ok([major, minor, patch + 1]),
        _ => bail!("unknown bump «{level}»: auto, patch, minor, major or X.Y.Z"),
    }
}

/// Replaces the first `{before}"<old>"{after}` value: `prefix` ends right before the opening
/// quote of the version. Anything but one found value is an error, never a silent skip.
fn replace_quoted(text: &str, prefix: &str, next: &str, what: &str) -> Result<String> {
    let Some(at) = text.find(prefix) else {
        bail!("{what}: nothing to update ({prefix:?})");
    };
    let from = at + prefix.len();
    let Some(len) = text[from..].find('"') else {
        bail!("{what}: unterminated version after {prefix:?}");
    };
    if len == 0 {
        bail!("{what}: empty version after {prefix:?}");
    }
    Ok(format!("{}{next}{}", &text[..from], &text[from + len..]))
}

pub fn set_tauri_version(text: &str, next: &str) -> Result<String> {
    // The top-level key (two spaces deep, whole line): nested objects do not match.
    const KEY: &str = "  \"version\": \"";
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let body = line.strip_suffix('\n').unwrap_or(line);
        if let Some(rest) = body.strip_prefix(KEY)
            && let Some(old) = rest.strip_suffix("\",")
            && !old.is_empty()
            && !old.contains('"')
        {
            let from = offset + KEY.len();
            return Ok(format!(
                "{}{next}{}",
                &text[..from],
                &text[from + old.len()..]
            ));
        }
        offset += line.len();
    }
    bail!("crates/app/tauri.conf.json: nothing to update (\"version\")")
}

pub fn set_workspace_version(text: &str, next: &str) -> Result<String> {
    replace_quoted(
        text,
        "[workspace.package]\nversion = \"",
        next,
        "Cargo.toml",
    )
}

pub fn set_lock_version(text: &str, krate: &str, next: &str) -> Result<String> {
    replace_quoted(
        text,
        &format!("name = \"{krate}\"\nversion = \""),
        next,
        "Cargo.lock",
    )
}

/// The `<release>` of `next` gets the date, or a new one opens the `<releases>` list.
pub fn set_metainfo_release(text: &str, next: &str, date: &str) -> Result<String> {
    let entry = format!("<release version=\"{next}\"");
    if let Some(at) = text.find(&entry) {
        let tail = &text[at..];
        let Some(close) = tail.find("/>") else {
            bail!("{METAINFO}: the <release> of {next} is not closed");
        };
        let fresh = format!("<release version=\"{next}\" date=\"{date}\"");
        if !tail[entry.len()..close].trim_start().starts_with("date=\"") {
            bail!("{METAINFO}: nothing to update (<release> of {next} has no date)");
        }
        return Ok(format!("{}{fresh}{}", &text[..at], &tail[close..]));
    }
    let Some(at) = text.find("<releases>\n") else {
        bail!("{METAINFO}: nothing to update (<releases>)");
    };
    let from = at + "<releases>\n".len();
    Ok(format!(
        "{}    <release version=\"{next}\" date=\"{date}\"/>\n{}",
        &text[..from],
        &text[from..]
    ))
}

fn require_git(root: &Path) -> Result<()> {
    let ok = Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .current_dir(root)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success());
    if !ok {
        bail!(
            "needs a git checkout with the history — run it in the main (colocated) checkout, not a jj side workspace"
        );
    }
    Ok(())
}

fn git_tags(root: &Path) -> Result<String> {
    let output = Command::new("git")
        .args(["tag", "--list", "v*"])
        .current_dir(root)
        .output()
        .context("run git tag")?;
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn cliff(root: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("pnpm")
        .args(["exec", "git-cliff", "--config", "cliff.toml"])
        .args(args)
        .current_dir(root)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .context("run pnpm exec git-cliff")?;
    if !output.status.success() {
        bail!("git-cliff {} failed ({})", args.join(" "), output.status);
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// The nearest release boundary in the history of HEAD: a `release-base/vX.Y.Z` or a release tag
/// `vX.Y.Z` (never a pre-release `vX.Y.Z-N`). `None` before the first release.
fn boundary(root: &Path) -> Option<String> {
    let output = Command::new("git")
        .args([
            "describe",
            "--tags",
            "--abbrev=0",
            "--match",
            "release-base/v[0-9]*.[0-9]*.[0-9]*",
            "--match",
            "v[0-9]*.[0-9]*.[0-9]*",
            "--exclude",
            "v*-*",
            "HEAD",
        ])
        .current_dir(root)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let tag = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (output.status.success() && !tag.is_empty()).then_some(tag)
}

/// The git-cliff arguments for the commits since `boundary`.
fn list_args(boundary: Option<&str>) -> Vec<String> {
    let mut args = match boundary {
        Some(tag) => vec![format!("{tag}..HEAD")],
        None => vec!["--unreleased".to_string()],
    };
    args.extend(["--strip".to_string(), "all".to_string()]);
    args
}

fn commit_list(root: &Path) -> Result<String> {
    let args = list_args(boundary(root).as_deref());
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let body = cliff(root, &args)?;
    Ok(if body.is_empty() {
        EMPTY_LIST.into()
    } else {
        body
    })
}

fn read(root: &Path, rel: &str) -> Result<String> {
    std::fs::read_to_string(root.join(rel)).with_context(|| format!("read {rel}"))
}

fn write(root: &Path, rel: &str, text: &str) -> Result<()> {
    std::fs::write(root.join(rel), text).with_context(|| format!("write {rel}"))
}

pub fn run_changelog(root: &Path) -> Result<()> {
    require_git(root)?;
    let text = changelog::with_commits(&read(root, "CHANGELOG.md")?, &commit_list(root)?)?;
    write(root, "CHANGELOG.md", &text)?;
    println!("CHANGELOG.md: [Unreleased] commit list refreshed");
    Ok(())
}

/// The next version for `level`, with a note for the log: nothing is written.
pub fn next_for(root: &Path, level: &str) -> Result<(String, String)> {
    require_git(root)?;
    let current_text = crate::version::tauri_version(root)?;
    let current = parse(&current_text)
        .with_context(|| format!("version {current_text} of tauri.conf.json is not X.Y.Z"))?;
    let released = last_release(&git_tags(root)?);
    let next = next_version(level, current, released, || {
        let text = cliff(root, &["--bumped-version"])?;
        tag_version(&text).with_context(|| format!("git-cliff said «{text}», not a version"))
    })?;
    if let Some(last) = released
        && next <= last
    {
        bail!(
            "{} is not newer than the last release {}",
            show(next),
            show(last)
        );
    }
    let note = released.map_or(" (first release)".into(), |last| {
        format!(" (last release {})", show(last))
    });
    Ok((
        show(next),
        format!("version {current_text} → {}{note}", show(next)),
    ))
}

/// Writes the version `next` (X.Y.Z) into every file and promotes `[Unreleased]` to its section.
pub fn write_bump(root: &Path, next: &str) -> Result<()> {
    let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
    // Everything is computed before the first write: a failing file leaves the tree untouched.
    let listed = changelog::with_commits(&read(root, "CHANGELOG.md")?, &commit_list(root)?)?;
    let mut edits = vec![
        ("CHANGELOG.md", changelog::promote(&listed, next, &date)?),
        (
            "crates/app/tauri.conf.json",
            set_tauri_version(&read(root, "crates/app/tauri.conf.json")?, next)?,
        ),
        (
            "Cargo.toml",
            set_workspace_version(&read(root, "Cargo.toml")?, next)?,
        ),
    ];
    let mut lock = read(root, "Cargo.lock")?;
    for krate in ["trade-app", "trade-core"] {
        lock = set_lock_version(&lock, krate, next)?;
    }
    edits.push(("Cargo.lock", lock));
    edits.push((
        METAINFO,
        set_metainfo_release(&read(root, METAINFO)?, next, &date)?,
    ));
    for (rel, text) in &edits {
        write(root, rel, text)?;
    }
    Ok(())
}

pub fn run_bump(root: &Path, level: &str) -> Result<()> {
    let (next, note) = next_for(root, level)?;
    write_bump(root, &next)?;
    println!("{note}");
    println!("commit it: jj describe -m \"chore(release): v{next}\"");
    Ok(())
}

#[cfg(test)]
mod tests;
