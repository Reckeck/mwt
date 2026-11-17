//! `strangle forward <area>`: regenerates the `forward!` block of each shim from the signatures of
//! the area's new functions (spec D3, D6 step 6). A function is forwarded when its first parameter
//! is `vault: &UnlockedVault`, `vault: &mut UnlockedVault`, `books: &Books<'_>` or `doc: &VaultDoc`;
//! the block sits between the two marker lines and is rewritten whole, so a second run changes
//! nothing.

use std::path::Path;

use anyhow::{Context, Result, bail};
use clap::Args;
use syn::{FnArg, Item, ItemFn, Pat, Type, Visibility};

use super::fileset::FileSet;
use super::home::{Home, homes};
use super::map::Map;
use super::scan::is_test_attr;
use super::signature::{Parts, doc_lines};
use super::source::LineIndex;

pub const FORWARD_BEGIN: &str = "// forward:begin";
pub const FORWARD_END: &str = "// forward:end";

#[derive(Args)]
pub struct ForwardArgs {
    /// One area of `strangle.toml`.
    pub area: String,
}

pub fn run(root: &Path, args: &ForwardArgs) -> Result<()> {
    let map = Map::load(root)?;
    let mut files = FileSet::new(root);
    for line in forward(&mut files, &map, &args.area)? {
        println!("forward: {line}");
    }
    files.write()
}

fn forward(files: &mut FileSet, map: &Map, area: &str) -> Result<Vec<String>> {
    let area = &map.areas[map.area_index(area)?];
    let mut report = Vec::new();
    for home in homes(map, area) {
        let shim = home.shim();
        let text = files.read(&shim)?.to_string();
        if text.is_empty() {
            continue;
        }
        let entries = entries_of(files, &home)?;
        let block = if entries.is_empty() {
            String::new()
        } else {
            format!("forward! {{\n{}}}\n", entries.concat())
        };
        let updated = replace_region(&text, &block).with_context(|| {
            format!("{shim}: no `{FORWARD_BEGIN}` / `{FORWARD_END}` (relocate first)")
        })?;
        files.set(&shim, updated)?;
        report.push(format!("{shim}: {} delegate(s)", entries.len()));
    }
    if report.is_empty() {
        bail!("{}: no shim file to forward from", area.name);
    }
    Ok(report)
}

/// One text per forwarded function of the stem: the pure file first, then the scenarios.
fn entries_of(files: &mut FileSet, home: &Home) -> Result<Vec<String>> {
    let mut entries = Vec::new();
    for (path, module) in [
        (home.pure_file(), home.pure_path()),
        (home.scenario_file(), home.scenario_path()),
    ] {
        let source = files.read(&path)?.to_string();
        let file = syn::parse_file(&source).with_context(|| format!("parse {path}"))?;
        let index = LineIndex::new(&source);
        for item in &file.items {
            if let Item::Fn(node) = item
                && let Some(entry) = entry_of(&source, &index, node, &module)
            {
                entries.push(entry);
            }
        }
    }
    Ok(entries)
}

/// How the first parameter of a function maps to the delegate.
struct Receiver {
    text: &'static str,
    /// The marker after the path (`@books`, `@doc`); none when the vault goes first.
    marker: Option<&'static str>,
}

fn receiver_of(first: &FnArg) -> Option<Receiver> {
    let FnArg::Typed(typed) = first else {
        return None;
    };
    let (Pat::Ident(name), Type::Reference(reference)) = (&*typed.pat, &*typed.ty) else {
        return None;
    };
    let Type::Path(target) = &*reference.elem else {
        return None;
    };
    let target = target.path.segments.last()?.ident.to_string();
    let (text, marker) = match (
        name.ident.to_string().as_str(),
        target.as_str(),
        reference.mutability.is_some(),
    ) {
        ("vault", "UnlockedVault", true) => ("&mut self", None),
        ("vault", "UnlockedVault", false) => ("&self", None),
        ("books", "Books", false) => ("&self", Some("@books")),
        ("doc", "VaultDoc", false) => ("&self", Some("@doc")),
        _ => return None,
    };
    Some(Receiver { text, marker })
}

fn entry_of(source: &str, index: &LineIndex, node: &ItemFn, module: &str) -> Option<String> {
    if !matches!(node.vis, Visibility::Public(_)) || node.attrs.iter().any(is_test_attr) {
        return None;
    }
    let receiver = receiver_of(node.sig.inputs.first()?)?;
    let parts = Parts::new(source, index, &node.sig);
    let params: Vec<String> = std::iter::once(receiver.text.to_string())
        .chain(parts.params.iter().skip(1).cloned())
        .collect();
    let mut entry: String = doc_lines(&node.attrs)
        .iter()
        .map(|doc| format!("    ///{doc}\n"))
        .collect();
    entry += &format!("    {}\n", parts.render("pub ", &params));
    entry += &format!("        => {module}::{}", parts.name);
    if let Some(marker) = receiver.marker {
        entry += &format!(" {marker}");
    }
    entry += ";\n";
    Some(entry)
}

/// The text with everything between the marker lines replaced by `block`.
fn replace_region(text: &str, block: &str) -> Option<String> {
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let (begin, end) = region_lines(&lines)?;
    Some(format!(
        "{}{block}{}",
        lines[..=begin].concat(),
        lines[end..].concat()
    ))
}

