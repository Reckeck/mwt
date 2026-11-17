//! `xtask arch`: the module graph of `crates/core/src`, read from the non-test `crate::` paths.

mod layers;
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
/// Flipped by core-edges T15: until then a stale allowlist entry only warns.
const STALE_IS_ERROR: bool = false;

/// A back edge: the same data as an edge, the ones the layer table does not allow.
pub type Violation = Edge;

/// One line of `arch-allow.toml`: back edges from `from` (under `crates/core/src`) to module `to`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AllowEntry {
    pub from: String,
    pub to: String,
    /// The task or phase that removes the edge.
    pub until: String,
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
    stale_is_error: bool,
}

impl Report {
    /// How many findings fail the check.
    pub fn failures(&self) -> usize {
        self.violations.len()
            + if self.stale_is_error {
                self.stale.len()
            } else {
                0
            }
    }

    /// The log lines: every violation as `file:line`, every stale entry as a warning or an error.
    pub fn lines(&self) -> Vec<String> {
        let mut lines: Vec<String> = self
            .violations
            .iter()
            .map(|edge| {
                format!(
                    "error: {CORE_SRC}/{}:{}: `{}` must not use `{}` (back edge of the layer table)",
                    edge.from_file, edge.line, edge.from_module, edge.to_module
                )
            })
            .collect();
        let label = if self.stale_is_error {
            "error"
        } else {
            "warning"
        };
        lines.extend(self.stale.iter().map(|entry| {
            format!(
                "{label}: {ALLOW_FILE}: stale entry {} -> {} (until {}): no such edge any more, delete it",
                entry.from, entry.to, entry.until
            )
        }));
        lines
    }
}

/// Sorts the edges into back edges and allowed ones; entries that matched no back edge are stale.
fn evaluate(edges: &[Edge], allow: &[AllowEntry], stale_is_error: bool) -> Report {
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
        stale_is_error,
    }
}

/// Scans the core crate under the repository `root` and holds it against the layer table and the allowlist.
pub fn check(root: &Path) -> Result<Report> {
    let edges = scan(&root.join(CORE_SRC))?;
    let path = root.join(ALLOW_FILE);
    let text = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    Ok(evaluate(&edges, &parse_allowlist(&text)?, STALE_IS_ERROR))
}

/// Directories outside the layer table: binaries are adapters, `tests` is test code.
fn is_skipped_dir(name: &str) -> bool {
    matches!(name, "bin" | "tests")
}

fn is_test_file(name: &str) -> bool {
    name.contains("test")
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

/// The `.rs` files that count, found under `dir`; `skipped` holds files declared under `#[cfg(test)]`.
fn collect_files(
    dir: &Path,
    skipped: &mut BTreeSet<PathBuf>,
    files: &mut Vec<PathBuf>,
) -> Result<()> {
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
            declare_test_modules(&path, skipped)?;
            files.push(path);
        }
    }
    for subdir in subdirs {
        collect_files(&subdir, skipped, files)?;
    }
    Ok(())
}

/// Marks the files of the modules this file declares under `#[cfg(test)]`.
fn declare_test_modules(file: &Path, skipped: &mut BTreeSet<PathBuf>) -> Result<()> {
    let text = fs::read_to_string(file).with_context(|| format!("read {}", file.display()))?;
    let dir = child_dir(file);
    for name in scan::test_modules(&text) {
        skipped.insert(dir.join(format!("{name}.rs")));
        skipped.insert(dir.join(&name).join("mod.rs"));
    }
    Ok(())
}

/// Every non-test `crate::` path under `root` (`crates/core/src`), in file order.
pub fn scan(root: &Path) -> Result<Vec<Edge>> {
    let modules = module_names(root)?;
    let mut skipped = BTreeSet::new();
    let mut files = Vec::new();
    collect_files(root, &mut skipped, &mut files)?;
    let mut edges = Vec::new();
    for file in files.iter().filter(|file| !skipped.contains(*file)) {
        let relative = file.strip_prefix(root).unwrap_or(file);
        let text = fs::read_to_string(file).with_context(|| format!("read {}", file.display()))?;
        edges.extend(scan::read_edges(
            &relative.to_string_lossy().replace('\\', "/"),
            &module_of(relative),
            &text,
            &modules,
        ));
    }
    Ok(edges)
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
        }
    }

    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn every_module_of_core_is_in_the_layer_table() {
        let names = module_names(&repo_root().join(CORE_SRC)).unwrap();
        let missing: Vec<_> = names
            .iter()
            .filter(|name| layers::group_of(name).is_none())
            .collect();
        assert!(missing.is_empty(), "not in layers.rs: {missing:?}");
    }

    #[test]
    fn the_baseline_matches_the_tree() {
        let report = check(&repo_root()).unwrap();
        assert!(report.violations.is_empty(), "{:#?}", report.lines());
    }

    #[test]
    fn a_downward_import_is_legal_and_an_upward_one_is_not() {
        assert!(layers::is_legal("service", "model"));
        assert!(layers::is_legal("workflow", "workflow"));
        assert!(!layers::is_legal("error", "service"));
        assert!(!layers::is_legal("planning", "money"));
        assert!(!layers::is_legal("sync", "storage"));
    }

    #[test]
    fn testkit_sits_on_top_and_nothing_below_may_use_it() {
        assert!(layers::is_legal("testkit", "service"));
        assert!(!layers::is_legal("service", "testkit"));
    }

    #[test]
    fn an_allowed_edge_passes() {
        let edges = [
            edge("model/vault.rs", 405, "model", "exchange"),
            edge("model/vault.rs", 456, "model", "exchange"),
        ];
        let report = evaluate(&edges, &[entry("model/vault.rs", "exchange")], false);
        assert!(report.violations.is_empty() && report.stale.is_empty());
        assert_eq!(report.failures(), 0);
    }

    #[test]
    fn a_violation_outside_the_list_fails_with_file_and_line() {
        let edges = [edge("error.rs", 131, "error", "service")];
        let report = evaluate(&edges, &[entry("model/vault.rs", "exchange")], false);
        assert_eq!(report.violations, edges);
        assert!(report.lines()[0].contains("crates/core/src/error.rs:131"));
        assert_eq!(report.failures(), 1);
    }

    #[test]
    fn a_stale_entry_warns_and_later_fails() {
        let allow = [entry("model/vault.rs", "exchange")];
        let warns = evaluate(&[], &allow, false);
        assert_eq!(warns.stale, allow);
        assert_eq!(warns.failures(), 0);
        assert!(warns.lines()[0].starts_with("warning:"));
        let fails = evaluate(&[], &allow, true);
        assert_eq!(fails.failures(), 1);
        assert!(fails.lines()[0].starts_with("error:"));
    }

    #[test]
    fn the_allowlist_file_parses() {
        let text = fs::read_to_string(repo_root().join(ALLOW_FILE)).unwrap();
        let entries = parse_allowlist(&text).unwrap();
        assert!(entries.iter().all(|item| !item.until.is_empty()));
    }
}
