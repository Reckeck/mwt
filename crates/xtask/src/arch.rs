//! `xtask arch`: the module graph of `crates/core/src`, read from the non-test `crate::` paths.

mod crates;
mod domain;
mod layers;
mod manifests;
mod scan;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

/// One `crate::` path: the file and line that names it, and the modules on both ends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    pub from_file: String,
    pub line: usize,
    pub from_module: String,
    pub to_module: String,
}

const CORE_SRC: &str = "crates/core/src";
const ALLOW_FILE: &str = "crates/xtask/arch-allow.toml";
/// Group of the modules gated by the `testkit` feature (see `layers::GROUPS`).
const TESTKIT: &str = "testkit";

/// A back edge: the same data as an edge, the ones the crate table does not allow.
pub type Violation = Edge;

/// One line of `arch-allow.toml`: back edges from `from` (under `crates/core/src`) to module `to`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AllowEntry {
    pub from: String,
    pub to: String,
    /// The task or phase that removes the edge.
    pub until: String,
    /// Why the edge stays until then.
    pub reason: String,
}

#[derive(Deserialize)]
struct AllowFile {
    allow: Vec<AllowEntry>,
}

fn parse_allowlist(text: &str) -> Result<Vec<AllowEntry>> {
    let file: AllowFile = toml::from_str(text).context("parse arch-allow.toml")?;
    Ok(file.allow)
}

impl AllowEntry {
    fn covers(&self, edge: &Edge) -> bool {
        self.from == edge.from_file && self.to == edge.to_module
    }
}

/// What the check found: back edges the allowlist does not cover, and entries that cover nothing.
#[derive(Debug)]
pub struct Report {
    pub violations: Vec<Violation>,
    pub stale: Vec<AllowEntry>,
    /// Forbidden names in the domain modules the exemptions do not cover, and exemptions that cover nothing.
    pub domain: domain::Findings,
    /// Crates that list a dev-only crate as a normal dependency (`manifests.rs`).
    pub manifests: Vec<String>,
    /// Workspace crate edges and external crates the crate table forbids (`crates.rs`).
    pub crates: Vec<String>,
}

impl Report {
    /// How many findings fail the check.
    pub fn failures(&self) -> usize {
        self.violations.len()
            + self.stale.len()
            + self.domain.failures()
            + self.manifests.len()
            + self.crates.len()
    }

    /// The log lines: every violation as `file:line`, every stale entry as an error.
    pub fn lines(&self) -> Vec<String> {
        let mut lines: Vec<String> = self
            .violations
            .iter()
            .map(|edge| {
                format!(
                    "error: {CORE_SRC}/{}:{}: `{}` must not use `{}` (edge the crate table forbids)",
                    edge.from_file, edge.line, edge.from_module, edge.to_module
                )
            })
            .collect();
        lines.extend(self.stale.iter().map(|entry| {
            format!(
                "error: {ALLOW_FILE}: stale entry {} -> {} (until {}): no such edge any more, delete it",
                entry.from, entry.to, entry.until
            )
        }));
        lines.extend(self.domain.lines(CORE_SRC));
        lines.extend(self.manifests.iter().cloned());
        lines.extend(self.crates.iter().cloned());
        lines
    }
}

/// Sorts the edges into back edges and allowed ones; entries that matched no back edge are stale.
fn evaluate(edges: &[Edge], allow: &[AllowEntry], domain: domain::Findings) -> Report {
    let back_edges: Vec<&Edge> = edges
        .iter()
        .filter(|edge| !layers::is_legal(&edge.from_module, &edge.to_module))
        .collect();
    let violations = back_edges
        .iter()
        .filter(|edge| !allow.iter().any(|entry| entry.covers(edge)))
        .map(|edge| (*edge).clone())
        .collect();
    let stale = allow
        .iter()
        .filter(|entry| !back_edges.iter().any(|edge| entry.covers(edge)))
        .cloned()
        .collect();
    Report {
        violations,
        stale,
        domain,
        manifests: Vec::new(),
        crates: Vec::new(),
    }
}

/// Scans the core crate under the repository `root` and holds it against the crate table and the allowlist.
pub fn check(root: &Path) -> Result<Report> {
    let sources = sources(&root.join(CORE_SRC))?;
    let edges = edges_of(&root.join(CORE_SRC), &sources)?;
    let domain = domain::check(&sources, domain::EXEMPTIONS);
    let path = root.join(ALLOW_FILE);
    let text = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let mut report = evaluate(&edges, &parse_allowlist(&text)?, domain);
    report.manifests = manifests::check(&root.join("crates"))?;
    report.crates = crates::check(root)?;
    Ok(report)
}

