//! `cargo xtask publish | prune-prereleases | backmerge | publish-aur`: the release steps on GitHub
//! (gh), the AUR push. `--dry-run` prints the exact commands and writes nothing remote.

mod aur;
mod backmerge;
mod notes;
mod prune;
#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};
use clap::Args;
use sha2::{Digest, Sha256};

pub const DEFAULT_REPO: &str = "Reckeck/mwt";
const SUMS: &str = "SHA256SUMS";

#[derive(Args)]
pub struct PublishArgs {
    /// A pre-release (X.Y.Z-N) from development instead of a release (X.Y.Z) from main.
    #[arg(long)]
    pre: bool,
    #[arg(long)]
    version: String,
    /// The directory whose files go into the release.
    #[arg(long)]
    dir: PathBuf,
    /// Print the commands, run nothing against GitHub.
    #[arg(long)]
    dry_run: bool,
}

#[derive(Args)]
pub struct PruneArgs {
    /// Pre-releases to keep (default: $PRERELEASES_KEPT or 10).
    #[arg(long)]
    keep: Option<usize>,
    #[arg(long)]
    dry_run: bool,
}

#[derive(Args)]
pub struct BackmergeArgs {
    #[arg(long)]
    version: String,
    #[arg(long)]
    dry_run: bool,
}

#[derive(Args)]
pub struct AurArgs {
    #[arg(long)]
    version: String,
    /// The directory with the release tarball.
    #[arg(long)]
    dir: PathBuf,
    /// Print the steps; no network, no key.
    #[arg(long)]
    dry_run: bool,
}

/// `$GITHUB_REPOSITORY` or the default repository.
pub fn repo() -> String {
    std::env::var("GITHUB_REPOSITORY")
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_REPO.to_string())
}

pub fn has_token() -> bool {
    ["GH_TOKEN", "GITHUB_TOKEN"]
        .iter()
        .any(|name| std::env::var(name).is_ok_and(|value| !value.is_empty()))
}

fn quote(arg: &str) -> String {
    let plain = !arg.is_empty()
        && arg
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || "-_./:=@%+,".contains(ch));
    if plain {
        arg.to_string()
    } else {
        format!("'{}'", arg.replace('\'', "'\\''"))
    }
}

/// The command as a shell line, for dry runs and logs.
pub fn shell_line(program: &str, args: &[String]) -> String {
    let mut parts = vec![program.to_string()];
    parts.extend(args.iter().map(|arg| quote(arg)));
    parts.join(" ")
}

/// Prints `$ <command>` and, unless `dry`, runs it.
fn run_logged(program: &str, args: &[String], dry: bool) -> Result<()> {
    println!("$ {}", shell_line(program, args));
    if dry {
        return Ok(());
    }
    let status = Command::new(program)
        .args(args)
        .status()
        .with_context(|| format!("cannot start `{program}`"))?;
    if !status.success() {
        bail!("`{program}` exited with {status}");
    }
    Ok(())
}

fn capture(program: &str, args: &[String]) -> Result<String> {
    let output = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .with_context(|| format!("cannot start `{program}`"))?;
    if !output.status.success() {
        bail!(
            "`{}` failed: {}",
            shell_line(program, args),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn git(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|text| !text.is_empty())
}

/// `vX.Y.Z-N`.
pub fn is_prerelease_tag(tag: &str) -> bool {
    let Some((release, run)) = tag.strip_prefix('v').and_then(|rest| rest.split_once('-')) else {
        return false;
    };
    is_release_version(release) && is_number(run)
}

fn is_number(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit())
}

/// `X.Y.Z`.
pub fn is_release_version(text: &str) -> bool {
    let parts: Vec<&str> = text.split('.').collect();
    parts.len() == 3 && parts.iter().all(|part| is_number(part))
}

fn check_version(version: &str, pre: bool) -> Result<()> {
    let ok = if pre {
        is_prerelease_tag(&format!("v{version}"))
    } else {
        is_release_version(version)
    };
    if !ok {
        bail!(
            "version `{version}` is not {}",
            if pre { "X.Y.Z-N" } else { "X.Y.Z" }
        );
    }
    Ok(())
}

/// The files of a release directory (not the checksum list), sorted by name, with their bytes.
fn release_files(dir: &Path) -> Result<Vec<(String, Vec<u8>)>> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir).with_context(|| format!("read {}", dir.display()))? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if entry.file_type()?.is_file() && name != SUMS {
            files.push((name, std::fs::read(entry.path())?));
        }
    }
    files.sort_by(|left, right| left.0.cmp(&right.0));
    if files.is_empty() {
        bail!("{} has no files to release", dir.display());
    }
    Ok(files)
}

