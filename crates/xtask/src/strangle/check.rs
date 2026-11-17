//! `strangle check`: holds a moved area against `baseline.json` (spec D6 step 6, Interfaces):
//!
//! 1. the baseline's public methods are the `forward!` entries of the shims;
//! 2. no `impl UnlockedVault` method with logic is left in the area's files;
//! 3. no `persist()` call is left in the area's files;
//! 4. every baseline test name is still in the area (new tests are welcome, none may be dropped);
//! 5. (`--diff <base>`) the changed files are inside the task's allowed set.
//!
//! `check <area>` runs every rule on that area. `check --all` runs rules 1-4 only on the areas that
//! have moved (a shim with the forward markers, `stubs = true`) and rule 5 once, per task (module
//! `scope`: the diff must fit one task, or the one named by `--task`): it is the CI gate, so an
//! area that has not moved is not red.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};
use clap::Args;
use syn::visit::{self, Visit};
use syn::{Block, Expr, Stmt};

use super::Workspace;
use super::baseline::{self, AreaBaseline, Baseline, production_methods};
use super::forward::forwarded_entries;
use super::home::{allowed_files, area_files, homes};
use super::map::Area;
use super::scope;

#[derive(Args)]
pub struct CheckArgs {
    /// One area of `strangle.toml`.
    #[arg(required_unless_present_any = ["all", "write_baseline"])]
    pub area: Option<String>,
    /// Every area that has moved.
    #[arg(long, conflicts_with = "area")]
    pub all: bool,
    /// Also fail when a file outside the allowed set changed since this revision (`git diff`).
    #[arg(long, value_name = "BASE", conflicts_with = "write_baseline")]
    pub diff: Option<String>,
    /// With `--all --diff`: the task the changed files must belong to (else: any single task).
    #[arg(long, requires = "all")]
    pub task: Option<String>,
    /// Write `baseline.json` from the tree as it is now (once, before the areas move).
    #[arg(long, conflicts_with_all = ["area", "all"])]
    pub write_baseline: bool,
}

pub fn run(root: &Path, args: &CheckArgs) -> Result<()> {
    let workspace = Workspace::load(root)?;
    if args.write_baseline {
        let baseline = baseline::capture(root, &workspace)?;
        baseline.write(root)?;
        let methods: usize = baseline.areas.values().map(|area| area.methods.len()).sum();
        println!(
            "check: wrote {} ({} areas, {methods} methods)",
            baseline::BASELINE_FILE,
            baseline.areas.len()
        );
        return Ok(());
    }
    let baseline = Baseline::load(root)?;
    let changed = args
        .diff
        .as_deref()
        .map(|base| changed_files(root, base))
        .transpose()?;
    let findings = findings(
        root,
        &workspace,
        &baseline,
        args.area.as_deref(),
        changed.as_deref(),
        args.task.as_deref(),
    )?;
    if findings.is_empty() {
        println!("check: ok");
        return Ok(());
    }
    bail!(
        "check: {} finding(s)\n  {}",
        findings.len(),
        findings.join("\n  ")
    )
}

/// Every finding for one area (`only`) or for the moved areas, and rule 5 for the changed files.
fn findings(
    root: &Path,
    workspace: &Workspace,
    baseline: &Baseline,
    only: Option<&str>,
    changed: Option<&[String]>,
    task: Option<&str>,
) -> Result<Vec<String>> {
    let areas: Vec<&Area> = match only {
        Some(name) => vec![&workspace.map.areas[workspace.map.area_index(name)?]],
        None => workspace
            .map
            .areas
            .iter()
            .filter(|area| has_moved(root, workspace, area))
            .collect(),
    };
    let mut found = Vec::new();
    for area in &areas {
        let subject = Subject {
            root,
            workspace,
            area,
            baseline: baseline.area(&area.name)?,
        };
        found.extend(check_area(&subject)?);
    }
    if let Some(changed) = changed {
        match only {
            Some(name) => {
                let allowed: BTreeSet<String> = areas
                    .iter()
                    .flat_map(|area| allowed_files(workspace, area))
                    .collect();
                found.extend(outside_allowed(name, changed, &allowed));
            }
            None => found.extend(scope::findings(workspace, changed, task)),
        }
    }
    Ok(found)
}

