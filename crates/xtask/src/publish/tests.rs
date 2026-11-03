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
fn prerelease_notes_group_commits_like_the_changelog() {
    let sha = "0123456789abcdef0123456789abcdef01234567";
    let commits = parse_log(&format!(
        "{sha}\u{1f}fix(ui): one\nabcdef1234567\u{1f}feat: two\nbbbbbbb\u{1f}chore(release): 0.2.0\ncccccccc\u{1f}Merge stuff\n\n"
    ));
    assert_eq!(commits.len(), 4);
    let notes = prerelease_notes(
        "Reckeck/mwt",
        "development",
        sha,
        Some("v0.1.0-3"),
        &commits,
        false,
    );
    assert_eq!(
        notes,
        "**Pre-release** from `development` (commit [`0123456`](https://github.com/Reckeck/mwt/commit/0123456789abcdef0123456789abcdef01234567)) - for testing, not for working with real data.\n\n\
### Changes since v0.1.0-3\n\n\
[Full diff v0.1.0-3...0123456](https://github.com/Reckeck/mwt/compare/v0.1.0-3...0123456789abcdef0123456789abcdef01234567)\n\n\
#### Added\n\n- Two ([`abcdef1`](https://github.com/Reckeck/mwt/commit/abcdef1234567))\n\n\
#### Fixed\n\n- **ui:** One ([`0123456`](https://github.com/Reckeck/mwt/commit/0123456789abcdef0123456789abcdef01234567))\n\n\
#### Other\n\n- Merge stuff ([`ccccccc`](https://github.com/Reckeck/mwt/commit/cccccccc))"
    );
    assert!(!notes.contains("chore(release)") && !notes.contains("0.2.0"));
}

#[test]
fn prerelease_notes_without_a_release_or_commits() {
    let notes = prerelease_notes("o/r", "development", "abc1234", None, &[], false);
    assert!(notes.contains("since the start of the project"));
    assert!(!notes.contains("compare"));
    assert!(notes.ends_with("_no changes_"));
    assert_eq!(
        parse_log("a\u{1f}b"),
        [Commit {
            sha: "a".into(),
            subject: "b".into()
        }]
    );
}

