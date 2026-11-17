//! `xtask arch`: the module graph of `crates/core/src`, read from the non-test `crate::` paths.

mod scan;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// One `crate::` path: the file and line that names it, and the modules on both ends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    pub from_file: String,
    pub line: usize,
    pub from_module: String,
    pub to_module: String,
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
