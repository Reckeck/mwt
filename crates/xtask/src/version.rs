//! The version a build ships under, and the checks that keep it honest (docs/workflow.md).
//!
//! Channels:
//! - prod: `X.Y.Z`, main: a release; the tag `vX.Y.Z` must not exist yet
//! - dev: `X.Y.Z-<N>`, development: a pre-release of the coming X.Y.Z; once X.Y.Z is released
//!   (and `mise run bump` has not run yet) of X.Y.(Z+1). `N` is the highest published
//!   `vX.Y.Z-N` tag of that version plus one (1 for the first), so published numbers run
//!   in a row whatever the CI runs did; `--run` overrides it
//! - none: `X.Y.Z-<run>`, builds that publish nothing (manual, scheduled)
//!
//! `<run>` is the run counter of the CI. It is also the Android versionCode, which only has to
//! grow, unlike the pre-release number, which starts again at 1 with every new X.Y.Z.
//! The pre-release part is numeric: MSI accepts nothing else (max 65535).

use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

use crate::channel::Channel;

/// What an MSI version can hold in its last field.
pub const MAX_RUN: u64 = 65535;

/// The release channel: the one value set of every `--channel` (`dev`, `prod`, `none`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Kind {
    Prod,
    Dev,
    /// A build that publishes nothing: the dev names, a `X.Y.Z-<run>` version.
    None,
}

impl Kind {
    /// The names a build ships under: `none` has the dev ones.
    pub fn names(self) -> Channel {
        match self {
            Self::Prod => Channel::Prod,
            Self::Dev | Self::None => Channel::Dev,
        }
    }

    /// For a command that publishes into a channel folder: `none` has none.
    pub fn publishing(self, command: &str) -> Result<Channel> {
        if self == Self::None {
            bail!(
                "`{command}` cannot take `--channel none`: a build needs a channel to ship under (dev or prod)"
            );
        }
        Ok(self.names())
    }

    fn label(self) -> &'static str {
        match self {
            Self::Prod => "prod",
            Self::Dev => "dev",
            Self::None => "none",
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Computed {
    pub build: String,
    /// The Android versionCode: always the run number (it only has to grow), never the
    /// pre-release number, which restarts at 1 with each new X.Y.Z.
    pub code: u64,
    /// Said on stderr (a pre-release aimed past a released version).
    pub note: Option<String>,
}

/// `X.Y.Z` as numbers; anything else (a pre-release, a missing part) is an error.
pub fn parse_plain(version: &str) -> Result<[u64; 3]> {
    let bad = || {
        anyhow::anyhow!(
            "version {version} must be plain X.Y.Z; CI adds the pre-release part itself"
        )
    };
    let parts: Vec<&str> = version.split('.').collect();
    let [major, minor, patch] = parts.as_slice() else {
        return Err(bad());
    };
    let number = |text: &str| -> Result<u64> {
        if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(bad());
        }
        text.parse().map_err(|_| bad())
    };
    Ok([number(major)?, number(minor)?, number(patch)?])
}

/// The run number: the argument, else the run counter of GitHub Actions
/// (`GITHUB_RUN_NUMBER`), else 1.
pub fn run_number(arg: Option<u64>, github: Option<&str>) -> Result<u64> {
    if let Some(number) = arg {
        return Ok(number);
    }
    match github.filter(|value| !value.is_empty()) {
        Some(text) => text
            .parse()
            .with_context(|| format!("run number `{text}` is not a number")),
        None => Ok(1),
    }
}

/// Tag names (without `v`) out of `git ls-remote --tags` output. The peeled `^{}` lines of
/// annotated tags repeat their tag and are skipped.
fn tag_names(ls_remote: &str) -> Vec<&str> {
    ls_remote
        .lines()
        .filter_map(|line| line.split_whitespace().nth(1))
        .filter(|reference| !reference.ends_with("^{}"))
        .filter_map(|reference| reference.strip_prefix("refs/tags/v"))
        .collect()
}

/// The next pre-release number of `base`: the highest `<base>-<N>` tag plus one, else 1.
fn next_pre_release(tags: &[&str], base: &str) -> u64 {
    tags.iter()
        .filter_map(|tag| tag.strip_prefix(base)?.strip_prefix('-'))
        .filter(|digits| !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit()))
        .filter_map(|digits| digits.parse::<u64>().ok())
        .max()
        .map_or(1, |highest| highest + 1)
}