/// Directories outside the crate table: binaries are adapters, `tests` is test code.
fn is_skipped_dir(name: &str) -> bool {
    matches!(name, "bin" | "tests")
}

/// Test files by name: `tests.rs`, `*_tests.rs`, `test_*.rs`.
fn is_test_file(name: &str) -> bool {
    name == "tests.rs"
        || name.ends_with("_tests.rs")
        || (name.starts_with("test_") && name.ends_with(".rs"))
}

/// The top-level module a file belongs to: `model/vault.rs` and `model.rs` are `model`.
fn module_of(relative: &Path) -> String {
    let first = relative
        .components()
        .next()
        .map_or_else(String::new, |part| {
            part.as_os_str().to_string_lossy().into_owned()
        });
    first.strip_suffix(".rs").unwrap_or(&first).to_string()
}

/// The modules of the crate: its `.rs` files and directories under `root`, minus `lib` and the skipped ones.
fn module_names(root: &Path) -> Result<BTreeSet<String>> {
    let mut names = BTreeSet::new();
    for entry in fs::read_dir(root).with_context(|| format!("read {}", root.display()))? {
        let entry = entry?;
        let file_name = entry.file_name().to_string_lossy().into_owned();
        let is_dir = entry.file_type()?.is_dir();
        if (is_dir && !is_skipped_dir(&file_name))
            || (!is_dir && file_name.ends_with(".rs") && file_name != "lib.rs")
        {
            names.insert(module_of(Path::new(&file_name)));
        }
    }
    Ok(names)
}

/// Where the children of a module file live: `a/b.rs` -> `a/b/`, `a/mod.rs` and `lib.rs` -> their own directory.
fn child_dir(file: &Path) -> PathBuf {
    let parent = file.parent().map(Path::to_path_buf).unwrap_or_default();
    match file.file_stem().and_then(|stem| stem.to_str()) {
        Some("mod" | "lib") | None => parent,
        Some(stem) => parent.join(stem),
    }
}

/// The `.rs` files that count, found under `dir`; `skipped` holds the files of cfg-gated modules.
fn collect_files(dir: &Path, skipped: &mut Files, files: &mut Vec<PathBuf>) -> Result<()> {
    let mut entries = fs::read_dir(dir)
        .with_context(|| format!("read {}", dir.display()))?
        .collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(fs::DirEntry::file_name);
    let mut subdirs = Vec::new();
    for entry in entries {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if entry.file_type()?.is_dir() {
            if !is_skipped_dir(&name) {
                subdirs.push(path);
            }
        } else if name.ends_with(".rs") && !is_test_file(&name) {
            declare_gated_modules(&path, skipped)?;
            files.push(path);
        }
    }
    for subdir in subdirs {
        if skipped.test_dirs.contains(&subdir) {
            continue;
        }
        collect_files(&subdir, skipped, files)?;
    }
    Ok(())
}

/// Files of modules declared under a cfg gate: `#[cfg(test)]` ones are skipped, ones gated by the
/// `testkit` feature are test support and are checked as the `testkit` crate.
#[derive(Default)]
struct Files {
    test: BTreeSet<PathBuf>,
    test_dirs: BTreeSet<PathBuf>,
    testkit: BTreeSet<PathBuf>,
}

fn module_files(dir: &Path, name: &str) -> [PathBuf; 2] {
    [
        dir.join(format!("{name}.rs")),
        dir.join(name).join("mod.rs"),
    ]
}

/// Marks the files of the modules this file declares under `#[cfg(test)]` or the testkit gate.
fn declare_gated_modules(file: &Path, gated: &mut Files) -> Result<()> {
    let text = fs::read_to_string(file).with_context(|| format!("read {}", file.display()))?;
    let dir = child_dir(file);
    for name in scan::test_modules(&text) {
        gated.test.extend(module_files(&dir, &name));
        gated.test_dirs.insert(dir.join(&name));
    }
    for name in scan::testkit_modules(&text) {
        gated.testkit.extend(module_files(&dir, &name));
    }
    Ok(())
}

/// One non-test source file of the core crate.
pub struct Source {
    /// Path under `crates/core/src`, with `/`.
    pub file: String,
    /// The top-level module, or `testkit` for the files gated by the testkit feature.
    pub module: String,
    pub text: String,
}

