//! The three counts of `remaining`, over the text of the files.

use std::path::Path;

use anyhow::{Context, Result};

use super::{PerFile, Remaining};
use crate::strangle::map::Map;
use crate::strangle::scan::VAULT_TYPE;

/// The roots after which `service::` names the shims.
const SERVICE_ROOTS: [&str; 3] = ["crate::", "trade_core::", "super::"];
const SERVICE_SEGMENT: &str = "service::";
const FORWARD_OPEN: &str = "forward! {";

/// Every metric of every `.rs` file under the map's `remaining_dirs`, less the excluded ones.
pub fn measure(root: &Path, map: &Map) -> Result<Remaining> {
    let mut files = Vec::new();
    for dir in &map.remaining_dirs {
        rust_files(&root.join(dir), &mut files)?;
    }
    let mut measured = Remaining::default();
    let service_prefix = format!("{}/", map.service_dir);
    for path in files {
        let relative = relative_path(root, &path);
        if map
            .remaining_exclude
            .iter()
            .any(|skip| relative.starts_with(skip))
        {
            continue;
        }
        let text = std::fs::read_to_string(&path).with_context(|| format!("read {relative}"))?;
        record(
            &mut measured.vault,
            &relative,
            count_word(&text, VAULT_TYPE),
        );
        record(&mut measured.forward, &relative, forward_entries(&text));
        if !relative.starts_with(&service_prefix) {
            record(&mut measured.service_paths, &relative, service_paths(&text));
        }
    }
    Ok(measured)
}

fn record(metric: &mut PerFile, path: &str, count: usize) {
    if count > 0 {
        metric.insert(path.to_string(), count);
    }
}

fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Build output and dependencies are not source.
fn is_skipped_dir(name: &str) -> bool {
    name == "target" || name == "node_modules" || name.starts_with('.')
}

fn rust_files(dir: &Path, found: &mut Vec<std::path::PathBuf>) -> Result<()> {
    let entries = std::fs::read_dir(dir).with_context(|| format!("read {}", dir.display()))?;
    for entry in entries {
        let path = entry?.path();
        if path.is_dir() {
            if !is_skipped_dir(&path.file_name().unwrap_or_default().to_string_lossy()) {
                rust_files(&path, found)?;
            }
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
    Ok(())
}

fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

/// Occurrences of `needle` not glued to an identifier character on its left; with `whole_word`
/// also not on its right.
fn count_matches(text: &str, needle: &str, whole_word: bool) -> usize {
    text.match_indices(needle)
        .filter(|(at, _)| !text[..*at].ends_with(is_word_char))
        .filter(|(at, _)| !whole_word || !text[at + needle.len()..].starts_with(is_word_char))
        .count()
}

pub fn count_word(text: &str, word: &str) -> usize {
    count_matches(text, word, true)
}

/// The shim paths: `crate::service::`, `trade_core::service::`, `super::service::`.
pub fn service_paths(text: &str) -> usize {
    SERVICE_ROOTS
        .iter()
        .map(|root| count_matches(text, &format!("{root}{SERVICE_SEGMENT}"), false))
        .sum()
}

/// The `=>` lines inside the `forward! { .. }` blocks: a block ends at the first line that is a
/// lone `}`.
pub fn forward_entries(text: &str) -> usize {
    let mut inside = false;
    let mut entries = 0;
    for line in text.lines().map(str::trim) {
        if inside {
            if line == "}" {
                inside = false;
            } else if line.starts_with("=>") {
                entries += 1;
            }
        } else if line.starts_with(FORWARD_OPEN) {
            inside = true;
        }
    }
    entries
}
