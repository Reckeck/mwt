//! `baseline.json`: per area, every public method of `impl UnlockedVault` with its signature text
//! and the names of its tests, written once before the areas move (spec D6 step 6). `check` holds
//! the moved area against it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use syn::{ImplItemFn, Item, Visibility};

use super::Workspace;
use super::home::area_files;
use super::map::Area;
use super::scan::is_test_attr;
use super::signature::Parts;
use super::source::{self, LineIndex};

/// Where the baseline lives, relative to the workspace root.
pub const BASELINE_FILE: &str = "crates/xtask/strangle/baseline.json";

#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Baseline {
    pub areas: BTreeMap<String, AreaBaseline>,
}

#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AreaBaseline {
    /// Public methods of `impl UnlockedVault`, by name.
    pub methods: Vec<BaselineMethod>,
    /// Names of the test functions, sorted, once each.
    pub tests: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BaselineMethod {
    pub name: String,
    pub signature: String,
}

impl Baseline {
    pub fn load(root: &Path) -> Result<Self> {
        let path = root.join(BASELINE_FILE);
        let text = std::fs::read_to_string(&path).with_context(|| {
            format!(
                "read {} (write it with `strangle check --write-baseline`)",
                path.display()
            )
        })?;
        serde_json::from_str(&text).with_context(|| format!("parse {}", path.display()))
    }

    pub fn write(&self, root: &Path) -> Result<()> {
        let path = root.join(BASELINE_FILE);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
        }
        let text = serde_json::to_string_pretty(self)? + "\n";
        std::fs::write(&path, text).with_context(|| format!("write {}", path.display()))
    }

    pub fn area(&self, name: &str) -> Result<&AreaBaseline> {
        self.areas
            .get(name)
            .with_context(|| format!("no area `{name}` in {BASELINE_FILE}"))
    }
}

/// The baseline of every area of the map, as the tree is now.
pub fn capture(root: &Path, workspace: &Workspace) -> Result<Baseline> {
    let mut areas = BTreeMap::new();
    for area in &workspace.map.areas {
        let mut methods = public_methods(root, workspace, area)?;
        methods.sort_by(|left, right| left.name.cmp(&right.name));
        let tests = test_names(workspace, area).into_iter().collect();
        areas.insert(area.name.clone(), AreaBaseline { methods, tests });
    }
    Ok(Baseline { areas })
}

/// The names of the test functions in the files of the area.
pub fn test_names(workspace: &Workspace, area: &Area) -> BTreeSet<String> {
    area_files(&workspace.map, area)
        .iter()
        .filter_map(|path| workspace.corpus.get(path))
        .flat_map(|scanned| scanned.facts.tests.iter())
        .map(|test| test.name.clone())
        .collect()
}

/// `pub` methods of the production `impl UnlockedVault` blocks in the files of the area.
fn public_methods(root: &Path, workspace: &Workspace, area: &Area) -> Result<Vec<BaselineMethod>> {
    let mut found = Vec::new();
    for path in area_files(&workspace.map, area) {
        if workspace.corpus.get(&path).is_none() {
            continue;
        }
        let source =
            std::fs::read_to_string(root.join(&path)).with_context(|| format!("read {path}"))?;
        let file = syn::parse_file(&source).with_context(|| format!("parse {path}"))?;
        let index = LineIndex::new(&source);
        for method in production_methods(&file)
            .into_iter()
            .filter(|method| matches!(method.vis, Visibility::Public(_)))
        {
            let parts = Parts::new(&source, &index, &method.sig);
            found.push(BaselineMethod {
                name: parts.name.clone(),
                signature: parts.render("pub ", &parts.params),
            });
        }
    }
    Ok(found)
}

/// The methods of the top-level `impl UnlockedVault` blocks that are not test code.
pub fn production_methods(file: &syn::File) -> Vec<&ImplItemFn> {
    file.items
        .iter()
        .filter_map(|item| match item {
            Item::Impl(node) if !node.attrs.iter().any(is_test_attr) => Some(node),
            _ => None,
        })
        .flat_map(source::vault_methods_of)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strangle::area_fixture::original_tree;

    #[test]
    fn the_baseline_lists_public_methods_with_signatures_and_the_test_names() {
        let tree = original_tree();
        let workspace = Workspace::load(tree.root()).unwrap();
        let baseline = capture(tree.root(), &workspace).unwrap();
        let area = baseline.area("loyalty").unwrap();
        let listed: Vec<(&str, &str)> = area
            .methods
            .iter()
            .map(|method| (method.name.as_str(), method.signature.as_str()))
            .collect();
        assert_eq!(
            listed,
            [
                ("clash", "pub fn clash(&self, id: &str) -> bool"),
                (
                    "set_info",
                    "pub fn set_info(&mut self, id: &str, info: Info) -> Result<Store>"
                ),
            ]
        );
        assert_eq!(area.tests, ["clash_works", "sets_info"]);
        assert!(baseline.area("nope").is_err());
    }

    #[test]
    fn it_survives_a_round_trip_through_the_file() {
        let tree = original_tree();
        let workspace = Workspace::load(tree.root()).unwrap();
        let baseline = capture(tree.root(), &workspace).unwrap();
        baseline.write(tree.root()).unwrap();
        assert_eq!(Baseline::load(tree.root()).unwrap(), baseline);
    }
}
