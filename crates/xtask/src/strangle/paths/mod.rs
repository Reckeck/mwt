//! `strangle paths --dir <path> [--dry-run]`: rewrites the `service::` paths of the shims to the
//! paths of their owners (spec D4): `crate::service::X`, `trade_core::service::X` and `super::X`
//! when it names a shim type, in use trees and in expressions. The owner comes from the `pub use`
//! map of the `service/` shims (`shim`), including `pub(crate) use`, grouped lists and aliases. A name
//! with two owners, one defined inside `service/` (no owner yet) or behind a form that cannot be read
//! safely is skipped and listed, never guessed. rustfmt runs on the changed files. `--dry-run` prints
//! the plan and writes nothing.

mod rewrite;
mod shim;
#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use clap::Args;
use syn::visit::Visit;

use super::fileset::FileSet;
use super::map::Map;
use rewrite::{FileContext, Rewriter};
use shim::ShimMap;

use super::source::apply;

#[derive(Args)]
pub struct PathsArgs {
    /// Directory (or one file) to rewrite, relative to the workspace root.
    #[arg(long)]
    pub dir: String,
    /// Print the rewrites and the skipped names; write no file.
    #[arg(long)]
    pub dry_run: bool,
}

pub fn run(root: &Path, args: &PathsArgs) -> Result<()> {
    let map = Map::load(root)?;
    let mut files = FileSet::new(root);
    let report = rewrite_dir(root, &mut files, &map, args)?;
    for line in &report.rewrites {
        if args.dry_run {
            println!("paths: would rewrite {line}");
        }
    }
    for line in &report.skipped {
        println!("paths: skipped {line}");
    }
    if args.dry_run {
        println!(
            "paths: dry run, {} path(s) in {} file(s) would be rewritten, {} skipped",
            report.rewrites.len(),
            report.files,
            report.skipped.len()
        );
        return Ok(());
    }
    println!(
        "paths: {} path(s) rewritten in {} file(s), {} skipped",
        report.rewrites.len(),
        report.files,
        report.skipped.len()
    );
    let changed: Vec<String> = files.changed().iter().map(ToString::to_string).collect();
    files.write()?;
    format(root, &changed)
}

#[derive(Default)]
struct Report {
    /// Files with at least one rewrite.
    files: usize,
    /// `file:line: before -> after`.
    rewrites: Vec<String>,
    /// `file:line: path: reason`.
    skipped: Vec<String>,
}

fn rewrite_dir(root: &Path, files: &mut FileSet, map: &Map, args: &PathsArgs) -> Result<Report> {
    let shims = ShimMap::load(&root.join(&map.service_dir))?;
    let core_src = super::home::core_dir(map);
    let service_dir = &map.service_dir;
    let mut report = Report::default();
    for path in rust_files(root, &args.dir)? {
        let source = files.read(&path)?.to_string();
        let context = FileContext {
            in_core: path.starts_with(&format!("{core_src}/")),
            service_depth: service_depth(&path, service_dir),
        };
        let file = syn::parse_file(&source).with_context(|| format!("parse {path}"))?;
        let mut rewriter = Rewriter::new(&source, &shims, &context);
        rewriter.visit_file(&file);
        let found = rewriter.report;
        report.files += usize::from(!found.rewrites.is_empty());
        report
            .rewrites
            .extend(found.rewrites.iter().map(|done| format!("{path}:{done}")));
        report
            .skipped
            .extend(found.skipped.iter().map(|skip| format!("{path}:{skip}")));
        files.set(&path, apply(&source, found.splices))?;
    }
    Ok(report)
}

/// Module depth of a file below the service dir (`foo.rs` 1, `a/b.rs` 2, `mod.rs` 0).
fn service_depth(path: &str, service_dir: &str) -> Option<usize> {
    let inner = path.strip_prefix(service_dir)?.strip_prefix('/')?;
    let module = inner.strip_suffix(".rs")?;
    let parts: Vec<&str> = module.split('/').collect();
    Some(parts.len() - usize::from(parts.last() == Some(&"mod")))
}

/// Every `.rs` file under `dir` (or `dir` itself), as paths from the root, sorted.
fn rust_files(root: &Path, dir: &str) -> Result<Vec<String>> {
    let mut found = Vec::new();
    collect(root, &root.join(dir), &mut found)?;
    if found.is_empty() {
        bail!("{dir}: no .rs file");
    }
    found.sort();
    Ok(found)
}

fn collect(root: &Path, path: &Path, found: &mut Vec<String>) -> Result<()> {
    if path.is_dir() {
        let mut children: Vec<PathBuf> = std::fs::read_dir(path)
            .with_context(|| format!("read {}", path.display()))?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<std::io::Result<_>>()?;
        children.sort();
        for child in children {
            let name = child.file_name().unwrap_or_default().to_string_lossy();
            if name != "target" && !name.starts_with('.') {
                collect(root, &child, found)?;
            }
        }
    } else if path.extension().is_some_and(|extension| extension == "rs") {
        let relative = path.strip_prefix(root).unwrap_or(path);
        found.push(relative.to_string_lossy().replace('\\', "/"));
    }
    Ok(())
}

fn format(root: &Path, paths: &[String]) -> Result<()> {
    if paths.is_empty() {
        return Ok(());
    }
    let status = Command::new("rustfmt")
        .args(["--edition", "2024"])
        .args(paths)
        .current_dir(root)
        .status()
        .context("run rustfmt")?;
    if !status.success() {
        bail!("rustfmt failed on {}", paths.join(", "));
    }
    Ok(())
}
