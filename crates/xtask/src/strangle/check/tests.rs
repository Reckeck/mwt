use super::rules::without_binding_mut;
use super::*;
use crate::strangle::area_fixture::{
    ORIGINAL, PURE, SCENARIO, SERVICE, TESTS, moved_files, moved_tree, original_tree,
};
use crate::strangle::fileset::FileSet;
use crate::strangle::fixture::TempTree;
use crate::strangle::forward::forward;
use crate::strangle::map::MAP_FILE;
use crate::strangle::relocate::relocate;

/// The baseline of the original tree, then the findings on `after`.
fn found_in(after: &TempTree, changed: Option<&[String]>) -> Vec<String> {
    let before = original_tree();
    let baseline =
        baseline::capture(before.root(), &Workspace::load(before.root()).unwrap()).unwrap();
    let workspace = Workspace::load(after.root()).unwrap();
    findings(
        after.root(),
        &workspace,
        &baseline,
        Some("loyalty"),
        changed,
        None,
    )
    .unwrap()
}

fn broken(path: &str, from: &str, to: &str) -> Vec<String> {
    let tree = moved_tree(|file, text| {
        if file == path {
            assert!(text.contains(from), "{from}");
            text.replace(from, to)
        } else {
            text
        }
    });
    found_in(&tree, Some(&[]))
}

#[test]
fn the_good_fixture_is_green() {
    let tree = moved_tree(|_, text| text);
    let changed = [SERVICE.to_string(), PURE.to_string(), SCENARIO.to_string()];
    assert_eq!(found_in(&tree, Some(&changed)), Vec::<String>::new());
}

#[test]
fn a_missing_delegate_breaks_parity() {
    let found = broken(
        SERVICE,
        "    pub fn clash(&self, id: &str) -> bool\n        => crate::household::loyalty::clash @books;\n",
        "",
    );
    assert_eq!(found, ["loyalty: parity: not forwarded: clash"]);
}

#[test]
fn an_extra_delegate_breaks_parity() {
    let found = broken(
        SERVICE,
        "forward! {\n",
        "forward! {\n    pub fn more(&self) -> u8\n        => crate::household::loyalty::more;\n",
    );
    assert_eq!(
        found,
        ["loyalty: parity: forwarded but not in the baseline: more"]
    );
}

fn with_shim_impl(body: &str) -> Vec<String> {
    let method = format!(
        "\nimpl UnlockedVault {{\n    pub fn x(&self) -> u8 {{\n        {body}\n    }}\n}}\n"
    );
    broken(
        SERVICE,
        "// forward:begin",
        &format!("{method}// forward:begin"),
    )
}

#[test]
fn an_impl_method_in_a_group_file_is_found() {
    let method = "\nimpl UnlockedVault {\n    pub fn x(&self) -> u8 {\n        crate::household::loyalty::go(self)\n    }\n}\n";
    let found = broken(
        PURE,
        "pub struct Card {",
        &format!("{method}pub struct Card {{"),
    );
    assert_eq!(
        found,
        [format!(
            "loyalty: logic: {PURE}: `x` is an `impl UnlockedVault` method outside the shim"
        )]
    );
}

#[test]
fn a_shim_body_with_extra_logic_is_found() {
    for body in [
        "if true { 1 } else { 2 }",
        "let n = 1; crate::household::loyalty::go(self, n)",
        "other::go(self)",
        "crate::household::loyalty::go(self, 1 + 2)",
        "self.go()",
    ] {
        assert_eq!(
            with_shim_impl(body),
            [format!(
                "loyalty: logic: {SERVICE}: `x` is not a passthrough call into the group or usecases"
            )],
            "{body}"
        );
    }
}

#[test]
fn a_plain_passthrough_shim_passes() {
    for body in [
        "crate::household::loyalty::go(self)",
        "crate::usecases::household::loyalty::go(self, &id)",
    ] {
        assert!(with_shim_impl(body).is_empty(), "{body}");
    }
}

#[test]
fn a_persist_call_is_found_outside_tests_only() {
    let found = broken(
        SCENARIO,
        "vault.transact(|tx| apply(tx, id, info))",
        "vault.persist();\n    vault.transact(|tx| apply(tx, id, info))",
    );
    assert_eq!(
        found,
        [format!("loyalty: persist: {SCENARIO}:4: persist()")]
    );
}