/// Indices of the begin and end marker lines.
fn region_lines(lines: &[&str]) -> Option<(usize, usize)> {
    let marker = |wanted: &str| lines.iter().position(|line| line.trim() == wanted);
    let begin = marker(FORWARD_BEGIN)?;
    let end = marker(FORWARD_END).filter(|&end| end > begin)?;
    Some((begin, end))
}

/// The names of the methods the forward block of a shim delegates (none for a shim without markers).
pub fn forwarded_names(shim: &str) -> Option<Vec<String>> {
    let lines: Vec<&str> = shim.split_inclusive('\n').collect();
    let (begin, end) = region_lines(&lines)?;
    Some(
        lines[begin + 1..end]
            .iter()
            .filter_map(|line| {
                let rest = line.trim().strip_prefix("pub ")?;
                let rest = rest.strip_prefix("async ").unwrap_or(rest);
                let name = rest.strip_prefix("fn ")?;
                let end = name.find(|ch: char| !(ch.is_alphanumeric() || ch == '_'))?;
                Some(name[..end].to_string())
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strangle::area_fixture::{PURE, SCENARIO, SERVICE, moved_tree};
    use crate::strangle::fixture::TempTree;

    const SHIM_WITH_EMPTY_REGION: &str = "//! Shim.\n\npub use crate::household::loyalty::Card;\n\n// forward:begin\n// forward:end\n";

    fn forwarded(tree: &TempTree) -> Result<String> {
        let map = Map::load(tree.root())?;
        let mut files = FileSet::new(tree.root());
        forward(&mut files, &map, "loyalty")?;
        files.write()?;
        Ok(std::fs::read_to_string(tree.root().join(SERVICE))?)
    }

    fn tree_with_empty_region() -> TempTree {
        moved_tree(|path, text| {
            if path == SERVICE {
                SHIM_WITH_EMPTY_REGION.to_string()
            } else {
                text
            }
        })
    }

    #[test]
    fn the_block_is_written_from_the_signatures_between_the_markers() {
        let shim = forwarded(&tree_with_empty_region()).unwrap();
        assert_eq!(
            shim,
            "//! Shim.

pub use crate::household::loyalty::Card;

// forward:begin
forward! {
    /// Is there a clash.
    pub fn clash(&self, id: &str) -> bool
        => crate::household::loyalty::clash @books;
    /// Sets the info.
    pub fn set_info(&mut self, id: &str, info: Info) -> Result<Store>
        => crate::usecases::household::loyalty::set_info;
}
// forward:end
"
        );
    }

    #[test]
    fn running_it_twice_changes_nothing() {
        let tree = tree_with_empty_region();
        let first = forwarded(&tree).unwrap();
        assert_eq!(forwarded(&tree).unwrap(), first);
    }

    #[test]
    fn the_four_first_parameter_shapes_map_to_a_delegate_and_others_are_skipped() {
        let tree = tree_with_empty_region();
        let source = "pub fn shared(vault: &UnlockedVault, a: u8) -> u8 { a }
pub fn exclusive(vault: &mut UnlockedVault) {}
pub fn reader(books: &Books<'_>, a: u8) -> u8 { a }
pub fn doc_reader(doc: &VaultDoc) -> usize { 0 }
pub fn wrong_name(other: &VaultDoc) {}
pub fn wrong_mut(doc: &mut VaultDoc) {}
fn private(doc: &VaultDoc) {}
pub fn no_params() {}
#[test]
pub fn a_test(doc: &VaultDoc) {}
";
        std::fs::write(tree.root().join(PURE), source).unwrap();
        std::fs::write(tree.root().join(SCENARIO), "").unwrap();
        let shim = forwarded(&tree).unwrap();
        for wanted in [
            "pub fn shared(&self, a: u8) -> u8\n        => crate::household::loyalty::shared;",
            "pub fn exclusive(&mut self)\n        => crate::household::loyalty::exclusive;",
            "pub fn reader(&self, a: u8) -> u8\n        => crate::household::loyalty::reader @books;",
            "pub fn doc_reader(&self) -> usize\n        => crate::household::loyalty::doc_reader @doc;",
        ] {
            assert!(shim.contains(wanted), "{wanted}\n{shim}");
        }
        for skipped in ["wrong_name", "wrong_mut", "private", "no_params", "a_test"] {
            assert!(!shim.contains(skipped), "{skipped}");
        }
    }

    #[test]
    fn a_shim_without_markers_is_refused() {
        let tree = moved_tree(|path, text| {
            if path == SERVICE {
                "//! no markers\n".to_string()
            } else {
                text
            }
        });
        let error = forwarded(&tree).unwrap_err();
        assert!(error.to_string().contains("relocate first"), "{error:#}");
    }

    #[test]
    fn forwarded_names_reads_the_entries_back() {
        let shim =
            std::fs::read_to_string(moved_tree(|_, text| text).root().join(SERVICE)).unwrap();
        assert_eq!(forwarded_names(&shim).unwrap(), ["set_info", "clash"]);
        assert_eq!(forwarded_names("pub fn x() {}"), None);
    }
}
