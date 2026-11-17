//! `strangle scaffold`: every group module, area stub, scenario stub and ipc_contract stub of the
//! target layout (spec D1, OD3), created once and declared once. Existing files are left alone.

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Result, bail};
use clap::Args;

use super::fileset::FileSet;
use super::map::{Area, Map};

#[derive(Args)]
pub struct ScaffoldArgs {
    /// Change nothing; fail when a stub or a declaration is missing.
    #[arg(long)]
    pub check: bool,
}

pub fn run(root: &Path, args: &ScaffoldArgs) -> Result<()> {
    let map = Map::load(root)?;
    let mut files = FileSet::new(root);
    plan(&map, &mut files)?;
    let changed = files.changed();
    if changed.is_empty() {
        println!("scaffold: every stub exists and is declared");
        return Ok(());
    }
    if args.check {
        bail!(
            "scaffold: {} file(s) missing or undeclared:\n  {}",
            changed.len(),
            changed.join("\n  ")
        );
    }
    files.write()?;
    println!(
        "scaffold: wrote {} file(s):\n  {}",
        changed.len(),
        changed.join("\n  ")
    );
    Ok(())
}

/// One area file of a group: `<group>/<name>.rs`, with its scenario home.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct Stub<'map> {
    group: &'map str,
    name: String,
    context: &'map str,
    task: &'map str,
}

fn plan(map: &Map, files: &mut FileSet) -> Result<()> {
    let core = Path::new(&map.service_dir)
        .parent()
        .map_or_else(String::new, |dir| dir.to_string_lossy().into_owned());
    let lib = format!("{core}/lib.rs");
    for stub in stubs(map) {
        let placeholder = format!("//! placeholder, filled by {}\n", stub.task);
        let group = Module::declare(files, &lib, &core, stub.group)?;
        group.add_file(files, &stub.name, &placeholder)?;
        group.add_file(files, &format!("{}_tests", stub.name), &placeholder)?;
        let usecases = Module::declare(files, &lib, &core, "usecases")?;
        let context = Module::declare(files, &usecases.file, &usecases.dir, stub.context)?;
        context.add_file(files, &stub.name, &placeholder)?;
    }
    for area in map
        .areas
        .iter()
        .filter(|area| area.stubs && area.ipc_contract)
    {
        let ipc = Module::existing(&core, "ipc_contract");
        ipc.add_file(
            files,
            &area.name,
            &format!("//! placeholder, filled by {}\n", area.task),
        )?;
    }
    Ok(())
}

/// The area files of the map, once each: a file entry gives its stem, a region-only area its name
/// (`mod_stores` is `stores`, the file `carve` makes).
fn stubs(map: &Map) -> BTreeSet<Stub<'_>> {
    let mut found = BTreeSet::new();
    for area in map.areas.iter().filter(|area| area.stubs) {
        for name in stems(area) {
            found.insert(Stub {
                group: area.homes.get(&name).unwrap_or(&area.group),
                name,
                context: &area.context,
                task: &area.task,
            });
        }
    }
    found
}

pub(super) fn stems(area: &Area) -> BTreeSet<String> {
    if area.files.is_empty() {
        let name = area.name.strip_prefix("mod_").unwrap_or(&area.name);
        return BTreeSet::from([name.to_string()]);
    }
    area.files
        .iter()
        .map(|entry| entry.trim_end_matches('/').trim_end_matches(".rs"))
        .filter(|stem| !stem.ends_with("_tests"))
        .map(str::to_string)
        .collect()
}

/// A module of the core crate: the file that holds its declarations and the dir of its children.
struct Module {
    file: String,
    dir: String,
}

impl Module {
    /// `name` declared (`pub mod`) in `parent_file`, created as a stub when it does not exist.
    fn declare(
        files: &mut FileSet,
        parent_file: &str,
        parent_dir: &str,
        name: &str,
    ) -> Result<Self> {
        files.declare(parent_file, name, &[format!("pub mod {name};")])?;
        let module = Self::existing(parent_dir, name);
        let sibling = format!("{parent_dir}/{name}.rs");
        let file = if files.exists(&sibling) {
            sibling
        } else {
            module.file.clone()
        };
        files.create_if_missing(
            &file,
            &format!("//! The `{name}` module group (spec core-strangler, D1).\n"),
        )?;
        Ok(Self { file, ..module })
    }

    fn existing(parent_dir: &str, name: &str) -> Self {
        let dir = format!("{parent_dir}/{name}");
        let file = if name == "ipc_contract" {
            format!("{parent_dir}/{name}.rs")
        } else {
            format!("{dir}/mod.rs")
        };
        Self { file, dir }
    }

