//! `strangle widen`: every non-`pub` method of every `impl UnlockedVault` under the service dir
//! becomes `pub(crate)`, so a method moved out of `service/` keeps access to its private siblings
//! (spec D1, OD5). One token changes per method; nothing else. Fields of `UnlockedVault` (`password`,
//! `key`, ...) are never touched here: the few a moved file reads are widened by hand in a named task.

use std::path::Path;

use anyhow::{Context, Result};

use super::Workspace;
use super::fileset::FileSet;
use super::source::{self, LineIndex};

pub fn run(root: &Path, workspace: &Workspace) -> Result<()> {
    let mut files = FileSet::new(root);
    let mut widened = 0;
    for (relative, scanned) in workspace.service_files() {
        if scanned.facts.methods.iter().all(|method| method.public) {
            continue;
        }
        let path = format!("{}/{relative}", workspace.map.service_dir);
        let (text, count) = widen_source(files.read(&path)?).with_context(|| path.clone())?;
        if count > 0 {
            files.set(&path, text)?;
            widened += count;
        }
    }
    let changed = files.changed().len();
    files.write()?;
    println!("widen: {widened} method(s) in {changed} file(s) are pub(crate) now");
    Ok(())
}

/// The source with its vault methods widened, and how many were.
pub fn widen_source(text: &str) -> Result<(String, usize)> {
    let file = syn::parse_file(text).context("parse")?;
    let index = LineIndex::new(text);
    let splices: Vec<_> = source::vault_methods(&file)
        .into_iter()
        .filter_map(|method| {
            source::widen(&index, &method.vis, source::signature_start(&method.sig))
        })
        .collect();
    let count = splices.len();
    Ok((source::apply(text, splices), count))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strangle::fixture::TempTree;
    use crate::strangle::map::{MAP_FILE, Map};

    const MAP: &str = r#"
service_dir = "crates/svc"
command_dir = "crates/app"
scan_dirs = ["crates/svc"]

[[area]]
name = "all"
task = "T"
group = "g"
context = "c"
files = ["mod.rs"]
"#;

    const MOD_RS: &str = "// header
impl UnlockedVault {
    pub fn open(&self) { let _ = \"fn hidden(&self) {}\"; }
    fn private(&self) {}
    pub(crate) fn already(&self) {}
}
";
    const SIDE_RS: &str = "impl UnlockedVault {
    /// Doc.
    pub(super) async fn later(&mut self) {}
}
impl Display for UnlockedVault {
    fn fmt(&self) {}
}
fn free() {}
";

    #[test]
    fn it_changes_the_visibility_token_of_non_pub_methods_and_nothing_else() {
        let tree = TempTree::new(&[
            (MAP_FILE, MAP),
            ("crates/svc/mod.rs", MOD_RS),
            ("crates/svc/side.rs", SIDE_RS),
            (
                "crates/svc/nested/deep.rs",
                "impl UnlockedVault { fn deep(&self) {} }\n",
            ),
            ("crates/svc/plain.rs", "fn free() {}\n"),
        ]);
        let workspace = Workspace::build(tree.root(), Map::load(tree.root()).unwrap()).unwrap();
        run(tree.root(), &workspace).unwrap();
        let read = |name: &str| {
            std::fs::read_to_string(tree.root().join("crates/svc").join(name)).unwrap()
        };
        assert_eq!(
            read("mod.rs"),
            MOD_RS.replace("    fn private", "    pub(crate) fn private")
        );
        assert_eq!(
            read("side.rs"),
            SIDE_RS.replace("pub(super) async", "pub(crate) async")
        );
        assert_eq!(
            read("nested/deep.rs"),
            "impl UnlockedVault { pub(crate) fn deep(&self) {} }\n"
        );
        assert_eq!(read("plain.rs"), "fn free() {}\n");
    }

    #[test]
    fn a_second_run_changes_nothing() {
        let (once, first) = widen_source(MOD_RS).unwrap();
        assert_eq!(first, 1);
        assert_eq!(widen_source(&once).unwrap(), (once.clone(), 0));
    }

    #[test]
    fn run_leaves_the_vault_definition_fields_alone() {
        let definition = "pub struct UnlockedVault {\n    password: String,\n    key: u8,\n    doc: u8,\n}\nimpl UnlockedVault {\n    fn read(&self) -> u8 { self.doc }\n}\n";
        let tree = TempTree::new(&[(MAP_FILE, MAP), ("crates/svc/mod.rs", definition)]);
        let workspace = Workspace::build(tree.root(), Map::load(tree.root()).unwrap()).unwrap();
        run(tree.root(), &workspace).unwrap();
        let text = std::fs::read_to_string(tree.root().join("crates/svc/mod.rs")).unwrap();
        assert_eq!(
            text,
            definition.replace("    fn read", "    pub(crate) fn read")
        );
    }
}