/// `version` from tauri.conf.json against `cargo_version` from Cargo.toml. `run` is the CI run
/// number (the Android versionCode, and the pre-release number of `none` builds); `pinned` says
/// it came from `--run`, which then also fixes the pre-release number of `dev`. `tags` returns
/// the `git ls-remote --tags` output of the release remote; it runs only when the channel
/// publishes.
pub fn compute(
    kind: Kind,
    version: &str,
    cargo_version: &str,
    run: u64,
    pinned: bool,
    changelog: &str,
    tags: impl FnOnce() -> Result<String>,
) -> Result<Computed> {
    if cargo_version != version {
        bail!(
            "Cargo.toml workspace version {cargo_version} differs from tauri.conf.json {version}"
        );
    }
    let [major, minor, patch] = parse_plain(version)?;
    let mut note = None;
    let mut base = version.to_string();
    let mut pre = run;
    if kind != Kind::None {
        let listing = tags()?;
        let names = tag_names(&listing);
        // A released version is final: main must move on (mise run bump), and the
        // pre-releases of development aim past it until the next bump.
        if names.contains(&version) {
            if kind == Kind::Prod {
                bail!(
                    "v{version} is already released: run `mise run bump` on development before merging into main"
                );
            }
            base = format!("{major}.{minor}.{}", patch + 1);
            note = Some(format!(
                "v{version} is released; this pre-release is of {base}"
            ));
        }
        if kind == Kind::Dev && !pinned {
            pre = next_pre_release(&names, &base);
        }
    }
    let build = match kind {
        Kind::Prod => base,
        Kind::Dev | Kind::None => {
            if pre > MAX_RUN {
                bail!("pre-release number {pre} exceeds what an MSI version holds ({MAX_RUN})");
            }
            format!("{base}-{pre}")
        }
    };
    // A release carries its CHANGELOG section (mise run bump writes it).
    if kind == Kind::Prod
        && !changelog
            .lines()
            .any(|line| line.starts_with(&format!("## [{version}]")))
    {
        bail!("CHANGELOG.md has no «## [{version}]» section: run `mise run bump` on development");
    }
    Ok(Computed {
        build,
        code: run,
        note,
    })
}

/// `"version"` of crates/app/tauri.conf.json.
pub fn tauri_version(root: &Path) -> Result<String> {
    let path = root.join("crates/app/tauri.conf.json");
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let json: serde_json::Value = serde_json::from_str(&text)?;
    match json["version"].as_str() {
        Some(version) if !version.is_empty() => Ok(version.to_string()),
        _ => bail!("no version in crates/app/tauri.conf.json"),
    }
}

/// `version` of `[workspace.package]` in a Cargo.toml text.
pub fn workspace_version(cargo_toml: &str) -> Option<String> {
    let mut inside = false;
    for line in cargo_toml.lines() {
        if line.starts_with('[') {
            inside = line.trim_end() == "[workspace.package]";
        } else if inside && let Some(rest) = line.strip_prefix("version = \"") {
            return Some(rest.split('"').next()?.to_string());
        }
    }
    None
}

/// The remote the release tags live on: `MWT_RELEASE_REMOTE`, else `github` (a working copy
/// whose `origin` is not GitHub, e.g. a mirror), else `origin` (in GitHub Actions the checkout's
/// origin is GitHub itself, and there is no `github` remote).
pub fn release_remote(root: &Path) -> String {
    if let Some(remote) = std::env::var("MWT_RELEASE_REMOTE")
        .ok()
        .filter(|value| !value.is_empty())
    {
        return remote;
    }
    let remotes = Command::new("git")
        .args(["remote"])
        .current_dir(root)
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
        .unwrap_or_default();
    if remotes.lines().any(|line| line == "github") {
        "github".into()
    } else {
        "origin".into()
    }
}