#[test]
fn a_renamed_or_dropped_test_is_found() {
    let found = broken(TESTS, "fn clash_works", "fn clash_still_works");
    assert_eq!(found, ["loyalty: tests: test missing: clash_works"]);
}

#[test]
fn an_added_test_is_not_a_finding() {
    let found = broken(
        TESTS,
        "fn clash_works",
        "fn a_new_test() {}\n\n#[test]\nfn clash_works",
    );
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn a_changed_file_outside_the_allowed_set_is_found() {
    let tree = moved_tree(|_, text| text);
    let changed = [
        PURE.to_string(),
        "crates/core/src/service/mod.rs".to_string(),
    ];
    assert_eq!(
        found_in(&tree, Some(&changed)),
        ["loyalty: files: crates/core/src/service/mod.rs is outside the allowed set"]
    );
    // Without `--diff` the rule does not look.
    assert!(found_in(&tree, None).is_empty());
}

#[test]
fn an_unmoved_area_is_red_on_logic_persist_and_parity() {
    let tree = original_tree();
    let workspace = Workspace::load(tree.root()).unwrap();
    let area = &workspace.map.areas[0];
    let baseline = baseline::capture(tree.root(), &workspace).unwrap();
    let subject = Subject {
        root: tree.root(),
        workspace: &workspace,
        area,
        baseline: baseline.area("loyalty").unwrap(),
    };
    let found = check_area(&subject).unwrap();
    assert!(
        found.iter().any(|line| line.contains("logic: ")),
        "{found:?}"
    );
    assert!(
        found.iter().any(|line| line.contains("persist: ")),
        "{found:?}"
    );
    assert!(
        found
            .iter()
            .any(|line| line.contains("not forwarded: clash")),
        "{found:?}"
    );
}

#[test]
fn binding_mut_is_ignored_but_reference_mut_is_not() {
    assert_eq!(
        without_binding_mut("pub fn f(&mut self, mut rows: Vec<T>, other: &mut U)"),
        "pub fn f(&mut self, rows: Vec<T>, other: &mut U)"
    );
}

#[test]
fn a_changed_signature_breaks_parity() {
    let found = broken(
        SERVICE,
        "pub fn clash(&self, id: &str) -> bool",
        "pub fn clash(&self, id: &String) -> bool",
    );
    assert_eq!(
        found,
        [
            "loyalty: parity: signature differs: clash: pub fn clash(&self, id: &str) -> bool vs pub fn clash(&self, id: &String) -> bool"
        ]
    );
}

const DEEP_MAP: &str = r#"
service_dir = "crates/core/src/service"
command_dir = "crates/app/src"
scan_dirs = ["crates/core/src"]

[[area]]
name = "loyalty"
task = "T26"
group = "household"
context = "household"
files = ["loyalty.rs"]

[[area]]
name = "pending"
task = "T27"
group = "household"
context = "household"
files = ["pending.rs"]

[[area]]
name = "deep"
task = "T28"
group = "household"
context = "household"
files = ["deep/"]

[[area]]
name = "vault_side"
task = "phase-6"
stubs = false
group = "vault"
context = "vault"
files = ["vault.rs"]
"#;

const UNMOVED: &str = "impl UnlockedVault {\n    pub fn p(&self) -> u8 {\n        self.persist();\n        1\n    }\n}\n";
const PENDING: &str = "crates/core/src/service/pending.rs";
const VAULT: &str = "crates/core/src/service/vault.rs";
const DEEP: &str = "crates/core/src/service/deep/inner.rs";
const DEEP_WITH_TEST: &str = "impl UnlockedVault {\n    pub fn d(&self) -> u8 {\n        1\n    }\n}\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn deep_works() {}\n}\n";

/// Loyalty moved or not, plus an unmoved area, a phase-6 area and a directory area.
fn many_areas(moved: bool, deep: &str) -> TempTree {
    let files: Vec<(&str, String)> = if moved {
        moved_files()
    } else {
        vec![(SERVICE, ORIGINAL.to_string())]
    };
    let mut all: Vec<(&str, String)> = files
        .into_iter()
        .filter(|(path, _)| *path != MAP_FILE)
        .collect();
    all.extend([
        (MAP_FILE, DEEP_MAP.to_string()),
        (PENDING, UNMOVED.to_string()),
        (VAULT, UNMOVED.to_string()),
        (DEEP, deep.to_string()),
    ]);
    let borrowed: Vec<(&str, &str)> = all
        .iter()
        .map(|(path, text)| (*path, text.as_str()))
        .collect();
    TempTree::new(&borrowed)
}

fn baseline_of(tree: &TempTree) -> Baseline {
    baseline::capture(tree.root(), &Workspace::load(tree.root()).unwrap()).unwrap()
}

fn all_findings(tree: &TempTree, baseline: &Baseline, changed: Option<&[String]>) -> Vec<String> {
    let workspace = Workspace::load(tree.root()).unwrap();
    findings(tree.root(), &workspace, baseline, None, changed, None).unwrap()
}

#[test]
fn all_skips_unmoved_and_phase_six_areas_and_checks_the_files_once() {
    let baseline = baseline_of(&many_areas(false, DEEP_WITH_TEST));
    let tree = many_areas(true, "");
    // `deep` has no shim yet: skipped, even though its files lost their methods.
    assert_eq!(
        all_findings(&tree, &baseline, Some(&[])),
        Vec::<String>::new()
    );
    // Files outside `service/`, the group dirs and `usecases/` are left to the task's own list.
    let changed = [
        PURE.to_string(),
        "Cargo.lock".to_string(),
        "README.md".to_string(),
    ];
    assert_eq!(
        all_findings(&tree, &baseline, Some(&changed)),
        Vec::<String>::new()
    );
    let changed = [
        PURE.to_string(),
        "crates/core/src/service/stray.rs".to_string(),
    ];
    assert_eq!(
        all_findings(&tree, &baseline, Some(&changed)),
        ["all: files: crates/core/src/service/stray.rs is outside the allowed set"]
    );
    // Named on its own, the unmoved area is red.
    let workspace = Workspace::load(tree.root()).unwrap();
    let named = findings(
        tree.root(),
        &workspace,
        &baseline,
        Some("pending"),
        None,
        None,
    )
    .unwrap();
    assert!(!named.is_empty());
}

#[test]
fn files_of_two_areas_in_one_diff_are_red_and_each_alone_is_green() {
    let baseline = baseline_of(&many_areas(false, DEEP_WITH_TEST));
    let tree = many_areas(true, "");
    let both = [PURE.to_string(), PENDING.to_string()];
    let found = all_findings(&tree, &baseline, Some(&both));
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].starts_with("all: files: no single task allows every changed file"),
        "{found:?}"
    );
    for alone in [PURE, PENDING] {
        let one = [alone.to_string()];
        assert!(all_findings(&tree, &baseline, Some(&one)).is_empty());
    }
    let workspace = Workspace::load(tree.root()).unwrap();
    let named = findings(
        tree.root(),
        &workspace,
        &baseline,
        None,
        Some(&both),
        Some("T26"),
    )
    .unwrap();
    assert_eq!(
        named,
        ["all: files: crates/core/src/service/pending.rs is outside the allowed set"]
    );
}

