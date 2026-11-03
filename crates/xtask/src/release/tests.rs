use super::*;

fn never() -> Result<Version> {
    panic!("git-cliff must not be asked")
}

#[test]
fn parse_wants_three_plain_numbers() {
    assert_eq!(parse("1.2.3"), Some([1, 2, 3]));
    for bad in ["1.2", "1.2.3.4", "1.2.x", "1.2.3-4", "", "1..3", "+1.2.3"] {
        assert_eq!(parse(bad), None, "{bad}");
    }
}

#[test]
fn last_release_ignores_pre_release_tags_and_compares_numbers() {
    assert_eq!(
        last_release("v0.9.0\nv0.10.0\nv0.10.1-3\nvx\n"),
        Some([0, 10, 0])
    );
    assert_eq!(last_release(""), None);
}

#[test]
fn levels_count_from_the_last_release() {
    let current = [0, 1, 0];
    let last = Some([0, 4, 2]);
    assert_eq!(
        next_version("patch", current, last, never).unwrap(),
        [0, 4, 3]
    );
    assert_eq!(
        next_version("minor", current, last, never).unwrap(),
        [0, 5, 0]
    );
    assert_eq!(
        next_version("major", current, last, never).unwrap(),
        [1, 0, 0]
    );
    assert_eq!(
        next_version("patch", current, None, never).unwrap(),
        [0, 1, 1]
    );
    assert_eq!(
        next_version("1.2.3", current, last, never).unwrap(),
        [1, 2, 3]
    );
}

#[test]
fn auto_keeps_a_higher_current_and_ships_the_first_as_is() {
    let current = [0, 3, 0];
    assert_eq!(next_version("auto", current, None, never).unwrap(), current);
    assert_eq!(
        next_version("auto", current, Some([0, 2, 0]), || Ok([0, 2, 1])).unwrap(),
        current
    );
    assert_eq!(
        next_version("auto", current, Some([0, 3, 0]), || Ok([0, 4, 0])).unwrap(),
        [0, 4, 0]
    );
}

#[test]
fn unknown_level_is_an_error() {
    assert!(next_version("huge", [0, 1, 0], None, never).is_err());
    assert!(next_version("1.2", [0, 1, 0], None, never).is_err());
    assert!(next_version("1.2.3-4", [0, 1, 0], None, never).is_err());
}

#[test]
fn tauri_version_is_the_top_level_key_only() {
    let text = "{\n  \"productName\": \"x\",\n  \"version\": \"0.1.0\",\n  \"bundle\": {\n    \"version\": \"9\"\n  }\n}\n";
    let out = set_tauri_version(text, "0.2.0").unwrap();
    assert_eq!(out, text.replace("\"0.1.0\"", "\"0.2.0\""));
    assert!(set_tauri_version("{}", "0.2.0").is_err());
    assert!(set_tauri_version("{\n    \"version\": \"1\",\n}", "0.2.0").is_err());
}

#[test]
fn cargo_versions_follow_their_anchor() {
    let toml = "[package]\nversion = \"9.9.9\"\n\n[workspace.package]\nversion = \"0.1.0\"\n";
    assert_eq!(
        set_workspace_version(toml, "0.2.0").unwrap(),
        "[package]\nversion = \"9.9.9\"\n\n[workspace.package]\nversion = \"0.2.0\"\n"
    );
    assert!(set_workspace_version("[package]\nversion = \"1\"\n", "0.2.0").is_err());
    let lock =
        "name = \"trade-core\"\nversion = \"0.1.0\"\n\nname = \"trade-app\"\nversion = \"0.1.0\"\n";
    let out = set_lock_version(lock, "trade-app", "0.2.0").unwrap();
    assert_eq!(
        out,
        "name = \"trade-core\"\nversion = \"0.1.0\"\n\nname = \"trade-app\"\nversion = \"0.2.0\"\n"
    );
    assert!(set_lock_version(lock, "other", "0.2.0").is_err());
}

#[test]
fn metainfo_adds_or_redates_a_release() {
    let xml = "<releases>\n    <release version=\"0.1.0\" date=\"2026-10-13\"/>\n</releases>\n";
    assert_eq!(
        set_metainfo_release(xml, "0.2.0", "2026-11-01").unwrap(),
        "<releases>\n    <release version=\"0.2.0\" date=\"2026-11-01\"/>\n    <release version=\"0.1.0\" date=\"2026-10-13\"/>\n</releases>\n"
    );
    assert_eq!(
        set_metainfo_release(xml, "0.1.0", "2026-11-01").unwrap(),
        xml.replace("2026-10-13", "2026-11-01")
    );
    assert!(set_metainfo_release("<component/>", "0.2.0", "2026-11-01").is_err());
    assert!(set_metainfo_release("<release version=\"0.2.0\"/>", "0.2.0", "2026-11-01").is_err());
}