/// An area has moved when it gets stubs and one of its shims carries the forward markers.
fn has_moved(root: &Path, workspace: &Workspace, area: &Area) -> bool {
    area.stubs
        && homes(&workspace.map, area).iter().any(|home| {
            std::fs::read_to_string(root.join(home.shim()))
                .is_ok_and(|text| forwarded_entries(&text).is_some())
        })
}

/// What a rule looks at.
struct Subject<'run> {
    root: &'run Path,
    workspace: &'run Workspace,
    area: &'run Area,
    baseline: &'run AreaBaseline,
}

type Rule = fn(&Subject) -> Result<Vec<String>>;

const RULES: [(&str, Rule); 4] = [
    ("parity", parity),
    ("logic", no_logic),
    ("persist", no_persist),
    ("tests", test_names),
];

/// Every finding of every rule, as `<area>: <rule>: <detail>`.
fn check_area(subject: &Subject) -> Result<Vec<String>> {
    let mut findings = Vec::new();
    for (name, rule) in RULES {
        for detail in rule(subject)? {
            findings.push(format!("{}: {name}: {detail}", subject.area.name));
        }
    }
    Ok(findings)
}

/// The signature without `mut` on parameter bindings (`mut name: T`): the delegate leaves the
/// binding out, so both sides are compared without it. `&mut T` and `&mut self` stay.
fn without_binding_mut(signature: &str) -> String {
    let mut text = String::with_capacity(signature.len());
    let mut rest = signature;
    while let Some(at) = rest.find("mut ") {
        let (before, after) = rest.split_at(at);
        text.push_str(before);
        let name_len = after[4..]
            .find(|ch: char| !(ch.is_alphanumeric() || ch == '_'))
            .unwrap_or(0);
        let starts_param = text.ends_with('(') || text.ends_with(", ");
        let is_binding = starts_param && name_len > 0 && after[4 + name_len..].starts_with(':');
        if !is_binding {
            text.push_str("mut ");
        }
        rest = &after[4..];
    }
    text.push_str(rest);
    text
}

/// Rule 1: the baseline methods and the forward entries are the same set, with the same signatures.
fn parity(subject: &Subject) -> Result<Vec<String>> {
    let mut forwarded = BTreeMap::new();
    for home in homes(&subject.workspace.map, subject.area) {
        let path = subject.root.join(home.shim());
        if let Ok(text) = std::fs::read_to_string(&path) {
            for entry in forwarded_entries(&text).unwrap_or_default() {
                forwarded.insert(entry.name, entry.signature);
            }
        }
    }
    let wanted: BTreeMap<String, String> = subject
        .baseline
        .methods
        .iter()
        .map(|method| (method.name.clone(), method.signature.clone()))
        .collect();
    let (wanted_names, forwarded_names) = (names(&wanted), names(&forwarded));
    let differing = wanted.iter().filter_map(|(name, old)| {
        let new = forwarded.get(name)?;
        (without_binding_mut(old) != without_binding_mut(new))
            .then(|| format!("signature differs: {name}: {old} vs {new}"))
    });
    Ok(difference("not forwarded", &wanted_names, &forwarded_names)
        .into_iter()
        .chain(difference(
            "forwarded but not in the baseline",
            &forwarded_names,
            &wanted_names,
        ))
        .chain(differing)
        .collect())
}

fn names(entries: &BTreeMap<String, String>) -> BTreeSet<String> {
    entries.keys().cloned().collect()
}

/// Rule 2: an `impl UnlockedVault` method may be left only as a plain delegate.
fn no_logic(subject: &Subject) -> Result<Vec<String>> {
    let mut found = Vec::new();
    for path in area_files(subject.workspace, subject.area) {
        let Ok(source) = std::fs::read_to_string(subject.root.join(&path)) else {
            continue;
        };
        let file = syn::parse_file(&source).with_context(|| format!("parse {path}"))?;
        found.extend(
            production_methods(&file)
                .into_iter()
                .filter(|method| has_logic(&method.block))
                .map(|method| format!("{path}: `{}` has a body with logic", method.sig.ident)),
        );
    }
    Ok(found)
}