    /// `<dir>/<name>.rs` created and declared in the module file (tests stubs: `#[cfg(test)]`).
    fn add_file(&self, files: &mut FileSet, name: &str, text: &str) -> Result<()> {
        files.create_if_missing(&format!("{}/{name}.rs", self.dir), text)?;
        let lines = if name.ends_with("_tests") {
            vec!["#[cfg(test)]".to_string(), format!("mod {name};")]
        } else {
            vec![format!("pub mod {name};")]
        };
        files.declare(&self.file, name, &lines)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strangle::fixture::TempTree;
    use crate::strangle::map::MAP_FILE;

    const MAP: &str = r#"
service_dir = "crates/core/service"
command_dir = "crates/app"
scan_dirs = ["crates/core"]

[[area]]
name = "alpha"
task = "T1"
group = "money"
context = "money"
files = ["alpha.rs", "alpha_tests.rs", "dir/", "moved.rs"]
homes = { moved = "planning" }
ipc_contract = true

[[area]]
name = "mod_stores"
task = "T2"
group = "money"
context = "money"
regions = [{ file = "mod.rs", from = "// -- a", to = "// -- b" }]

[[area]]
name = "later"
task = "phase-6"
group = "vault"
context = "vault"
files = ["later.rs"]
stubs = false
"#;

    fn tree() -> TempTree {
        TempTree::new(&[
            (MAP_FILE, MAP),
            (
                "crates/core/lib.rs",
                "pub mod error;\npub mod money;\n\npub use error::Error;\n",
            ),
            (
                "crates/core/ipc_contract.rs",
                "use x::Y;\n\npub mod security;\n",
            ),
            ("crates/core/money/mod.rs", "//! money\n\npub mod alpha;\n"),
            ("crates/core/money/alpha.rs", "// real code\n"),
        ])
    }

    fn read(tree: &TempTree, path: &str) -> String {
        std::fs::read_to_string(tree.root().join(path)).unwrap()
    }

    fn scaffold(tree: &TempTree, check: bool) -> Result<()> {
        run(tree.root(), &ScaffoldArgs { check })
    }

    #[test]
    fn it_creates_every_stub_of_the_layout_and_declares_each_once() {
        let tree = tree();
        scaffold(&tree, false).unwrap();
        for stub in [
            "money/dir.rs",
            "money/alpha_tests.rs",
            "money/stores.rs",
            "planning/mod.rs",
            "planning/moved.rs",
            "usecases/mod.rs",
            "usecases/money/mod.rs",
            "usecases/money/stores.rs",
            "usecases/money/moved.rs",
            "ipc_contract/alpha.rs",
        ] {
            assert!(
                tree.root().join("crates/core").join(stub).exists(),
                "{stub}"
            );
        }
        assert!(!tree.root().join("crates/core/vault").exists());
        assert_eq!(read(&tree, "crates/core/money/alpha.rs"), "// real code\n");
        assert_eq!(
            read(&tree, "crates/core/money/mod.rs"),
            "//! money\n\npub mod alpha;\n#[cfg(test)]\nmod alpha_tests;\npub mod dir;\n#[cfg(test)]\nmod dir_tests;\npub mod stores;\n#[cfg(test)]\nmod stores_tests;\n"
        );
        assert_eq!(
            read(&tree, "crates/core/lib.rs"),
            "pub mod error;\npub mod money;\npub mod usecases;\npub mod planning;\n\npub use error::Error;\n"
        );
        assert_eq!(
            read(&tree, "crates/core/ipc_contract.rs"),
            "use x::Y;\n\npub mod security;\npub mod alpha;\n"
        );
        assert_eq!(
            read(&tree, "crates/core/money/stores.rs"),
            "//! placeholder, filled by T2\n"
        );
    }

    #[test]
    fn a_second_run_changes_nothing() {
        let tree = tree();
        scaffold(&tree, false).unwrap();
        let before: Vec<String> = [
            "crates/core/lib.rs",
            "crates/core/money/mod.rs",
            "crates/core/usecases/money/mod.rs",
        ]
        .iter()
        .map(|path| read(&tree, path))
        .collect();
        scaffold(&tree, false).unwrap();
        scaffold(&tree, true).unwrap();
        let after: Vec<String> = [
            "crates/core/lib.rs",
            "crates/core/money/mod.rs",
            "crates/core/usecases/money/mod.rs",
        ]
        .iter()
        .map(|path| read(&tree, path))
        .collect();
        assert_eq!(before, after);
    }

    #[test]
    fn check_fails_while_a_stub_is_missing_and_writes_nothing() {
        let tree = tree();
        let error = scaffold(&tree, true).unwrap_err().to_string();
        assert!(error.contains("money/stores.rs"), "{error}");
        assert!(!tree.root().join("crates/core/money/stores.rs").exists());
        scaffold(&tree, false).unwrap();
        std::fs::remove_file(tree.root().join("crates/core/usecases/money/stores.rs")).unwrap();
        assert!(scaffold(&tree, true).is_err());
    }
}
