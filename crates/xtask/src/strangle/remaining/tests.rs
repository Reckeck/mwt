use super::count::{count_word, forward_entries, service_paths};
use super::*;
use crate::strangle::fixture::TempTree;

const MAP: &str = r#"
service_dir = "crates/core/src/service"
command_dir = "crates/app/src"
scan_dirs = []
remaining_dirs = ["crates"]
remaining_exclude = ["crates/tool/"]

[[area]]
name = "alpha"
task = "T1"
group = "g"
context = "c"
"#;

const SHIM: &str = "forward! {
    pub fn one(&self)
        => crate::a::one;
    pub fn two(&self)
        => crate::a::two;
}
// => not counted outside a block
";

fn map() -> Map {
    Map::parse(MAP).unwrap()
}

/// A tree with the record of `files` written; `edit` changes the sources afterwards.
fn tree(sources: &[(&str, &str)]) -> TempTree {
    TempTree::new(sources)
}

fn record(tree: &TempTree) {
    let measured = count::measure(tree.root(), &map()).unwrap();
    std::fs::create_dir_all(tree.root().join("crates/xtask/strangle")).unwrap();
    measured.write(tree.root()).unwrap();
}

fn findings_of(tree: &TempTree) -> Vec<String> {
    findings(tree.root(), &map()).unwrap()
}

const BASE: [(&str, &str); 2] = [
    (
        "crates/a/src/lib.rs",
        "use UnlockedVault;\nuse crate::service::x;\n",
    ),
    ("crates/tool/src/lib.rs", "UnlockedVault UnlockedVault\n"),
];

#[test]
fn counts_follow_the_three_definitions() {
    assert_eq!(
        count_word(
            "UnlockedVault MyUnlockedVault UnlockedVaultX UnlockedVault",
            "UnlockedVault"
        ),
        2
    );
    assert_eq!(forward_entries(SHIM), 2);
    assert_eq!(
        service_paths(
            "use crate::service::a; use trade_core::service::b; super::service::c; super::super::service::d; mycrate::service::e; service::f"
        ),
        4
    );
}

#[test]
fn the_service_dir_and_the_excluded_paths_are_not_counted() {
    let tree = tree(&[
        (
            "crates/core/src/service/mod.rs",
            "use crate::service::x; UnlockedVault",
        ),
        ("crates/tool/src/lib.rs", "UnlockedVault"),
        ("crates/a/src/lib.rs", "UnlockedVault crate::service::y"),
        ("crates/a/target/skip.rs", "UnlockedVault"),
    ]);
    let measured = count::measure(tree.root(), &map()).unwrap();
    assert_eq!(
        measured.vault,
        PerFile::from([
            ("crates/a/src/lib.rs".to_string(), 1),
            ("crates/core/src/service/mod.rs".to_string(), 1),
        ])
    );
    assert_eq!(
        measured.service_paths,
        PerFile::from([("crates/a/src/lib.rs".to_string(), 1)])
    );
}

#[test]
fn an_unchanged_tree_is_clean() {
    let tree = tree(&BASE);
    record(&tree);
    assert_eq!(findings_of(&tree), Vec::<String>::new());
}

#[test]
fn a_per_file_increase_is_red() {
    let tree = tree(&BASE);
    record(&tree);
    std::fs::write(
        tree.root().join("crates/a/src/lib.rs"),
        "UnlockedVault UnlockedVault\ncrate::service::x\n",
    )
    .unwrap();
    assert_eq!(
        findings_of(&tree),
        ["remaining: vault: crates/a/src/lib.rs: increased from 1 to 2"]
    );
}

#[test]
fn a_new_file_above_zero_is_red() {
    let tree = tree(&BASE);
    record(&tree);
    std::fs::write(tree.root().join("crates/a/src/new.rs"), "UnlockedVault").unwrap();
    assert_eq!(
        findings_of(&tree),
        ["remaining: vault: crates/a/src/new.rs: new file with 1 (not recorded)"]
    );
}

#[test]
fn an_unrecorded_decrease_is_red_and_names_write() {
    let tree = tree(&BASE);
    record(&tree);
    std::fs::write(
        tree.root().join("crates/a/src/lib.rs"),
        "use crate::service::x;\n",
    )
    .unwrap();
    let found = findings_of(&tree);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].contains("lower") && found[0].contains("remaining --write"),
        "{found:?}"
    );
}

#[test]
fn write_records_a_decrease_and_refuses_a_raise() {
    let tree = tree(&BASE);
    record(&tree);
    std::fs::write(
        tree.root().join("crates/a/src/lib.rs"),
        "use crate::service::x;\n",
    )
    .unwrap();
    write(tree.root(), &count::measure(tree.root(), &map()).unwrap()).unwrap();
    assert_eq!(findings_of(&tree), Vec::<String>::new());

    std::fs::write(
        tree.root().join("crates/a/src/lib.rs"),
        "UnlockedVault crate::service::x\n",
    )
    .unwrap();
    let error = write(tree.root(), &count::measure(tree.root(), &map()).unwrap()).unwrap_err();
    assert!(error.to_string().contains("refuses to raise"), "{error}");
    assert_eq!(Remaining::load(tree.root()).unwrap().vault, PerFile::new());
}
