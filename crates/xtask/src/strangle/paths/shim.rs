//! The `pub use` map of the `service/` shims: for a name `service::X`, the path of its owner, or the
//! reason it cannot be named safely. Chains are followed (`mod.rs` re-exports from a shim, the shim
//! from the owner). A form that cannot be read (a glob, a `cfg` gate, a name that two entries give
//! different owners) is never guessed: it resolves to a [`Skip`].

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result};
use syn::{Item, UseTree, Visibility};

/// The owner path of a name, as segments (`["crate", "money", "Amount"]`), or why there is none.
pub type Resolution = Result<Vec<String>, Skip>;

/// Why a name is left alone.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Skip {
    /// The item is defined inside `service/`: it has no owner yet.
    Defined,
    /// Two `pub use` entries give the name different owners.
    TwoOwners(String),
    /// The only entry is behind a `#[cfg(..)]`.
    Gated,
    /// The shim has a glob re-export and the name is not listed: the owner is unknown.
    Glob,
    /// No `pub use` and no definition of the name was found.
    Unknown,
    /// A path of the shim that this tool does not follow.
    Unreadable(String),
}

impl Skip {
    pub fn reason(&self) -> String {
        match self {
            Self::Defined => "defined inside service/, no owner yet".into(),
            Self::TwoOwners(owners) => format!("re-exported from two owners ({owners})"),
            Self::Gated => "re-exported only under a cfg gate".into(),
            Self::Glob => "the shim re-exports by glob, the owner is not readable".into(),
            Self::Unknown => "not in the pub use map of service/".into(),
            Self::Unreadable(path) => format!("unreadable pub use path `{path}`"),
        }
    }
}

/// One name a `pub use` brings in.
#[derive(Debug)]
struct Entry {
    exported: String,
    path: Vec<String>,
    glob: bool,
    gated: bool,
}

#[derive(Default)]
struct Module {
    entries: Vec<Entry>,
    defined: BTreeSet<String>,
}

/// The shims of one `service/` dir by module path (`""` is `mod.rs`, `"a::b"` is `a/b.rs`).
pub struct ShimMap {
    modules: BTreeMap<Vec<String>, Module>,
}

/// Deep enough for any real chain, and stops a cycle.
const MAX_DEPTH: usize = 12;

impl ShimMap {
    pub fn load(service_dir: &Path) -> Result<Self> {
        let mut modules = BTreeMap::new();
        collect(service_dir, &mut Vec::new(), &mut modules)?;
        Ok(Self { modules })
    }

    /// The owner of `service::name`.
    pub fn resolve(&self, name: &str) -> Resolution {
        self.lookup(&[], name, 0)
    }

    fn lookup(&self, module: &[String], name: &str, depth: usize) -> Resolution {
        let Some(found) = self.modules.get(module) else {
            return Err(Skip::Unknown);
        };
        let entries: Vec<&Entry> = found
            .entries
            .iter()
            .filter(|entry| !entry.glob && entry.exported == name)
            .collect();
        if entries.is_empty() {
            return Err(if found.defined.contains(name) {
                Skip::Defined
            } else if found.entries.iter().any(|entry| entry.glob) {
                Skip::Glob
            } else {
                Skip::Unknown
            });
        }
        if depth > MAX_DEPTH {
            return Err(Skip::Unreadable(entries[0].path.join("::")));
        }
        let owners: Vec<Resolution> = entries
            .iter()
            .map(|entry| self.follow(module, &entry.path, depth))
            .collect();
        let distinct: BTreeSet<&Resolution> = owners.iter().collect();
        match distinct.len() {
            1 if entries[0].gated => Err(Skip::Gated),
            1 => owners.into_iter().next().unwrap_or(Err(Skip::Unknown)),
            _ => Err(Skip::TwoOwners(
                distinct
                    .iter()
                    .map(|owner| match owner {
                        Ok(path) => path.join("::"),
                        Err(skip) => skip.reason(),
                    })
                    .collect::<Vec<_>>()
                    .join(" | "),
            )),
        }
    }