/// The `git ls-remote --tags` output for `v*` on the release remote. An unreachable remote is
/// an error: guessing "no tags" could let a second release of the same version through.
fn remote_tags(root: &Path) -> Result<String> {
    let remote = release_remote(root);
    let output = Command::new("git")
        .args(["ls-remote", "--tags", &remote, "refs/tags/v*"])
        .current_dir(root)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .context("run git ls-remote")?;
    if !output.status.success() {
        bail!("cannot list the tags of the remote `{remote}` (MWT_RELEASE_REMOTE names another)");
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub struct Args {
    pub channel: Option<Kind>,
    pub run: Option<u64>,
    pub output: Option<String>,
}

/// `cargo xtask version`: the app version, or with a channel the version a build ships under.
pub fn run(root: &Path, args: &Args) -> Result<()> {
    let version = tauri_version(root)?;
    let Some(kind) = args.channel else {
        println!("{version}");
        return Ok(());
    };
    let cargo_toml = std::fs::read_to_string(root.join("Cargo.toml"))?;
    let cargo_version = workspace_version(&cargo_toml).unwrap_or_default();
    let changelog = if kind == Kind::Prod {
        std::fs::read_to_string(root.join("CHANGELOG.md")).context("read CHANGELOG.md")?
    } else {
        String::new()
    };
    let run = run_number(args.run, std::env::var("GITHUB_RUN_NUMBER").ok().as_deref())?;
    let computed = compute(
        kind,
        &version,
        &cargo_version,
        run,
        args.run.is_some(),
        &changelog,
        || remote_tags(root),
    )?;
    if let Some(note) = &computed.note {
        eprintln!("{note}");
    }
    if let Some(path) = &args.output {
        let text = format!(
            "BUILD_VERSION={}\nANDROID_VERSION_CODE={}\nRELEASE_CHANNEL={}\n",
            computed.build,
            computed.code,
            kind.label()
        );
        // Appended: $GITHUB_OUTPUT collects the lines of every step.
        use std::io::Write;
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .and_then(|mut file| file.write_all(text.as_bytes()))
            .with_context(|| format!("write {path}"))?;
    }
    println!("{}", computed.build);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHANGELOG: &str = "# Changelog\n\n## [Unreleased]\n\n## [1.2.3] - 2026-01-01\n";

    fn listing(names: &[&str]) -> String {
        names
            .iter()
            .map(|name| format!("0123abcd\trefs/tags/v{name}\n"))
            .collect()
    }

    fn with_tags(kind: Kind, run: u64, pinned: bool, tags: &[&str]) -> Result<Computed> {
        compute(kind, "1.2.3", "1.2.3", run, pinned, CHANGELOG, || {
            Ok(listing(tags))
        })
    }

    fn go(kind: Kind, run: u64, tag: bool, changelog: &str) -> Result<Computed> {
        let tags: &[&str] = if tag { &["1.2.3"] } else { &[] };
        compute(kind, "1.2.3", "1.2.3", run, true, changelog, || {
            Ok(listing(tags))
        })
    }

    #[test]
    fn dev_is_a_pre_release_of_the_coming_version() {
        let done = go(Kind::Dev, 7, false, "").unwrap();
        assert_eq!(done.build, "1.2.3-7");
        assert_eq!(done.code, 7);
        assert_eq!(done.note, None);
    }

    #[test]
    fn dev_aims_past_a_released_version() {
        let done = go(Kind::Dev, 7, true, "").unwrap();
        assert_eq!(done.build, "1.2.4-7");
        assert!(done.note.unwrap().contains("1.2.4"));
    }

    #[test]
    fn prod_is_the_plain_version() {
        assert_eq!(go(Kind::Prod, 7, false, CHANGELOG).unwrap().build, "1.2.3");
    }

    #[test]
    fn prod_with_an_existing_tag_is_an_error() {
        let error = go(Kind::Prod, 7, true, CHANGELOG).unwrap_err().to_string();
        assert!(error.contains("already released"), "{error}");
    }

    #[test]
    fn prod_needs_its_changelog_section() {
        let error = go(Kind::Prod, 1, false, "## [Unreleased]\n")
            .unwrap_err()
            .to_string();
        assert!(error.contains("no «## [1.2.3]» section"), "{error}");
    }

    #[test]
    fn none_never_asks_the_remote() {
        let done = compute(Kind::None, "1.2.3", "1.2.3", 3, false, "", || {
            bail!("asked")
        })
        .unwrap();
        assert_eq!(done.build, "1.2.3-3");
    }

    #[test]
    fn run_limit_is_65535() {
        assert_eq!(
            go(Kind::Dev, 65535, false, "").unwrap().build,
            "1.2.3-65535"
        );
        let error = go(Kind::Dev, 65536, false, "").unwrap_err().to_string();
        assert!(
            error.contains("exceeds what an MSI version holds (65535)"),
            "{error}"
        );
    }

    #[test]
    fn versions_must_agree_and_be_plain() {
        let error = compute(Kind::Dev, "1.2.3", "1.2.4", 1, false, "", || {
            Ok(String::new())
        })
        .unwrap_err();
        assert!(error.to_string().contains("differs"));
        assert!(parse_plain("1.2.3-4").is_err());
        assert!(parse_plain("1.2").is_err());
        assert!(parse_plain("1.2.x").is_err());
        assert_eq!(parse_plain("10.0.3").unwrap(), [10, 0, 3]);
    }

    #[test]
    fn run_number_sources_in_order() {
        assert_eq!(run_number(Some(5), Some("9")).unwrap(), 5);
        assert_eq!(run_number(None, Some("9")).unwrap(), 9);
        assert_eq!(run_number(None, None).unwrap(), 1);
        assert!(run_number(None, Some("x")).is_err());
    }

    #[test]
    fn dev_without_tags_starts_at_one() {
        let done = with_tags(Kind::Dev, 99, false, &[]).unwrap();
        assert_eq!(done.build, "1.2.3-1");
        assert_eq!(done.code, 99);
    }

    #[test]
    fn dev_continues_after_the_highest_number() {
        let done = with_tags(Kind::Dev, 99, false, &["1.2.3-9", "1.2.3-10", "1.2.3-15"]).unwrap();
        assert_eq!(done.build, "1.2.3-16");
        assert_eq!(done.code, 99);
    }

    #[test]
    fn other_versions_do_not_count() {
        let done = with_tags(Kind::Dev, 99, false, &["1.2.2-40", "1.2.30-7", "1.2.3-2"]).unwrap();
        assert_eq!(done.build, "1.2.3-3");
    }

    #[test]
    fn released_version_gives_the_next_one_its_own_counter() {
        let done = with_tags(Kind::Dev, 99, false, &["1.2.3", "1.2.3-15", "1.2.4-2"]).unwrap();
        assert_eq!(done.build, "1.2.4-3");
        let fresh = with_tags(Kind::Dev, 99, false, &["1.2.3", "1.2.3-15"]).unwrap();
        assert_eq!(fresh.build, "1.2.4-1");
    }

    #[test]
    fn junk_and_peeled_tags_are_ignored() {
        let text = "aa\trefs/tags/v1.2.3-4\nbb\trefs/tags/v1.2.3-4^{}\ncc\trefs/tags/v1.2.3-rc1\n\
                    dd\trefs/tags/v1.2.3-\nee\trefs/tags/v1.2.3-5-x\nff\trefs/tags/v1.2.3-99^{}\n";
        assert_eq!(next_pre_release(&tag_names(text), "1.2.3"), 5);
    }

    #[test]
    fn pinned_run_overrides_the_counter() {
        let done = with_tags(Kind::Dev, 42, true, &["1.2.3-15"]).unwrap();
        assert_eq!(done.build, "1.2.3-42");
        assert_eq!(done.code, 42);
    }

    #[test]
    fn msi_limit_applies_to_the_counter() {
        let error = with_tags(Kind::Dev, 1, false, &["1.2.3-65535"])
            .unwrap_err()
            .to_string();
        assert!(error.contains("65535"), "{error}");
    }

    #[test]
    fn workspace_version_is_read_from_its_section() {
        let text = "[package]\nversion = \"9.9.9\"\n\n[workspace.package]\nedition = \"2024\"\nversion = \"1.2.3\"\n\n[other]\nversion = \"0\"\n";
        assert_eq!(workspace_version(text).as_deref(), Some("1.2.3"));
        assert_eq!(workspace_version("[a]\nversion = \"1\"\n"), None);
    }
}
