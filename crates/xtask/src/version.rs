//! The version a build ships under, and the checks that keep it honest (docs/workflow.md).
//!
//! Channels:
//! - prod: `X.Y.Z`, main: a release; the tag `vX.Y.Z` must not exist yet
//! - dev: `X.Y.Z-<run>`, development: a pre-release of the coming X.Y.Z; once X.Y.Z is released
//!   (and `mise run bump` has not run yet) of X.Y.(Z+1)
//! - none: `X.Y.Z-<run>`, builds that publish nothing (manual, scheduled)
//!
//! The pre-release part is numeric: MSI accepts nothing else (max 65535).

use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

/// What an MSI version can hold in its last field.
pub const MAX_RUN: u64 = 65535;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Prod,
    Dev,
    None,
}

impl Kind {
    pub fn parse(name: &str) -> Result<Self> {
        match name {
            "prod" => Ok(Self::Prod),
            "dev" => Ok(Self::Dev),
            "none" => Ok(Self::None),
            other => bail!("channel must be dev, prod or none, not `{other}`"),
        }
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
    /// The Android versionCode: the run number.
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

/// The pre-release number: the argument, else the run counter of the CI (GitHub, then GitLab),
/// else 1.
pub fn run_number(arg: Option<u64>, github: Option<&str>, gitlab: Option<&str>) -> Result<u64> {
    if let Some(number) = arg {
        return Ok(number);
    }
    match github.or(gitlab).filter(|value| !value.is_empty()) {
        Some(text) => text
            .parse()
            .with_context(|| format!("run number `{text}` is not a number")),
        None => Ok(1),
    }
}

/// `version` from tauri.conf.json against `cargo_version` from Cargo.toml; the tag lookup runs
/// only when the channel publishes (`tag_exists` asks the remote).
pub fn compute(
    kind: Kind,
    version: &str,
    cargo_version: &str,
    run: u64,
    changelog: &str,
    tag_exists: impl FnOnce() -> Result<bool>,
) -> Result<Computed> {
    if cargo_version != version {
        bail!(
            "Cargo.toml workspace version {cargo_version} differs from tauri.conf.json {version}"
        );
    }
    let [major, minor, patch] = parse_plain(version)?;
    let mut note = None;
    let mut build = match kind {
        Kind::Prod => version.to_string(),
        Kind::Dev | Kind::None => {
            if run > MAX_RUN {
                bail!("pipeline number {run} exceeds what an MSI version holds ({MAX_RUN})");
            }
            format!("{version}-{run}")
        }
    };
    // A released version is final: main must move on (mise run bump), and the pre-releases of
    // development aim past it until the next bump.
    if kind != Kind::None && tag_exists()? {
        if kind == Kind::Prod {
            bail!(
                "v{version} is already released: run `mise run bump` on development before merging into main"
            );
        }
        let next = format!("{major}.{minor}.{}", patch + 1);
        build = format!("{next}-{run}");
        note = Some(format!(
            "v{version} is released; this pre-release is of {next}"
        ));
    }
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
/// whose `origin` is the old GitLab), else `origin` (in GitHub Actions the checkout's origin is
/// GitHub itself, and there is no `github` remote).
fn release_remote(root: &Path) -> String {
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

/// Whether the tag `v<version>` is on the release remote. An unreachable remote is an error:
/// guessing "no tag" could let a second release of the same version through.
fn remote_has_tag(root: &Path, version: &str) -> Result<bool> {
    let remote = release_remote(root);
    let status = Command::new("git")
        .args(["ls-remote", "--exit-code", "--tags", &remote])
        .arg(format!("refs/tags/v{version}"))
        .current_dir(root)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("run git ls-remote")?;
    // --exit-code: 2 is "no such ref".
    match status.code() {
        Some(0) => Ok(true),
        Some(2) => Ok(false),
        _ => bail!(
            "cannot ask the remote `{remote}` for the tag v{version} (MWT_RELEASE_REMOTE names another)"
        ),
    }
}

pub struct Args {
    pub channel: Option<String>,
    pub run: Option<u64>,
    pub output: Option<String>,
}

/// `cargo xtask version`: the app version, or with a channel the version a build ships under.
pub fn run(root: &Path, args: &Args) -> Result<()> {
    let version = tauri_version(root)?;
    let Some(channel) = &args.channel else {
        println!("{version}");
        return Ok(());
    };
    let kind = Kind::parse(channel)?;
    let cargo_toml = std::fs::read_to_string(root.join("Cargo.toml"))?;
    let cargo_version = workspace_version(&cargo_toml).unwrap_or_default();
    let changelog = if kind == Kind::Prod {
        std::fs::read_to_string(root.join("CHANGELOG.md")).context("read CHANGELOG.md")?
    } else {
        String::new()
    };
    let run = run_number(
        args.run,
        std::env::var("GITHUB_RUN_NUMBER").ok().as_deref(),
        std::env::var("CI_PIPELINE_IID").ok().as_deref(),
    )?;
    let computed = compute(kind, &version, &cargo_version, run, &changelog, || {
        remote_has_tag(root, &version)
    })?;
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

    fn go(kind: Kind, run: u64, tag: bool, changelog: &str) -> Result<Computed> {
        compute(kind, "1.2.3", "1.2.3", run, changelog, || Ok(tag))
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
        let done = compute(Kind::None, "1.2.3", "1.2.3", 3, "", || bail!("asked")).unwrap();
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
        let error = compute(Kind::Dev, "1.2.3", "1.2.4", 1, "", || Ok(false)).unwrap_err();
        assert!(error.to_string().contains("differs"));
        assert!(parse_plain("1.2.3-4").is_err());
        assert!(parse_plain("1.2").is_err());
        assert!(parse_plain("1.2.x").is_err());
        assert_eq!(parse_plain("10.0.3").unwrap(), [10, 0, 3]);
    }

    #[test]
    fn run_number_sources_in_order() {
        assert_eq!(run_number(Some(5), Some("9"), Some("8")).unwrap(), 5);
        assert_eq!(run_number(None, Some("9"), Some("8")).unwrap(), 9);
        assert_eq!(run_number(None, None, Some("8")).unwrap(), 8);
        assert_eq!(run_number(None, None, None).unwrap(), 1);
        assert!(run_number(None, Some("x"), None).is_err());
    }

    #[test]
    fn workspace_version_is_read_from_its_section() {
        let text = "[package]\nversion = \"9.9.9\"\n\n[workspace.package]\nedition = \"2024\"\nversion = \"1.2.3\"\n\n[other]\nversion = \"0\"\n";
        assert_eq!(workspace_version(text).as_deref(), Some("1.2.3"));
        assert_eq!(workspace_version("[a]\nversion = \"1\"\n"), None);
    }
}