/// Every source file under `root` (`crates/core/src`) that is not test code, in file order.
fn sources(root: &Path) -> Result<Vec<Source>> {
    let mut gated = Files::default();
    let mut files = Vec::new();
    collect_files(root, &mut gated, &mut files)?;
    let mut found = Vec::new();
    for file in files.iter().filter(|file| !gated.test.contains(*file)) {
        let relative = file.strip_prefix(root).unwrap_or(file);
        let text = fs::read_to_string(file).with_context(|| format!("read {}", file.display()))?;
        let module = if gated.testkit.contains(file) {
            TESTKIT.to_string()
        } else {
            module_of(relative)
        };
        found.push(Source {
            file: relative.to_string_lossy().replace('\\', "/"),
            module,
            text,
        });
    }
    Ok(found)
}

/// The names a `crate::<name>` path may resolve to: the directory modules plus every module of the
/// table, so names re-exported from extracted crates (`pub use mwt_workflow as workflow;`) stay visible.
fn edges_of(root: &Path, sources: &[Source]) -> Result<Vec<Edge>> {
    let mut modules = module_names(root)?;
    modules.extend(
        layers::GROUPS
            .iter()
            .flat_map(|group| group.modules.iter())
            .filter(|name| **name != "lib")
            .map(|name| (*name).to_string()),
    );
    Ok(sources
        .iter()
        .flat_map(|source| scan::read_edges(&source.file, &source.module, &source.text, &modules))
        .collect())
}

