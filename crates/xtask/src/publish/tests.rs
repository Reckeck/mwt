use std::path::PathBuf;

use super::aur::{Plan, Step, steps, tarball_name, tarball_url};
use super::notes::{Commit, Release, parse_log, prerelease_notes, release_args, release_notes};
use super::prune::{delete_args, select};
use super::*;

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_string()).collect()
}

const CHANGELOG: &str = "# Changelog\n\n## [Unreleased]\n\n- next\n\n## [0.2.0] - 2026-11-01\n\n### Added\n\n- thing\n\n## [0.1.0] - 2026-10-01\n\n- old\n";

#[test]
fn release_notes_are_the_changelog_section_of_the_version() {
    let notes = release_notes(CHANGELOG, "0.2.0", "Reckeck/mwt");
    assert_eq!(notes, "### Added\n\n- thing");
    assert!(!notes.contains("old") && !notes.contains("next"));
}

#[test]
fn release_notes_fall_back_to_a_link_at_the_tag() {
    assert_eq!(
        release_notes(CHANGELOG, "0.3.0", "Reckeck/mwt"),
        "Changes are listed in [CHANGELOG.md](https://github.com/Reckeck/mwt/blob/v0.3.0/CHANGELOG.md)."
    );
    // `.` is not a wildcard: 0x2x0 is not 0.2.0
    assert!(section_is_empty("0x2x0"));
}

fn section_is_empty(version: &str) -> bool {
    super::notes::section(CHANGELOG, version).is_empty()
}

#[test]
fn prerelease_notes_list_commits_with_links() {
    let sha = "0123456789abcdef0123456789abcdef01234567";
    let commits = parse_log(&format!(
        "{sha}\u{1f}fix: one\nabcdef1234567\u{1f}feat: two\n\n"
    ));
    assert_eq!(commits.len(), 2);
    let notes = prerelease_notes("Reckeck/mwt", "development", sha, Some("v0.1.0"), &commits);
    assert_eq!(
        notes,
        "**Pre-release** from `development` (commit [`0123456`](https://github.com/Reckeck/mwt/commit/0123456789abcdef0123456789abcdef01234567)) - for testing, not for working with real data.\n\n### Changes since v0.1.0\n\n- fix: one ([`0123456`](https://github.com/Reckeck/mwt/commit/0123456789abcdef0123456789abcdef01234567))\n- feat: two ([`abcdef1`](https://github.com/Reckeck/mwt/commit/abcdef1234567))"
    );
}

#[test]
fn prerelease_notes_without_a_release_or_commits() {
    let notes = prerelease_notes("o/r", "development", "abc1234", None, &[]);
    assert!(notes.contains("since the start of the project"));
    assert!(notes.ends_with("_no commits_"));
    assert_eq!(
        parse_log("a\u{1f}b"),
        [Commit {
            sha: "a".into(),
            subject: "b".into()
        }]
    );
}

fn release<'a>(files: &'a [String], pre: bool) -> Release<'a> {
    Release {
        repo: "Reckeck/mwt",
        tag: if pre { "v0.1.0-7" } else { "v0.1.0" },
        files,
        sha: "cafe",
        title: if pre {
            "Pre-release 0.1.0-7"
        } else {
            "Multi Wallet Tracker 0.1.0"
        },
        notes_file: "/tmp/n.md",
        pre,
    }
}

#[test]
fn gh_release_arguments() {
    let files = strings(&["d/a.deb", "d/SHA256SUMS"]);
    assert_eq!(
        release_args(&release(&files, false)),
        strings(&[
            "release",
            "create",
            "v0.1.0",
            "d/a.deb",
            "d/SHA256SUMS",
            "--repo",
            "Reckeck/mwt",
            "--target",
            "cafe",
            "--title",
            "Multi Wallet Tracker 0.1.0",
            "--notes-file",
            "/tmp/n.md",
        ])
    );
    let pre = release_args(&release(&files, true));
    assert_eq!(pre.last().map(String::as_str), Some("--prerelease"));
    assert!(pre.contains(&"Pre-release 0.1.0-7".to_string()));
}

#[test]
fn shell_lines_quote_what_needs_it() {
    assert_eq!(
        shell_line("gh", &strings(&["--title", "Pre-release 1", "it's"])),
        "gh --title 'Pre-release 1' 'it'\\''s'"
    );
}