#[test]
fn prerelease_notes_say_when_the_list_is_cut() {
    let commits = parse_log("abc\u{1f}docs!: breaking docs");
    let notes = prerelease_notes("o/r", "development", "abc", Some("v1.0.0"), &commits, true);
    assert!(notes.contains("#### Documentation\n\n- Breaking docs"));
    assert!(notes.ends_with("_Older commits are in the full diff._"));
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

fn manifest_names(version: &str, with_apk: bool) -> Vec<String> {
    let mut names = vec![
        format!("multi-wallet-tracker-{version}-x86_64.AppImage"),
        format!("multi-wallet-tracker-{version}-x86_64.AppImage.sig"),
        format!("multi-wallet-tracker-{version}-x64-setup.exe"),
        format!("multi-wallet-tracker-{version}-x64-setup.exe.sig"),
        format!("multi-wallet-tracker-{version}-x64-en-US.msi"),
        format!("multi-wallet-tracker-{version}-x64-en-US.msi.sig"),
    ];
    if with_apk {
        names.push(format!(
            "multi-wallet-tracker-{version}-android-universal.apk"
        ));
    }
    names
}

fn manifest_for(names: &[String]) -> (serde_json::Value, Vec<String>) {
    let signatures = super::manifest::signed_names(names)
        .into_iter()
        .map(|name| {
            let sig = format!("sig-of-{name}\n");
            (name, sig)
        })
        .collect();
    super::manifest::build(&super::manifest::Input {
        repo: "Reckeck/mwt",
        tag: "v1.2.3",
        version: "1.2.3",
        notes: "Multi Wallet Tracker 1.2.3",
        pub_date: "2026-11-03T10:00:00Z",
        names,
        signatures: &signatures,
    })
}

#[test]
fn manifest_lists_signed_platforms_with_urls() {
    let (value, warnings) = manifest_for(&manifest_names("1.2.3", true));
    assert!(warnings.is_empty());
    assert_eq!(value["version"], "1.2.3");
    assert_eq!(value["pub_date"], "2026-11-03T10:00:00Z");
    let linux = &value["platforms"]["linux-x86_64"];
    assert_eq!(
        linux["url"],
        "https://github.com/Reckeck/mwt/releases/download/v1.2.3/multi-wallet-tracker-1.2.3-x86_64.AppImage"
    );
    assert_eq!(
        linux["signature"],
        "sig-of-multi-wallet-tracker-1.2.3-x86_64.AppImage"
    );
    for key in ["windows-x86_64", "windows-x86_64-nsis"] {
        assert!(
            value["platforms"][key]["url"]
                .as_str()
                .unwrap()
                .ends_with("-x64-setup.exe")
        );
    }
    assert!(
        value["platforms"]["windows-x86_64-msi"]["url"]
            .as_str()
            .unwrap()
            .ends_with("en-US.msi")
    );
    assert_eq!(
        value["release_url"],
        "https://github.com/Reckeck/mwt/releases/tag/v1.2.3"
    );
    assert!(
        value["apk_url"]
            .as_str()
            .unwrap()
            .ends_with("-android-universal.apk")
    );
}

#[test]
fn manifest_skips_unsigned_platforms_and_keeps_the_links() {
    let (value, warnings) = manifest_for(&manifest_names("1.2.3", false)[..0]);
    assert_eq!(warnings.len(), 3);
    assert!(value["platforms"].as_object().unwrap().is_empty());
    assert!(value["apk_url"].is_null());
    assert!(value["release_url"].is_string());
    let (partial, warnings) = manifest_for(&manifest_names("1.2.3", false)[..2]);
    assert_eq!(warnings.len(), 2);
    assert_eq!(partial["platforms"].as_object().unwrap().len(), 1);
}

#[test]
fn manifest_file_names_and_dev_release() {
    assert_eq!(super::manifest::file_name(false), "latest.json");
    assert_eq!(super::manifest::file_name(true), "latest-dev.json");
    let create = super::manifest::dev_release_args("o/r", "cafe", "d/latest-dev.json", false);
    assert_eq!(
        &create[..3],
        &strings(&["release", "create", "updater-dev"])[..]
    );
    assert!(create.contains(&"--prerelease".to_string()));
    let upload = super::manifest::dev_release_args("o/r", "cafe", "d/latest-dev.json", true);
    assert_eq!(&upload[..2], &strings(&["release", "upload"])[..]);
    assert!(upload.contains(&"--clobber".to_string()));
}

#[test]
fn prune_never_picks_the_updater_release() {
    let tags = strings(&["v1.0.0-3", "updater-dev", "v1.0.0", "v1.0.0-2", "v1.0.0-1"]);
    assert_eq!(select(&tags, 1), strings(&["v1.0.0-2", "v1.0.0-1"]));
    assert_eq!(select(&strings(&["updater-dev"]), 0), Vec::<String>::new());
}

#[test]
fn pre_env_is_true_only_for_the_word_true() {
    assert!(pre_from_env(Some("true")));
    assert!(!pre_from_env(Some("false")));
    assert!(!pre_from_env(Some("")));
    assert!(!pre_from_env(None));
}

#[test]
fn an_unset_or_empty_aur_key_is_missing() {
    assert!(super::aur::key_missing(None));
    assert!(super::aur::key_missing(Some("")));
    assert!(!super::aur::key_missing(Some("/root/key")));
    assert!(super::aur::NO_KEY_WARNING.starts_with("::warning::"));
}

fn git_run(dir: &Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .args(["-c", "user.name=test", "-c", "user.email=t@example.com"])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

#[test]
fn the_first_pre_release_after_a_release_starts_at_the_release_base() {
    let dir = std::env::temp_dir().join(format!("xtask-since-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let commit = |message: &str| git_run(&dir, &["commit", "-q", "--allow-empty", "-m", message]);
    git_run(&dir, &["init", "-q", "-b", "development"]);
    commit("feat: old");
    let first = git_run(&dir, &["rev-parse", "HEAD"]);
    commit("chore(release): v0.2.0");
    git_run(&dir, &["tag", "release-base/v0.2.0"]);
    // main: the squash of the release, tagged; merged back with `-s ours`
    git_run(&dir, &["switch", "-q", "-c", "main", &first]);
    commit("chore(release): v0.2.0 (squash)");
    git_run(&dir, &["tag", "v0.2.0"]);
    git_run(&dir, &["switch", "-q", "development"]);
    git_run(&dir, &["merge", "-q", "-s", "ours", "-m", "back", "main"]);
    commit("feat: next");
    let sha = git_run(&dir, &["rev-parse", "HEAD"]);
    assert_eq!(
        describe_since(&dir, "v0.2.1-1", &sha).as_deref(),
        Some("release-base/v0.2.0")
    );
    // a pre-release tag is closer still
    git_run(&dir, &["tag", "v0.2.1-1"]);
    commit("fix: later");
    let later = git_run(&dir, &["rev-parse", "HEAD"]);
    assert_eq!(
        describe_since(&dir, "v0.2.1-2", &later).as_deref(),
        Some("v0.2.1-1")
    );
    // no tag at all: the whole history
    let bare = std::env::temp_dir().join(format!("xtask-since-bare-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&bare);
    std::fs::create_dir_all(&bare).unwrap();
    git_run(&bare, &["init", "-q"]);
    git_run(&bare, &["commit", "-q", "--allow-empty", "-m", "feat: x"]);
    assert_eq!(describe_since(&bare, "v0.1.0", "HEAD"), None);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&bare);
}
