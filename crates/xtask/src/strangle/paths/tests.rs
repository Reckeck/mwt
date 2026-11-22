use super::*;
use crate::strangle::fixture::TempTree;
use crate::strangle::map::MAP_FILE;

const MAP: &str = r#"
service_dir = "crates/core/src/service"
command_dir = "crates/app/src"
scan_dirs = ["crates/core/src"]

[[area]]
name = "own"
task = "T1"
group = "g"
context = "c"
files = ["own.rs"]
"#;

const SERVICE: &str = "crates/core/src/service";
const USER: &str = "crates/core/src/usecases/user.rs";
const OUTSIDE: &str = "crates/app/src/main.rs";

fn tree(user: &str) -> TempTree {
    let modules = "mod datasources;\nmod budget;\nmod dup_a;\nmod dup_b;\nmod own;\nmod globbed;\nmod sibling;\n\
pub use datasources::{DataSourceDraft, is_due as source_is_due};\n\
pub(crate) use budget::MIN_HISTORY_MONTHS;\n\
pub use dup_a::Both;\npub use dup_b::Both;\npub use own::Draft;\npub use globbed::Wild;\n";
    TempTree::new(&[
        (MAP_FILE, MAP),
        (&format!("{SERVICE}/mod.rs"), modules),
        (
            &format!("{SERVICE}/datasources.rs"),
            "pub use mwt_planning::datasources::{DataSourceDraft, is_due};\n",
        ),
        (
            &format!("{SERVICE}/budget.rs"),
            "pub use crate::planning::budget::MIN_HISTORY_MONTHS;\n",
        ),
        (&format!("{SERVICE}/dup_a.rs"), "pub use mwt_a::Both;\n"),
        (&format!("{SERVICE}/dup_b.rs"), "pub use mwt_b::Both;\n"),
        (&format!("{SERVICE}/own.rs"), "pub struct Draft;\n"),
        (&format!("{SERVICE}/globbed.rs"), "pub use mwt_wild::*;\n"),
        (
            &format!("{SERVICE}/sibling.rs"),
            "use super::DataSourceDraft;\nuse super::{Draft, helper};\nfn f(_: super::MIN_HISTORY_MONTHS) {}\n\
#[cfg(test)]\nmod tests {\n    use super::super::DataSourceDraft as Again;\n    use super::*;\n}\n",
        ),
        (USER, user),
        (OUTSIDE, user),
    ])
}

fn planned(path: &str, user: &str) -> (Report, String) {
    let tree = tree(user);
    let map = Map::load(tree.root()).unwrap();
    let mut files = FileSet::new(tree.root());
    let args = PathsArgs {
        dir: path.into(),
        dry_run: false,
    };
    let report = rewrite_dir(tree.root(), &mut files, &map, &args).unwrap();
    let text = files.read(path).unwrap().to_string();
    (report, text)
}

#[test]
fn a_grouped_use_gets_one_line_per_name_and_keeps_the_refused_ones() {
    let (report, text) = planned(
        USER,
        "use crate::service::{DataSourceDraft, Draft, MIN_HISTORY_MONTHS};\n",
    );
    assert!(
        text.contains("use mwt_planning::datasources::DataSourceDraft;"),
        "{text}"
    );
    assert!(
        text.contains("use crate::planning::budget::MIN_HISTORY_MONTHS;"),
        "{text}"
    );
    assert!(text.contains("use crate::service::Draft;"), "{text}");
    assert_eq!(report.rewrites.len(), 2, "{:?}", report.rewrites);
    assert_eq!(report.skipped.len(), 1, "{:?}", report.skipped);
    assert!(report.skipped[0].contains("Draft"));
    assert!(report.skipped[0].contains("defined inside service/"));
}

#[test]
fn a_nested_group_and_a_foreign_branch_are_split() {
    let (report, text) = planned(
        USER,
        "use crate::{model::Thing, service::{DataSourceDraft, source_is_due}};\n",
    );
    assert!(text.contains("use crate::model::Thing;"), "{text}");
    assert!(
        text.contains("use mwt_planning::datasources::DataSourceDraft;"),
        "{text}"
    );
    assert_eq!(report.rewrites.len(), 2);
    assert!(report.skipped.is_empty());
}

#[test]
fn an_alias_of_the_shim_keeps_the_name_the_file_uses() {
    let (_, text) = planned(
        USER,
        "use crate::service::source_is_due;\nuse crate::service::DataSourceDraft as Source;\n",
    );
    assert!(
        text.contains("use mwt_planning::datasources::is_due as source_is_due;"),
        "{text}"
    );
    assert!(
        text.contains("use mwt_planning::datasources::DataSourceDraft as Source;"),
        "{text}"
    );
}

