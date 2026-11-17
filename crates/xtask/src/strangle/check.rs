//! `strangle check`: holds a moved area against `baseline.json` (spec D6 step 6, Interfaces):
//!
//! 1. the baseline's public methods are the `forward!` entries of the shims;
//! 2. no `impl UnlockedVault` method with logic is left in the area's files;
//! 3. no `persist()` call is left in the area's files;
//! 4. the area's test names are the baseline's;
//! 5. (`--diff <base>`) the changed files are inside the task's allowed set.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};
use clap::Args;
use syn::visit::{self, Visit};
use syn::{Block, Expr, Stmt};

use super::Workspace;
use super::baseline::{self, AreaBaseline, Baseline, production_methods};
use super::forward::forwarded_names;
use super::home::{allowed_files, area_files, homes};
use super::map::Area;

#[derive(Args)]
pub struct CheckArgs {
    /// One area of `strangle.toml`.
    #[arg(required_unless_present_any = ["all", "write_baseline"])]
    pub area: Option<String>,
    /// Every area.
    #[arg(long, conflicts_with = "area")]
    pub all: bool,
    /// Also fail when a file outside the area's allowed set changed since this revision (jj).
    #[arg(long, value_name = "BASE", conflicts_with = "write_baseline")]
    pub diff: Option<String>,
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
    let areas: Vec<&Area> = match &args.area {
        Some(name) => vec![&workspace.map.areas[workspace.map.area_index(name)?]],
        None => workspace.map.areas.iter().collect(),
    };
    let mut findings = Vec::new();
    for area in areas {
        let subject = Subject {
            root,
            workspace: &workspace,
            area,
            baseline: baseline.area(&area.name)?,
            changed: changed.as_deref(),
        };
        findings.extend(check_area(&subject)?);
    }
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

/// What a rule looks at.
struct Subject<'run> {
    root: &'run Path,
    workspace: &'run Workspace,
    area: &'run Area,
    baseline: &'run AreaBaseline,
    /// Files changed since the base revision; `None` without `--diff`.
    changed: Option<&'run [String]>,
}

type Rule = fn(&Subject) -> Result<Vec<String>>;

const RULES: [(&str, Rule); 5] = [
    ("parity", parity),
    ("logic", no_logic),
    ("persist", no_persist),
    ("tests", test_names),
    ("files", allowed_set),
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

/// Rule 1: the baseline methods and the forward entries are the same set.
fn parity(subject: &Subject) -> Result<Vec<String>> {
    let mut forwarded = BTreeSet::new();
    for home in homes(&subject.workspace.map, subject.area) {
        let path = subject.root.join(home.shim());
        if let Ok(text) = std::fs::read_to_string(&path) {
            forwarded.extend(forwarded_names(&text).unwrap_or_default());
        }
    }
    let wanted: BTreeSet<String> = subject
        .baseline
        .methods
        .iter()
        .map(|method| method.name.clone())
        .collect();
    Ok(difference("not forwarded", &wanted, &forwarded)
        .into_iter()
        .chain(difference(
            "forwarded but not in the baseline",
            &forwarded,
            &wanted,
        ))
        .collect())
}

/// Rule 2: an `impl UnlockedVault` method may be left only as a plain delegate.
fn no_logic(subject: &Subject) -> Result<Vec<String>> {
    let mut found = Vec::new();
    for path in area_files(&subject.workspace.map, subject.area) {
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
    for path in area_files(&subject.workspace.map, subject.area) {
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
    Ok(difference("test missing", &wanted, &now)
        .into_iter()
        .chain(difference("test not in the baseline", &now, &wanted))
        .collect())
}

/// Rule 5: every changed file is one the area may change.
fn allowed_set(subject: &Subject) -> Result<Vec<String>> {
    let Some(changed) = subject.changed else {
        return Ok(Vec::new());
    };
    let allowed = allowed_files(&subject.workspace.map, subject.area);
    Ok(changed
        .iter()
        .filter(|path| !allowed.contains(*path))
        .map(|path| format!("{path} is outside the allowed set"))
        .collect())
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

/// The files that differ from `base` in the working copy, by path from the repository root.
fn changed_files(root: &Path, base: &str) -> Result<Vec<String>> {
    let output = Command::new("jj")
        .args(["diff", "--name-only", "--from", base])
        .current_dir(root)
        .output()
        .context("run `jj diff`")?;
    if !output.status.success() {
        bail!(
            "jj diff: {}",
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
        PURE, SCENARIO, SERVICE, TESTS, moved_tree, original_tree,
    };
    use crate::strangle::fixture::TempTree;

    /// The baseline of the original tree, then the findings on `after`.
    fn findings(after: &TempTree, changed: Option<&[String]>) -> Vec<String> {
        let before = original_tree();
        let baseline =
            baseline::capture(before.root(), &Workspace::load(before.root()).unwrap()).unwrap();
        let workspace = Workspace::load(after.root()).unwrap();
        let area = &workspace.map.areas[0];
        let subject = Subject {
            root: after.root(),
            workspace: &workspace,
            area,
            baseline: baseline.area("loyalty").unwrap(),
            changed,
        };
        check_area(&subject).unwrap()
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
        findings(&tree, Some(&[]))
    }

    #[test]
    fn the_good_fixture_is_green() {
        let tree = moved_tree(|_, text| text);
        let changed = [SERVICE.to_string(), PURE.to_string(), SCENARIO.to_string()];
        assert_eq!(findings(&tree, Some(&changed)), Vec::<String>::new());
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
        assert_eq!(
            found,
            [
                "loyalty: tests: test missing: clash_works",
                "loyalty: tests: test not in the baseline: clash_still_works"
            ]
        );
    }

    #[test]
    fn a_changed_file_outside_the_allowed_set_is_found() {
        let tree = moved_tree(|_, text| text);
        let changed = [
            PURE.to_string(),
            "crates/core/src/service/mod.rs".to_string(),
        ];
        assert_eq!(
            findings(&tree, Some(&changed)),
            ["loyalty: files: crates/core/src/service/mod.rs is outside the allowed set"]
        );
        // Without `--diff` the rule does not look.
        assert!(findings(&tree, None).is_empty());
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
            changed: None,
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
}
