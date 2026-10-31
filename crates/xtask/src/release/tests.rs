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