#[test]
fn pub_crate_and_attributes_stay_on_every_new_line() {
    let (_, text) = planned(
        USER,
        "#[allow(unused)]\npub(crate) use crate::service::{DataSourceDraft, MIN_HISTORY_MONTHS};\n",
    );
    assert_eq!(
        text.matches("#[allow(unused)]\npub(crate) use ").count(),
        2,
        "{text}"
    );
}

#[test]
fn a_path_in_an_expression_and_a_type_is_rewritten() {
    let (report, text) = planned(
        USER,
        "fn f() -> crate::service::DataSourceDraft {\n    crate::service::DataSourceDraft::new(crate::service::source_is_due(1))\n}\n",
    );
    assert!(
        text.contains("-> mwt_planning::datasources::DataSourceDraft {"),
        "{text}"
    );
    assert!(
        text.contains(
            "mwt_planning::datasources::DataSourceDraft::new(mwt_planning::datasources::is_due(1))"
        ),
        "{text}"
    );
    assert_eq!(report.rewrites.len(), 3);
}

#[test]
fn a_name_with_two_owners_and_a_glob_are_refused_and_listed() {
    let (report, text) = planned(
        USER,
        "use crate::service::{Both, Wild};\nfn f() { crate::service::Both::new(); }\n",
    );
    assert!(report.rewrites.is_empty());
    assert!(text.contains("crate::service::Both::new()"), "{text}");
    let both: Vec<_> = report
        .skipped
        .iter()
        .filter(|line| line.contains("Both"))
        .collect();
    assert_eq!(both.len(), 2, "{:?}", report.skipped);
    assert!(
        both[0].contains("two owners (mwt_a::Both | mwt_b::Both)"),
        "{both:?}"
    );
    assert!(report.skipped.iter().any(|line| line.contains("glob")));
}

#[test]
fn super_names_a_shim_type_only_where_super_is_the_service_module() {
    let (report, text) = planned(&format!("{SERVICE}/sibling.rs"), "");
    assert!(
        text.contains("use mwt_planning::datasources::DataSourceDraft;"),
        "{text}"
    );
    assert!(
        text.contains("_: crate::planning::budget::MIN_HISTORY_MONTHS"),
        "{text}"
    );
    assert!(
        text.contains("use mwt_planning::datasources::DataSourceDraft as Again;"),
        "{text}"
    );
    assert!(text.contains("use super::{Draft, helper};"), "{text}");
    assert!(text.contains("use super::*;"), "{text}");
    assert_eq!(report.skipped.len(), 1, "{:?}", report.skipped);
}

#[test]
fn super_outside_the_service_dir_is_left_alone() {
    let (report, text) = planned(USER, "use super::DataSourceDraft;\n");
    assert_eq!(text, "use super::DataSourceDraft;\n");
    assert!(report.rewrites.is_empty());
}

#[test]
fn a_file_outside_the_core_crate_names_the_core_by_its_crate_name() {
    let (_, text) = planned(
        OUTSIDE,
        "use trade_core::service::{MIN_HISTORY_MONTHS, DataSourceDraft};\nuse crate::service::Draft;\n",
    );
    assert!(
        text.contains("use trade_core::planning::budget::MIN_HISTORY_MONTHS;"),
        "{text}"
    );
    assert!(
        text.contains("use mwt_planning::datasources::DataSourceDraft;"),
        "{text}"
    );
    assert!(text.contains("use crate::service::Draft;"), "{text}");
}

#[test]
fn macro_arguments_are_rewritten_and_other_macro_bodies_are_listed() {
    let (report, text) = planned(
        USER,
        "fn f() {\n    let a = format!(\"{}\", crate::service::source_is_due(1));\n    odd!(=> crate::service::source_is_due(2));\n}\n",
    );
    assert!(
        text.contains("format!(\"{}\", mwt_planning::datasources::is_due(1))"),
        "{text}"
    );
    assert_eq!(report.skipped.len(), 1, "{:?}", report.skipped);
    assert!(report.skipped[0].contains("macro body"));
}

#[test]
fn dry_run_writes_nothing_and_a_real_run_formats() {
    let source = "use crate::service::{DataSourceDraft,   MIN_HISTORY_MONTHS};\n";
    let tree = tree(source);
    let dry = PathsArgs {
        dir: "crates/core/src/usecases".into(),
        dry_run: true,
    };
    run(tree.root(), &dry).unwrap();
    assert_eq!(
        std::fs::read_to_string(tree.root().join(USER)).unwrap(),
        source
    );
    let real = PathsArgs {
        dir: "crates/core/src/usecases".into(),
        dry_run: false,
    };
    run(tree.root(), &real).unwrap();
    let text = std::fs::read_to_string(tree.root().join(USER)).unwrap();
    assert_eq!(
        text,
        "use crate::planning::budget::MIN_HISTORY_MONTHS;\nuse mwt_planning::datasources::DataSourceDraft;\n"
    );
}
