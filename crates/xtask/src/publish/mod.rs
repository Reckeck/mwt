//! `cargo xtask publish | prune-prereleases | backmerge | publish-aur`: the release steps on GitHub
//! (gh), the AUR push. `--dry-run` prints the exact commands and writes nothing remote.

mod aur;
mod backmerge;
mod manifest;
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
    /// A pre-release (X.Y.Z-N) from development instead of a release (X.Y.Z) from main. `PRE=true` does the same.
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

/// The most commits a pre-release lists; the compare link has the rest.
const LOG_LIMIT: usize = 80;

/// `PRE=true` (the release.yml output `pre`) means `--pre`; unset, empty or `false` does not.
pub fn pre_from_env(value: Option<&str>) -> bool {
    value == Some("true")
}

pub fn run_publish(root: &Path, args: &PublishArgs) -> Result<()> {
    let args = &PublishArgs {
        pre: args.pre || pre_from_env(std::env::var("PRE").ok().as_deref()),
        version: args.version.clone(),
        dir: args.dir.clone(),
        dry_run: args.dry_run,
    };
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
        // The closest release tag of any kind in this commit's history: the previous pre-release
        // or release. A tag outside the history (another branch, a rewritten past) is not a
        // boundary, and the tag being published is skipped when a rerun finds it on this commit.
        let since = git(
            root,
            &[
                "describe",
                "--tags",
                "--abbrev=0",
                "--match",
                "v[0-9]*",
                "--exclude",
                &tag,
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
                &(LOG_LIMIT + 1).to_string(),
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
        let mut commits = notes::parse_log(&log);
        let more = commits.len() > LOG_LIMIT;
        commits.truncate(LOG_LIMIT);
        let text = notes::prerelease_notes(&repo, &branch, &sha, since.as_deref(), &commits, more);
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

    let manifest_path = write_manifest(&args.dir, &files, &repo, &tag, &args.version, &title)?;
    let names: Vec<String> = files
        .iter()
        .map(|(name, _)| args.dir.join(name).display().to_string())
        .chain([
            args.dir.join(SUMS).display().to_string(),
            manifest_path.display().to_string(),
        ])
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
    result?;
    if args.pre {
        publish_dev_manifest(&repo, &sha, &manifest_path, args.dry_run)?;
    }
    Ok(())
}

/// Builds the updater manifest from the `.sig` files of `files` and writes it next to them.
fn write_manifest(
    dir: &Path,
    files: &[(String, Vec<u8>)],
    repo: &str,
    tag: &str,
    version: &str,
    title: &str,
) -> Result<PathBuf> {
    let names: Vec<String> = files.iter().map(|(name, _)| name.clone()).collect();
    let signatures = manifest::signed_names(&names)
        .into_iter()
        .filter_map(|bundle| {
            let sig = files
                .iter()
                .find(|(name, _)| *name == format!("{bundle}.sig"))?;
            Some((bundle, String::from_utf8_lossy(&sig.1).into_owned()))
        })
        .collect();
    let (value, warnings) = manifest::build(&manifest::Input {
        repo,
        tag,
        version,
        notes: title,
        pub_date: &chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        names: &names,
        signatures: &signatures,
    });
    for warning in warnings {
        eprintln!("WARNING: {warning}");
    }
    let path = dir.join(manifest::file_name(version.contains('-')));
    std::fs::write(&path, serde_json::to_string_pretty(&value)? + "\n")
        .context("write the manifest")?;
    println!(
        "manifest {}:\n{}",
        path.display(),
        std::fs::read_to_string(&path)?
    );
    Ok(path)
}

/// The dev channel reads its manifest from the `updater-dev` pre-release: create it or replace the file.
fn publish_dev_manifest(repo: &str, sha: &str, file: &Path, dry: bool) -> Result<()> {
    let file = file.display().to_string();
    let view = ["release", "view", manifest::DEV_RELEASE_TAG, "--repo", repo].map(String::from);
    let exists = !dry
        && Command::new("gh")
            .args(&view)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success());
    run_logged(
        "gh",
        &manifest::dev_release_args(repo, sha, &file, exists),
        dry,
    )
}

pub use backmerge::run as run_backmerge;
pub use prune::run as run_prune;

pub fn run_aur(root: &Path, args: &AurArgs) -> Result<()> {
    aur::run(root, args)
}