/// Every non-test `crate::` path under `root` (`crates/core/src`), in file order.
#[cfg(test)]
fn scan(root: &Path) -> Result<Vec<Edge>> {
    edges_of(root, &sources(root)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edge(from_file: &str, line: usize, from_module: &str, to_module: &str) -> Edge {
        Edge {
            from_file: from_file.to_string(),
            line,
            from_module: from_module.to_string(),
            to_module: to_module.to_string(),
        }
    }

    fn entry(from: &str, to: &str) -> AllowEntry {
        AllowEntry {
            from: from.to_string(),
            to: to.to_string(),
            until: "T9".to_string(),
            reason: "test".to_string(),
        }
    }

    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn every_module_of_core_is_in_the_crate_table() {
        let names = module_names(&repo_root().join(CORE_SRC)).unwrap();
        let missing: Vec<_> = names
            .iter()
            .filter(|name| layers::group_of(name).is_none())
            .collect();
        assert!(missing.is_empty(), "not in layers.rs: {missing:?}");
    }

    /// `name` of every `pub use mwt_<crate> as <name>;` and every lowercase item of `pub use mwt_base::{..}`.
    fn reexported_modules(lib: &str) -> Vec<String> {
        let mut names = Vec::new();
        for statement in lib.split(';') {
            const MARKER: &str = "pub use mwt_";
            let Some(start) = statement.find(MARKER) else {
                continue;
            };
            let rest = &statement[start + MARKER.len()..];
            if let Some((_, alias)) = rest.split_once(" as ") {
                names.push(alias.trim().to_string());
            } else if let Some(items) = rest.strip_prefix("base::{") {
                names.extend(
                    items
                        .trim_end_matches('}')
                        .split(',')
                        .map(str::trim)
                        .filter(|item| item.starts_with(|ch: char| ch.is_ascii_lowercase()))
                        .map(str::to_string),
                );
            }
        }
        names
    }

    #[test]
    fn reexported_modules_skip_nothing_after_other_text_and_ignore_restricted_use() {
        let lib = "/// plain docs, with a p\n#[cfg(test)]\npub use mwt_a as alpha;\n\
                   pub(crate) use mwt_x as hidden;\n\
                   // pending\npub use mwt_base::{beta, Gamma};\n";
        assert_eq!(reexported_modules(lib), ["alpha", "beta"]);
    }

    #[test]
    fn every_reexported_module_of_lib_is_a_module_of_the_table() {
        let lib = fs::read_to_string(repo_root().join(CORE_SRC).join("lib.rs")).unwrap();
        let names = reexported_modules(&lib);
        assert!(names.len() > 10, "{names:?}");
        let missing: Vec<_> = names
            .iter()
            .filter(|name| layers::group_of(name).is_none())
            .collect();
        assert!(missing.is_empty(), "not in layers.rs: {missing:?}");
    }

    #[test]
    fn a_name_reexported_from_an_extracted_crate_resolves_to_its_own_module() {
        let modules: BTreeSet<String> = layers::GROUPS
            .iter()
            .flat_map(|group| group.modules.iter())
            .map(|name| (*name).to_string())
            .collect();
        let edges = scan::read_edges(
            "planning/x.rs",
            "planning",
            "use crate::workflow::W;\nuse crate::Amount;\n",
            &modules,
        );
        let targets: Vec<&str> = edges.iter().map(|item| item.to_module.as_str()).collect();
        assert_eq!(targets, ["workflow", "amount"]);
        let report = evaluate(&edges, &[], domain::Findings::default());
        assert_eq!(report.violations.len(), 1);
        assert_eq!(report.violations[0].to_module, "workflow");
    }

    #[test]
    fn the_baseline_matches_the_tree() {
        let report = check(&repo_root()).unwrap();
        assert_eq!(report.failures(), 0, "{:#?}", report.lines());
    }

    #[test]
    fn a_downward_import_is_legal_and_an_upward_one_is_not() {
        assert!(layers::is_legal("service", "model"));
        assert!(layers::is_legal("workflow", "workflow"));
        assert!(!layers::is_legal("error", "service"));
        assert!(layers::is_legal("planning", "money"));
        assert!(!layers::is_legal("money", "planning"));
        assert!(!layers::is_legal("workflow", "money"));
        assert!(!layers::is_legal("sync", "storage"));
        assert!(!layers::is_legal("money", "pricing"));
    }

    #[test]
    fn modules_gated_by_the_testkit_feature_are_checked_as_the_testkit_crate() {
        let edges = scan(&repo_root().join(CORE_SRC)).unwrap();
        let fixtures: Vec<&Edge> = edges
            .iter()
            .filter(|edge| edge.from_file == "model/fixtures.rs")
            .collect();
        assert!(!fixtures.is_empty());
        assert!(fixtures.iter().all(|edge| edge.from_module == TESTKIT));
    }

    #[test]
    fn a_downward_import_is_not_reported() {
        let edges = [edge("service/x.rs", 1, "service", "model")];
        let report = evaluate(&edges, &[], domain::Findings::default());
        assert!(report.violations.is_empty());
        assert_eq!(report.failures(), 0);
    }

    #[test]
    fn is_test_file_matches_only_test_names() {
        for name in ["tests.rs", "vault_tests.rs", "test_vault.rs"] {
            assert!(is_test_file(name), "{name}");
        }
        for name in ["latest.rs", "attestation.rs", "contest.rs", "testkit.rs"] {
            assert!(!is_test_file(name), "{name}");
        }
    }

    #[test]
    fn a_cfg_test_module_declaration_skips_its_subdirectory() {
        let root = std::env::temp_dir().join(format!("xtask-arch-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("model/fixtures")).unwrap();
        fs::write(root.join("lib.rs"), "").unwrap();
        fs::write(
            root.join("model/mod.rs"),
            "#[cfg(test)]\nmod fixtures;\nuse crate::service::A;\n",
        )
        .unwrap();
        fs::write(
            root.join("model/fixtures/helper.rs"),
            "use crate::service::B;\n",
        )
        .unwrap();
        let edges = scan(&root).unwrap();
        fs::remove_dir_all(&root).unwrap();
        let files: Vec<&str> = edges.iter().map(|edge| edge.from_file.as_str()).collect();
        assert_eq!(files, vec!["model/mod.rs"]);
    }

    #[test]
    fn testkit_is_a_low_crate_and_nothing_below_the_facade_may_use_it() {
        assert!(layers::is_legal("testkit", "model"));
        assert!(!layers::is_legal("service", "testkit"));
    }

    #[test]
    fn an_allowed_edge_passes() {
        let edges = [
            edge("model/vault.rs", 405, "model", "exchange"),
            edge("model/vault.rs", 456, "model", "exchange"),
        ];
        let report = evaluate(
            &edges,
            &[entry("model/vault.rs", "exchange")],
            domain::Findings::default(),
        );
        assert!(report.violations.is_empty() && report.stale.is_empty());
        assert_eq!(report.failures(), 0);
    }

    #[test]
    fn a_violation_outside_the_list_fails_with_file_and_line() {
        let edges = [edge("error.rs", 131, "error", "service")];
        let report = evaluate(&edges, &[], domain::Findings::default());
        assert_eq!(report.violations, edges);
        assert!(report.lines()[0].contains("crates/core/src/error.rs:131"));
        assert_eq!(report.failures(), 1);
    }

    #[test]
    fn a_stale_entry_fails() {
        let allow = [entry("model/vault.rs", "exchange")];
        let report = evaluate(&[], &allow, domain::Findings::default());
        assert_eq!(report.stale, allow);
        assert_eq!(report.failures(), 1);
        assert!(report.lines()[0].starts_with("error:"));
    }

    #[test]
    fn the_allowlist_file_parses() {
        let text = fs::read_to_string(repo_root().join(ALLOW_FILE)).unwrap();
        let entries = parse_allowlist(&text).unwrap();
        assert!(
            entries
                .iter()
                .all(|item| !item.until.is_empty() && !item.reason.is_empty())
        );
    }
}