/// Rule 3: no `persist()` call outside tests.
fn no_persist(subject: &Subject) -> Result<Vec<String>> {
    let mut found = Vec::new();
    for path in area_files(subject.workspace, subject.area) {
        let Some(scanned) = subject.workspace.corpus.get(&path) else {
            continue;
        };
        if scanned.is_test {
            continue;
        }
        found.extend(
            scanned
                .facts
                .calls
                .iter()
                .filter(|call| call.name == "persist" && !call.in_test)
                .map(|call| format!("{path}:{}: persist()", call.line)),
        );
    }
    Ok(found)
}

/// Rule 4: the test names are the baseline's.
fn test_names(subject: &Subject) -> Result<Vec<String>> {
    let now = baseline::test_names(subject.workspace, subject.area);
    let wanted: BTreeSet<String> = subject.baseline.tests.iter().cloned().collect();
    Ok(difference("test missing", &wanted, &now))
}

/// Rule 5: every changed file is one of the allowed set.
fn outside_allowed(label: &str, changed: &[String], allowed: &BTreeSet<String>) -> Vec<String> {
    changed
        .iter()
        .filter(|path| !allowed.contains(*path))
        .map(|path| format!("{label}: files: {path} is outside the allowed set"))
        .collect()
}

/// `<what>: <name>` for every name of `left` that `right` lacks.
fn difference(what: &str, left: &BTreeSet<String>, right: &BTreeSet<String>) -> Vec<String> {
    left.difference(right)
        .map(|name| format!("{what}: {name}"))
        .collect()
}

/// A body is a plain delegate when it is one call without control flow in it.
fn has_logic(block: &Block) -> bool {
    match block.stmts.as_slice() {
        [] => false,
        [Stmt::Expr(expr @ (Expr::Call(_) | Expr::MethodCall(_)), _)] => {
            let mut flow = ControlFlow(false);
            flow.visit_expr(expr);
            flow.0
        }
        _ => true,
    }
}

/// Finds a branch, loop, closure or nested block.
struct ControlFlow(bool);

impl<'ast> Visit<'ast> for ControlFlow {
    fn visit_expr(&mut self, node: &'ast Expr) {
        if matches!(
            node,
            Expr::If(_)
                | Expr::Match(_)
                | Expr::While(_)
                | Expr::ForLoop(_)
                | Expr::Loop(_)
                | Expr::Closure(_)
                | Expr::Block(_)
                | Expr::Unsafe(_)
                | Expr::Async(_)
        ) {
            self.0 = true;
        }
        visit::visit_expr(self, node);
    }
}

/// The files changed from `base` to `HEAD`, by path from the repository root (`git` also reads a
/// colocated jj repository).
fn changed_files(root: &Path, base: &str) -> Result<Vec<String>> {
    let output = Command::new("git")
        .args(["diff", "--name-only", &format!("{base}...HEAD")])
        .current_dir(root)
        .output()
        .context("run `git diff`")?;
    if !output.status.success() {
        bail!(
            "git diff: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_string)
        .collect())
}

#[cfg(test)]
mod tests {
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

    #[test]
    fn an_impl_body_with_logic_is_found_and_a_plain_delegate_is_not() {
        let logic = "\nimpl UnlockedVault {\n    pub fn x(&self) -> u8 {\n        if true { 1 } else { 2 }\n    }\n}\n";
        let found = broken(
            PURE,
            "pub struct Card {",
            &format!("{logic}pub struct Card {{"),
        );
        assert_eq!(
            found,
            [format!("loyalty: logic: {PURE}: `x` has a body with logic")]
        );
        let delegate = "\nimpl UnlockedVault {\n    pub fn x(&self) -> u8 {\n        crate::household::loyalty::go(self)\n    }\n}\n";
        assert!(
            broken(
                PURE,
                "pub struct Card {",
                &format!("{delegate}pub struct Card {{")
            )
            .is_empty()
        );
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

    fn all_findings(
        tree: &TempTree,
        baseline: &Baseline,
        changed: Option<&[String]>,
    ) -> Vec<String> {
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
}