#[test]
fn versions_and_tags() {
    assert!(is_prerelease_tag("v0.1.0-7"));
    for bad in [
        "v0.1.0",
        "0.1.0-7",
        "v0.1.0-",
        "v0.1-7",
        "v0.1.0-x",
        "v0.1.0-7-8",
        "v1.2.3.4-5",
    ] {
        assert!(!is_prerelease_tag(bad), "{bad}");
    }
    assert!(check_version("0.1.0-7", true).is_ok());
    assert!(check_version("0.1.0", true).is_err());
    assert!(check_version("0.1.0", false).is_ok());
    assert!(check_version("0.1.0-7", false).is_err());
}

#[test]
fn prune_keeps_the_newest_and_skips_everything_but_prerelease_tags() {
    let tags = strings(&[
        "v0.2.0-3", "v0.2.0", "v0.2.0-2", "v0.1.0", "v0.1.0-9", "nightly", "v0.1.0-8",
    ]);
    assert_eq!(select(&tags, 2), strings(&["v0.1.0-9", "v0.1.0-8"]));
    assert_eq!(select(&tags, 0).len(), 4);
    assert!(select(&tags, 4).is_empty());
}

#[test]
fn prune_never_picks_a_release_tag() {
    let releases = strings(&[
        "v1.0.0",
        "v0.9.0",
        "v0.1.0",
        "v10.20.30",
        "v1.0.0-rc1",
        "v1.0.0-",
    ]);
    for keep in [0, 1, 10] {
        assert!(select(&releases, keep).is_empty(), "keep {keep}");
    }
    let mixed = strings(&["v1.0.0", "v1.0.0-1", "v1.0.0-2"]);
    assert!(select(&mixed, 0).iter().all(|tag| tag != "v1.0.0"));
    assert_eq!(
        delete_args("o/r", "v1.0.0-1"),
        strings(&[
            "release",
            "delete",
            "v1.0.0-1",
            "--repo",
            "o/r",
            "--cleanup-tag",
            "--yes"
        ])
    );
}

const SECRET: &str = "-----BEGIN OPENSSH PRIVATE KEY-----SECRET-MATERIAL";

fn plan() -> Plan {
    Plan {
        version: "0.1.0".into(),
        tarball: PathBuf::from("/rel/multi-wallet-tracker-0.1.0-linux-x86_64.tar.gz"),
        url: tarball_url("Reckeck/mwt", "0.1.0"),
        home: PathBuf::from("/root"),
        work: PathBuf::from("/tmp/w"),
        root: PathBuf::from("/src"),
        git_name: "bot".into(),
        git_email: "bot@example.com".into(),
    }
}

#[test]
fn aur_url_points_at_the_github_release() {
    assert_eq!(
        tarball_name("0.1.0"),
        "multi-wallet-tracker-0.1.0-linux-x86_64.tar.gz"
    );
    assert_eq!(
        tarball_url("Reckeck/mwt", "0.1.0"),
        "https://github.com/Reckeck/mwt/releases/download/v0.1.0/multi-wallet-tracker-0.1.0-linux-x86_64.tar.gz"
    );
}

#[test]
fn aur_steps_never_carry_the_key() {
    // Whatever the environment holds, the plan is built from paths alone.
    unsafe { std::env::set_var("AUR_SSH_PRIVATE_KEY", SECRET) };
    let all = steps(&plan());
    unsafe { std::env::remove_var("AUR_SSH_PRIVATE_KEY") };
    let text: String = all.iter().map(Step::show).collect::<Vec<_>>().join("\n");
    assert!(!text.contains("SECRET") && !text.contains("PRIVATE KEY"));
    assert!(text.contains("write $AUR_SSH_PRIVATE_KEY to /root/.ssh/aur (mode 0600)"));
    assert!(text.contains("-i /root/.ssh/aur -o IdentitiesOnly=yes"));
    assert!(text.contains("render.sh 0.1.0 /rel/multi-wallet-tracker-0.1.0-linux-x86_64.tar.gz https://github.com/Reckeck/mwt/releases/download/v0.1.0/"));
    assert!(text.trim_end().ends_with("push origin HEAD:master"));
}