#[test]
fn a_file_in_a_service_directory_is_part_of_the_area() {
    let baseline = baseline_of(&many_areas(false, DEEP_WITH_TEST));
    let area = baseline.area("deep").unwrap();
    assert_eq!(area.methods.len(), 1);
    assert_eq!(area.tests, ["deep_works"]);
    // The test in the directory disappears: `deep` is red on its tests.
    let tree = many_areas(
        false,
        "impl UnlockedVault {\n    pub fn d(&self) -> u8 {\n        1\n    }\n}\n",
    );
    let workspace = Workspace::load(tree.root()).unwrap();
    let found = findings(tree.root(), &workspace, &baseline, Some("deep"), None, None).unwrap();
    assert!(
        found.contains(&"deep: tests: test missing: deep_works".to_string()),
        "{found:?}"
    );
}

#[test]
fn a_stem_override_renames_the_new_home_only_and_the_shim_stays() {
    const OLD: &str = "crates/core/src/service/reports.rs";
    const NEW_PURE: &str = "crates/core/src/household/overview.rs";
    let rename = |text: &str| {
        text.replace(
            "files = [\"loyalty.rs\"]",
            "files = [\"reports.rs\"]\nstem = { \"reports.rs\" = \"overview\" }",
        )
    };
    let before = TempTree::new(&[
        (
            MAP_FILE,
            &rename(&std::fs::read_to_string(original_tree().root().join(MAP_FILE)).unwrap()),
        ),
        (
            OLD,
            &ORIGINAL.replace("service::loyalty", "service::reports"),
        ),
        (NEW_PURE, "//! placeholder\n"),
        (
            "crates/core/src/household/overview_tests.rs",
            "//! placeholder\n",
        ),
        (
            "crates/core/src/usecases/household/overview.rs",
            "//! placeholder\n",
        ),
    ]);
    let root = before.root();
    let workspace = Workspace::load(root).unwrap();
    let baseline = baseline::capture(root, &workspace).unwrap();
    let mut files = FileSet::new(root);
    relocate(&mut files, &workspace.map, "loyalty").unwrap();
    files.write().unwrap();
    let moved = std::collections::BTreeMap::from_iter(moved_files());
    for (path, new_path) in [
        (PURE, NEW_PURE),
        (SCENARIO, "crates/core/src/usecases/household/overview.rs"),
        (TESTS, "crates/core/src/household/overview_tests.rs"),
    ] {
        let text = moved[path].replace("loyalty", "overview");
        std::fs::write(root.join(new_path), text).unwrap();
    }
    let mut files = FileSet::new(root);
    forward(&mut files, &workspace.map, &baseline, "loyalty").unwrap();
    files.write().unwrap();
    let shim = std::fs::read_to_string(root.join(OLD)).unwrap();
    assert!(shim.contains("crate::household::overview::Card"), "{shim}");
    let workspace = Workspace::load(root).unwrap();
    let found = findings(
        root,
        &workspace,
        &baseline,
        Some("loyalty"),
        Some(&[]),
        None,
    );
    assert_eq!(found.unwrap(), Vec::<String>::new());
}