#[test]
fn tag_versions_come_with_or_without_the_boundary_prefix() {
    assert_eq!(tag_version("v1.2.3"), Some([1, 2, 3]));
    assert_eq!(tag_version("release-base/v1.2.3\n"), Some([1, 2, 3]));
    assert_eq!(tag_version("release-base/v1.2.3-4"), None);
    assert_eq!(tag_version(""), None);
}

#[test]
fn the_commit_range_starts_at_the_boundary_or_covers_the_unreleased() {
    assert_eq!(
        list_args(Some("release-base/v0.2.0")),
        ["release-base/v0.2.0..HEAD", "--strip", "all"]
    );
    assert_eq!(list_args(None), ["--unreleased", "--strip", "all"]);
}

#[test]
fn cliff_counts_from_the_release_base_tag() {
    let cliff =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../cliff.toml")).unwrap();
    assert!(cliff.contains(r#"tag_pattern = "^release-base/v[0-9]+\\.[0-9]+\\.[0-9]+$""#));
    assert!(cliff.contains(&format!(r#"initial_tag = "{BASE_TAG_PREFIX}0.1.0""#)));
}

fn git_in(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args([
            "-c",
            "user.name=test",
            "-c",
            "user.email=test@example.com",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "init.defaultBranch=development",
        ])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn temp_repo(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("xtask-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    git_in(&dir, &["init", "-q"]);
    dir
}

fn commit_in(dir: &Path, message: &str) {
    git_in(dir, &["commit", "-q", "--allow-empty", "-m", message]);
}

/// The history a release leaves: the release tip is squashed into main (the tag `vX.Y.Z` is on
/// the squash, which is no ancestor of development), main is merged back with `-s ours`.
#[test]
fn the_boundary_follows_the_release_base_through_the_squash_and_the_back_merge() {
    let dir = temp_repo("boundary");
    assert_eq!(boundary(&dir), None, "no commits yet");
    commit_in(&dir, "feat: one");
    let first = git_in(&dir, &["rev-parse", "HEAD"]);
    assert_eq!(boundary(&dir), None, "before the first release");
    commit_in(&dir, "chore(release): v0.2.0");
    git_in(&dir, &["tag", "release-base/v0.2.0"]);
    git_in(&dir, &["tag", "v0.2.0-1"]);
    commit_in(&dir, "fix: during the release");
    // the squash on main: its parent is not in development
    git_in(&dir, &["switch", "-q", "-c", "main", &first]);
    commit_in(&dir, "chore(release): v0.2.0 (squash)");
    git_in(&dir, &["tag", "v0.2.0"]);
    git_in(&dir, &["switch", "-q", "development"]);
    assert_eq!(
        boundary(&dir).as_deref(),
        Some("release-base/v0.2.0"),
        "the squash tag is not an ancestor of development"
    );
    // the back-merge: the release tip + main by `-s ours`, merged into development
    git_in(
        &dir,
        &[
            "switch",
            "-q",
            "-C",
            "backmerge/v0.2.0",
            "release-base/v0.2.0",
        ],
    );
    git_in(&dir, &["merge", "-q", "-s", "ours", "-m", "back", "main"]);
    git_in(&dir, &["switch", "-q", "development"]);
    git_in(
        &dir,
        &["merge", "-q", "--no-ff", "-m", "merge", "backmerge/v0.2.0"],
    );
    commit_in(&dir, "feat: next");
    git_in(&dir, &["tag", "v0.2.1-1"]);
    assert_eq!(
        boundary(&dir).as_deref(),
        Some("release-base/v0.2.0"),
        "the tag of main is behind the release base, a pre-release is no boundary"
    );
    // a hotfix branch from main: the release tag is the boundary
    git_in(&dir, &["switch", "-q", "-c", "hotfix/x", "main"]);
    commit_in(&dir, "fix: hot");
    assert_eq!(boundary(&dir).as_deref(), Some("v0.2.0"));
    let _ = std::fs::remove_dir_all(&dir);
}