    /// Where a `pub use` path of `module` leads: into another shim, or out to an owner.
    fn follow(&self, module: &[String], path: &[String], depth: usize) -> Resolution {
        let Some(first) = path.first() else {
            return Err(Skip::Unknown);
        };
        let mut absolute: Vec<String> = match first.as_str() {
            "crate" => path.to_vec(),
            "self" | "super" => {
                let mut base: Vec<String> = ["crate", "service"]
                    .iter()
                    .map(ToString::to_string)
                    .chain(module.iter().cloned())
                    .collect();
                let mut rest = path;
                while let Some((head, tail)) = rest.split_first() {
                    match head.as_str() {
                        "self" => {}
                        "super" if base.len() > 1 => {
                            base.pop();
                        }
                        "super" => return Err(Skip::Unreadable(path.join("::"))),
                        _ => break,
                    }
                    rest = tail;
                }
                base.extend(rest.iter().cloned());
                base
            }
            _ if self.is_module(module, first) => {
                let mut base: Vec<String> = ["crate", "service"]
                    .iter()
                    .map(ToString::to_string)
                    .chain(module.iter().cloned())
                    .collect();
                base.extend(path.iter().cloned());
                base
            }
            _ if first.starts_with("mwt_") => return Ok(path.to_vec()),
            _ => return Err(Skip::Unreadable(path.join("::"))),
        };
        if absolute.get(1).map(String::as_str) != Some("service") || absolute.len() < 3 {
            return Ok(absolute);
        }
        let inside = absolute.split_off(2);
        let split = (0..inside.len())
            .rev()
            .find(|&at| self.modules.contains_key(&inside[..at]))
            .unwrap_or(0);
        let rest = &inside[split + 1..];
        let mut owner = self.lookup(&inside[..split], &inside[split], depth + 1)?;
        owner.extend(rest.iter().cloned());
        Ok(owner)
    }

    fn is_module(&self, parent: &[String], name: &str) -> bool {
        let mut child = parent.to_vec();
        child.push(name.to_string());
        self.modules.contains_key(&child)
    }

    #[cfg(test)]
    pub fn from_sources(sources: &[(&[&str], &str)]) -> Result<Self> {
        let mut modules = BTreeMap::new();
        for (module, text) in sources {
            let key = module.iter().map(ToString::to_string).collect();
            modules.insert(key, read_module(text)?);
        }
        Ok(Self { modules })
    }
}

fn collect(
    dir: &Path,
    module: &mut Vec<String>,
    modules: &mut BTreeMap<Vec<String>, Module>,
) -> Result<()> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .with_context(|| format!("read {}", dir.display()))?
        .collect::<std::io::Result<_>>()?;
    entries.sort_by_key(std::fs::DirEntry::path);
    for entry in entries {
        let path = entry.path();
        let stem = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        if path.is_dir() {
            module.push(stem);
            collect(&path, module, modules)?;
            module.pop();
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            let key = if stem == "mod" {
                module.clone()
            } else {
                module.iter().cloned().chain([stem]).collect()
            };
            let text = std::fs::read_to_string(&path)
                .with_context(|| format!("read {}", path.display()))?;
            let parsed = read_module(&text).with_context(|| format!("parse {}", path.display()))?;
            modules.insert(key, parsed);
        }
    }
    Ok(())
}

fn read_module(text: &str) -> Result<Module> {
    let file = syn::parse_file(text)?;
    let mut module = Module::default();
    for item in &file.items {
        match item {
            Item::Use(node) if !matches!(node.vis, Visibility::Inherited) => {
                let gated = node.attrs.iter().any(|attr| attr.path().is_ident("cfg"));
                for leaf in leaves(&node.tree) {
                    module.entries.push(Entry {
                        exported: leaf.exported(),
                        path: leaf.path,
                        glob: leaf.glob,
                        gated,
                    });
                }
            }
            Item::Use(_) => {}
            other => module.defined.extend(defined_name(other)),
        }
    }
    Ok(module)
}

