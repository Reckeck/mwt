//! `xtask ci-changes`: decides whether the platform lanes (check-android, clippy-windows) run.
//! Env: EVENT, PR_NUMBER, GITHUB_REPOSITORY, GH_TOKEN. Writes `platforms=true|false` to
//! `$GITHUB_OUTPUT`. A skipped lane counts as ok in ci-ok.

use std::process::Command;

use anyhow::{Context, Result};

use crate::cache::capture_checked;

/// Paths that cannot change a platform build. One changed file outside this list means the lanes run.
const SKIPPABLE_PREFIXES: &[&str] = &["src/", "public/", "tests/", "docs/"];
const SKIPPABLE_FILES: &[&str] = &[
    "biome.json",
    "vite.config.ts",
    "vitest.config.ts",
    "steiger.config.ts",
];

fn is_skippable(path: &str) -> bool {
    SKIPPABLE_PREFIXES
        .iter()
        .any(|prefix| path.starts_with(prefix))
        || SKIPPABLE_FILES.contains(&path)
        || path.ends_with(".md")
        || (path.starts_with("tsconfig") && path.ends_with(".json"))
}

/// Whether the platform lanes must run. `files` holds every changed path, and for a rename the
/// old path too. Anything but a pull request, and an empty list, runs the lanes.
pub fn platforms_needed(event: &str, files: &[&str]) -> bool {
    event != "pull_request" || files.is_empty() || !files.iter().all(|path| is_skippable(path))
}

fn var(name: &str) -> String {
    std::env::var(name).unwrap_or_default()
}

pub fn run() -> Result<()> {
    let event = var("EVENT");
    let listed = if event == "pull_request" {
        let repo = std::env::var("GITHUB_REPOSITORY").context("GITHUB_REPOSITORY is not set")?;
        capture_checked(
            Command::new("gh")
                .args(["api", "--paginate"])
                .arg(format!("repos/{repo}/pulls/{}/files", var("PR_NUMBER")))
                .args(["--jq", ".[] | .filename, (.previous_filename // empty)"]),
        )?
    } else {
        String::new()
    };
    let files: Vec<&str> = listed.lines().filter(|line| !line.is_empty()).collect();
    let platforms = platforms_needed(&event, &files);
    super::write_outputs(&[format!("platforms={platforms}")])?;
    println!("platforms={platforms}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontend_only_skips() {
        let files = [
            "src/app.vue",
            "docs/ci.md",
            "README.md",
            "tsconfig.app.json",
        ];
        assert!(!platforms_needed("pull_request", &files));
    }

    #[test]
    fn crates_run() {
        assert!(platforms_needed(
            "pull_request",
            &["src/a.ts", "crates/app/src/lib.rs"]
        ));
    }

    #[test]
    fn package_json_and_lockfile_run() {
        assert!(platforms_needed("pull_request", &["package.json"]));
        assert!(platforms_needed("pull_request", &["pnpm-lock.yaml"]));
    }

    #[test]
    fn a_rename_out_of_crates_runs() {
        assert!(platforms_needed(
            "pull_request",
            &["src/moved.rs", "crates/old.rs"]
        ));
    }

    #[test]
    fn an_empty_list_runs() {
        assert!(platforms_needed("pull_request", &[]));
    }

    #[test]
    fn other_events_run() {
        for event in ["push", "workflow_dispatch", "workflow_call"] {
            assert!(platforms_needed(event, &["src/a.ts"]));
        }
    }
}