/// The moved fixture with a `remaining_dirs` tree: `crates/other/x.rs` names `UnlockedVault`
/// `recorded` times in `remaining.json` and `now` times on disk.
fn ratchet_tree(recorded: usize, now: usize) -> TempTree {
    let before = original_tree();
    let baseline =
        baseline::capture(before.root(), &Workspace::load(before.root()).unwrap()).unwrap();
    let words = |count: usize| vec!["UnlockedVault"; count].join(" ");
    let tree = moved_tree(|file, text| {
        if file == MAP_FILE {
            text.replace(
                "scan_dirs",
                "remaining_dirs = [\"crates/other\"]\nscan_dirs",
            )
        } else {
            text
        }
    });
    let other = tree.root().join("crates/other");
    std::fs::create_dir_all(&other).unwrap();
    std::fs::write(other.join("x.rs"), words(recorded)).unwrap();
    baseline.write(tree.root()).unwrap();
    std::fs::create_dir_all(tree.root().join("crates/xtask/strangle")).unwrap();
    remaining::run(tree.root(), &remaining::RemainingArgs { write: true }).unwrap();
    std::fs::write(other.join("x.rs"), words(now)).unwrap();
    tree
}

fn check_args(all: bool) -> CheckArgs {
    CheckArgs {
        area: (!all).then(|| "loyalty".to_string()),
        all,
        diff: None,
        task: None,
        write_baseline: false,
    }
}

#[test]
fn check_all_holds_the_remaining_record_and_check_of_one_area_does_not() {
    let tree = ratchet_tree(1, 2);
    let error = run(tree.root(), &check_args(true)).unwrap_err();
    assert!(format!("{error:#}").contains("remaining:"), "{error:#}");
    assert!(format!("{error:#}").contains("increased from 1 to 2"));
    run(tree.root(), &check_args(false)).unwrap();
}