fn checksum_line(name: &str, bytes: &[u8]) -> String {
    format!("{:x}  {name}\n", Sha256::digest(bytes))
}

pub fn run_publish(root: &Path, args: &PublishArgs) -> Result<()> {
    check_version(&args.version, args.pre)?;
    let files = release_files(&args.dir)?;
    let sums: String = files
        .iter()
        .map(|(name, bytes)| checksum_line(name, bytes))
        .collect();
    std::fs::write(args.dir.join(SUMS), &sums).context("write SHA256SUMS")?;
    print!("{sums}");

    let repo = repo();
    let tag = format!("v{}", args.version);
    let sha = std::env::var("GITHUB_SHA")
        .ok()
        .filter(|value| !value.is_empty())
        .or_else(|| git(root, &["rev-parse", "HEAD"]))
        .context("no commit: GITHUB_SHA is unset and git rev-parse HEAD failed")?;
    let (title, text) = if args.pre {
        let since = git(
            root,
            &[
                "describe",
                "--tags",
                "--abbrev=0",
                "--match",
                "v[0-9]*",
                "--exclude",
                "v*-*",
                &sha,
            ],
        );
        let range = since
            .as_ref()
            .map_or(sha.clone(), |tag| format!("{tag}..{sha}"));
        let log = git(
            root,
            &[
                "log",
                "-n",
                "60",
                "--no-merges",
                "--format=%H%x1f%s",
                &range,
            ],
        )
        .unwrap_or_default();
        let branch = std::env::var("GITHUB_REF_NAME")
            .ok()
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "development".into());
        let text = notes::prerelease_notes(
            &repo,
            &branch,
            &sha,
            since.as_deref(),
            &notes::parse_log(&log),
        );
        (format!("Pre-release {}", args.version), text)
    } else {
        let changelog = std::fs::read_to_string(root.join("CHANGELOG.md")).unwrap_or_default();
        (
            format!("Multi Wallet Tracker {}", args.version),
            notes::release_notes(&changelog, &args.version, &repo),
        )
    };
    let notes_file = std::env::temp_dir().join(format!(
        "mwt-notes-{}-{}.md",
        args.version,
        std::process::id()
    ));
    std::fs::write(&notes_file, &text).context("write the release notes")?;
    println!("--- {title} ---\n{text}\n---");

    let names: Vec<String> = files
        .iter()
        .map(|(name, _)| args.dir.join(name).display().to_string())
        .chain([args.dir.join(SUMS).display().to_string()])
        .collect();
    let gh_args = notes::release_args(&notes::Release {
        repo: &repo,
        tag: &tag,
        files: &names,
        sha: &sha,
        title: &title,
        notes_file: &notes_file.display().to_string(),
        pre: args.pre,
    });
    let result = run_logged("gh", &gh_args, args.dry_run);
    let _ = std::fs::remove_file(&notes_file);
    result
}

pub use backmerge::run as run_backmerge;
pub use prune::run as run_prune;

pub fn run_aur(root: &Path, args: &AurArgs) -> Result<()> {
    aur::run(root, args)
}