fn defined_name(item: &Item) -> Option<String> {
    let ident = match item {
        Item::Struct(node) => &node.ident,
        Item::Enum(node) => &node.ident,
        Item::Union(node) => &node.ident,
        Item::Fn(node) => &node.sig.ident,
        Item::Const(node) => &node.ident,
        Item::Static(node) => &node.ident,
        Item::Type(node) => &node.ident,
        Item::Trait(node) => &node.ident,
        Item::Mod(node) => &node.ident,
        Item::Macro(node) => node.ident.as_ref()?,
        _ => return None,
    };
    Some(ident.to_string())
}

/// One name of a use tree, with the path it was written with.
pub struct Leaf {
    pub path: Vec<String>,
    pub alias: Option<String>,
    pub glob: bool,
}

impl Leaf {
    /// The name the leaf brings in (empty for a glob).
    fn exported(&self) -> String {
        self.alias
            .clone()
            .or_else(|| self.path.last().cloned())
            .unwrap_or_default()
    }
}

/// Every leaf of a use tree, groups flattened to full paths; `self` in a group is the group's prefix.
pub fn leaves(tree: &UseTree) -> Vec<Leaf> {
    let mut found = Vec::new();
    flatten(tree, &mut Vec::new(), &mut found);
    found
}

fn flatten(tree: &UseTree, prefix: &mut Vec<String>, found: &mut Vec<Leaf>) {
    let mut leaf = |tail: Option<String>, alias: Option<String>, glob: bool| {
        let mut path = prefix.clone();
        path.extend(tail);
        found.push(Leaf { path, alias, glob });
    };
    match tree {
        UseTree::Path(node) => {
            prefix.push(node.ident.to_string());
            flatten(&node.tree, prefix, found);
            prefix.pop();
        }
        UseTree::Name(node) => leaf(own_name(&node.ident), None, false),
        UseTree::Rename(node) => leaf(own_name(&node.ident), Some(node.rename.to_string()), false),
        UseTree::Glob(_) => leaf(None, None, true),
        UseTree::Group(node) => {
            for item in &node.items {
                flatten(item, prefix, found);
            }
        }
    }
}

/// `self` names the prefix itself.
fn own_name(ident: &syn::Ident) -> Option<String> {
    (ident != "self").then(|| ident.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owner(map: &ShimMap, name: &str) -> String {
        map.resolve(name).unwrap().join("::")
    }

    #[test]
    fn a_chain_through_a_shim_ends_at_the_owner() {
        let map = ShimMap::from_sources(&[
            (&[], "pub use loyalty::{Cashback, Balance as Held};\n"),
            (
                &["loyalty"],
                "pub use crate::money::loyalty::{Cashback, Balance};\n",
            ),
        ])
        .unwrap();
        assert_eq!(owner(&map, "Cashback"), "crate::money::loyalty::Cashback");
        assert_eq!(owner(&map, "Held"), "crate::money::loyalty::Balance");
    }

    #[test]
    fn a_name_defined_in_service_has_no_owner() {
        let map = ShimMap::from_sources(&[
            (&[], "pub use own::Draft;\n"),
            (&["own"], "pub struct Draft;\n"),
        ])
        .unwrap();
        assert_eq!(map.resolve("Draft"), Err(Skip::Defined));
    }

    #[test]
    fn globs_and_gates_are_refused() {
        let map = ShimMap::from_sources(&[
            (&[], "pub use a::{Glob, Gated};\n"),
            (
                &["a"],
                "pub use mwt_x::*;\n#[cfg(test)]\npub(crate) use mwt_y::Gated;\n",
            ),
        ])
        .unwrap();
        assert_eq!(map.resolve("Glob"), Err(Skip::Glob));
        assert_eq!(map.resolve("Gated"), Err(Skip::Gated));
    }

    #[test]
    fn two_owners_are_refused() {
        let map = ShimMap::from_sources(&[
            (&[], "pub use a::Both;\npub use b::Both;\n"),
            (&["a"], "pub use mwt_a::Both;\n"),
            (&["b"], "pub use mwt_b::Both;\n"),
        ])
        .unwrap();
        let Err(Skip::TwoOwners(owners)) = map.resolve("Both") else {
            panic!("expected two owners");
        };
        assert_eq!(owners, "mwt_a::Both | mwt_b::Both");
    }
}
