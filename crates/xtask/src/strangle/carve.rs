//! `strangle carve <items.toml>`: moves the named functions, types and `impl UnlockedVault`
//! methods of one service file into a new service file, as they are (order, docs, bodies), and
//! declares it in `service/mod.rs` (spec D1, OD4). An item the rest of the file still needs stays
//! reachable: a moved private item becomes `pub(crate)`, and the moved free items are re-exported.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::Args;
use serde::Deserialize;
use syn::{Item, Visibility};

use super::fileset::FileSet;
use super::map::Map;
use super::source::{self, LineIndex};
use super::widen::{field_reads, private_fields};

#[derive(Args)]
pub struct CarveArgs {
    /// The carve description (`from`, `to`, `items`).
    pub items: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Items {
    /// The file to cut from, under the service dir.
    pub from: String,
    /// The new file, under the service dir; it must not exist.
    pub to: String,
    /// Names of free functions, types and constants, and of vault methods.
    pub items: Vec<String>,
}

pub fn run(root: &Path, args: &CarveArgs) -> Result<()> {
    let text = std::fs::read_to_string(&args.items)
        .with_context(|| format!("read {}", args.items.display()))?;
    let items: Items = toml::from_str(&text).context("parse the carve description")?;
    let map = Map::load(root)?;
    let mut files = FileSet::new(root);
    carve(&mut files, &map.service_dir, &items)?;
    files.write()?;
    println!(
        "carve: {} item(s) moved from {} to {}",
        items.items.len(),
        items.from,
        items.to
    );
    Ok(())
}

/// One carved piece: whole lines of the source, to be cut out.
struct Piece {
    name: String,
    lines: (usize, usize),
}

/// A piece that is a free item also needs its re-export.
struct Free {
    piece: Piece,
    public: bool,
}

fn carve(files: &mut FileSet, service_dir: &str, items: &Items) -> Result<()> {
    let from = format!("{service_dir}/{}", items.from);
    let to = format!("{service_dir}/{}", items.to);
    if files.exists(&to) {
        bail!("{to} exists already");
    }
    let source = files.read(&from)?.to_string();
    let file = syn::parse_file(&source).with_context(|| format!("parse {from}"))?;
    let index = LineIndex::new(&source);
    let wanted = |name: &str| items.items.iter().any(|item| item == name);

    let mut splices = Vec::new();
    let mut free = Vec::new();
    for item in &file.items {
        let Some(named) = source::named(item).filter(|named| wanted(&named.name)) else {
            continue;
        };
        splices.extend(source::widen(&index, named.visibility, named.keyword));
        free.push(Free {
            public: matches!(named.visibility, Visibility::Public(_)),
            piece: Piece {
                name: named.name,
                lines: source::line_range(item),
            },
        });
    }
    let mut methods = Vec::new();
    for item in &file.items {
        let Item::Impl(node) = item else { continue };
        for method in
            source::vault_methods_of(node).filter(|method| wanted(&method.sig.ident.to_string()))
        {
            splices.extend(source::widen(
                &index,
                &method.vis,
                source::signature_start(&method.sig),
            ));
            methods.push(Piece {
                name: method.sig.ident.to_string(),
                lines: source::line_range(method),
            });
        }
    }
    if let Some(missing) = items.items.iter().find(|name| {
        !free.iter().any(|item| &item.piece.name == *name)
            && !methods.iter().any(|item| &item.name == *name)
    }) {
        bail!("`{missing}` is neither a free item nor a vault method of {from}");
    }

    let widened = source::apply(&source, splices);
    let lines: Vec<&str> = widened.split_inclusive('\n').collect();
    let block = |piece: &Piece| lines[piece.lines.0 - 1..piece.lines.1].concat();
    let mut body = String::from("use super::*;\n");
    for item in &free {
        body += &format!("\n{}", block(&item.piece));
    }
    if !methods.is_empty() {
        let inner: Vec<String> = methods.iter().map(block).collect();
        body += &format!("\nimpl UnlockedVault {{\n{}}}\n", inner.join("\n"));
    }
    let cut: Vec<(usize, usize)> = free
        .iter()
        .map(|item| item.piece.lines)
        .chain(methods.iter().map(|item| item.lines))
        .collect();
    let remaining = remove_lines(&lines, &cut);
    refuse_private_fields(&file, &source, &remaining, &free)?;
    files.set(&from, remaining)?;
    files.set(&to, body)?;
    files.declare(
        &format!("{service_dir}/mod.rs"),
        module_name(&items.to)?,
        &declaration(&items.to, &free)?,
    )
}

/// A carved struct with a private field that the code left behind still reads cannot move: the
/// field would be private to the new module. The message names the field.
fn refuse_private_fields(
    file: &syn::File,
    source: &str,
    remaining: &str,
    free: &[Free],
) -> Result<()> {
    let carved_structs: Vec<&str> = free
        .iter()
        .map(|item| item.piece.name.as_str())
        .filter(|name| {
            file.items
                .iter()
                .any(|item| matches!(item, Item::Struct(node) if node.ident == name))
        })
        .collect();
    if carved_structs.is_empty() {
        return Ok(());
    }
    let reads = field_reads(remaining)?;
    for name in carved_structs {
        if let Some(field) = private_fields(source, name)?
            .into_iter()
            .find(|field| reads.contains(field))
        {
            bail!(
                "`{name}` is carved but its private field `{field}` is read by code that stays: make it `pub(crate)` first"
            );
        }
    }
    Ok(())
}

fn module_name(file: &str) -> Result<&str> {
    file.strip_suffix(".rs")
        .filter(|name| !name.contains('/'))
        .with_context(|| format!("`to` must be a plain `<name>.rs`, not {file}"))
}

/// `mod x;` plus the re-exports of the free items (`pub use` of the public ones, `pub(crate) use`
/// of the ones carve widened).
fn declaration(to: &str, free: &[Free]) -> Result<Vec<String>> {
    let module = module_name(to)?;
    let mut lines = vec![format!("mod {module};")];
    for (public, keyword) in [(true, "pub use"), (false, "pub(crate) use")] {
        let names: Vec<&str> = free
            .iter()
            .filter(|item| item.public == public)
            .map(|item| item.piece.name.as_str())
            .collect();
        if !names.is_empty() {
            lines.push(format!("{keyword} {module}::{{{}}};", names.join(", ")));
        }
    }
    Ok(lines)
}

/// The text without the given line ranges (1-based, inclusive). A blank line that would be left
/// doubled by a cut goes with it.
fn remove_lines(lines: &[&str], cuts: &[(usize, usize)]) -> String {
    let mut dropped = vec![false; lines.len()];
    for &(first, last) in cuts {
        dropped[first - 1..last].fill(true);
        let blank_before = first < 2 || lines[first - 2].trim().is_empty();
        if blank_before && lines.get(last).is_some_and(|line| line.trim().is_empty()) {
            dropped[last] = true;
        }
    }
    lines
        .iter()
        .zip(&dropped)
        .filter(|(_, gone)| !**gone)
        .map(|(line, _)| *line)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strangle::fixture::TempTree;
    use crate::strangle::map::MAP_FILE;
    use std::process::Command;

    const MAP: &str = r#"
service_dir = "crates/svc/service"
command_dir = "crates/app"
scan_dirs = ["crates/svc"]

[[area]]
name = "all"
task = "T"
group = "g"
context = "c"
files = ["mod.rs"]
"#;

    const MOD_RS: &str = "//! The service.
mod other;
#[cfg(test)]
mod golden;

pub struct UnlockedVault {
    pub(crate) count: u32,
}

/// The draft of a store.
#[derive(Debug, Clone)]
struct Draft {
    /// How many.
    pub n: u32,
}

/// Counts one more.
fn bump(value: u32) -> u32 {
    value + 1
}

pub fn keep() -> u32 {
    0
}

pub const LIMIT: u32 = 3;

impl UnlockedVault {
    /// Stays here.
    pub fn stay(&self) -> u32 {
        self.count + bump(LIMIT) + self.second()
    }

    /// Creates a draft.
    pub fn create(&mut self) -> Draft {
        self.count = bump(self.count);
        Draft { n: self.count }
    }

    fn second(&self) -> u32 {
        self.count
    }

    pub(crate) fn third(&self) -> u32 {
        keep()
    }
}
";

    fn items(names: &[&str]) -> Items {
        Items {
            from: "mod.rs".into(),
            to: "stores.rs".into(),
            items: names.iter().map(|name| (*name).to_string()).collect(),
        }
    }

    fn tree() -> TempTree {
        TempTree::new(&[
            (MAP_FILE, MAP),
            ("crates/svc/lib.rs", "pub mod service;\n"),
            ("crates/svc/service/mod.rs", MOD_RS),
            ("crates/svc/service/other.rs", "//! other\n"),
            ("crates/svc/service/golden.rs", "//! golden\n"),
        ])
    }

    fn carved(tree: &TempTree, names: &[&str]) -> (String, String) {
        let mut files = FileSet::new(tree.root());
        carve(&mut files, "crates/svc/service", &items(names)).unwrap();
        files.write().unwrap();
        let read = |name: &str| {
            std::fs::read_to_string(tree.root().join("crates/svc/service").join(name)).unwrap()
        };
        (read("mod.rs"), read("stores.rs"))
    }

    fn compiles(tree: &TempTree) -> bool {
        let out = Command::new("rustc")
            .args([
                "--edition",
                "2021",
                "--crate-type",
                "lib",
                "--emit",
                "metadata",
                "--cap-lints",
                "allow",
                "-o",
            ])
            .arg(tree.root().join("out.rmeta"))
            .arg(tree.root().join("crates/svc/lib.rs"))
            .output()
            .expect("rustc runs");
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        true
    }

    #[test]
    fn items_and_methods_move_in_order_with_their_docs_and_the_result_compiles() {
        let tree = tree();
        // `golden`/`other` stay declared; the fixture needs them as empty files only.
        let (rest, moved) = carved(&tree, &["Draft", "bump", "create", "second"]);
        assert_eq!(
            moved,
            "use super::*;

/// The draft of a store.
#[derive(Debug, Clone)]
pub(crate) struct Draft {
    /// How many.
    pub n: u32,
}

/// Counts one more.
pub(crate) fn bump(value: u32) -> u32 {
    value + 1
}

impl UnlockedVault {
    /// Creates a draft.
    pub fn create(&mut self) -> Draft {
        self.count = bump(self.count);
        Draft { n: self.count }
    }

    pub(crate) fn second(&self) -> u32 {
        self.count
    }
}
"
        );
        assert_eq!(
            rest,
            "//! The service.
mod other;
#[cfg(test)]
mod golden;
mod stores;
pub(crate) use stores::{Draft, bump};

pub struct UnlockedVault {
    pub(crate) count: u32,
}

pub fn keep() -> u32 {
    0
}

pub const LIMIT: u32 = 3;

impl UnlockedVault {
    /// Stays here.
    pub fn stay(&self) -> u32 {
        self.count + bump(LIMIT) + self.second()
    }

    pub(crate) fn third(&self) -> u32 {
        keep()
    }
}
"
        );
        assert!(compiles(&tree));
    }

    #[test]
    fn public_items_are_reexported_with_pub_use() {
        let tree = tree();
        let (rest, _) = carved(&tree, &["keep", "LIMIT"]);
        assert!(
            rest.contains("mod stores;\npub use stores::{keep, LIMIT};\n"),
            "{rest}"
        );
    }

    #[test]
    fn an_unknown_name_or_an_existing_target_is_refused() {
        let tree = tree();
        let mut files = FileSet::new(tree.root());
        let error = carve(&mut files, "crates/svc/service", &items(&["nope"])).unwrap_err();
        assert!(error.to_string().contains("`nope`"), "{error}");
        let mut files = FileSet::new(tree.root());
        let clash = Items {
            to: "other.rs".into(),
            ..items(&["keep"])
        };
        assert!(
            carve(&mut files, "crates/svc/service", &clash)
                .unwrap_err()
                .to_string()
                .contains("exists")
        );
    }

    #[test]
    fn a_carved_struct_whose_private_field_is_read_by_the_rest_is_refused_by_field_name() {
        let tree = TempTree::new(&[
            (MAP_FILE, MAP),
            (
                "crates/svc/service/mod.rs",
                "struct Draft {\n    secret: u32,\n}\npub fn keep(draft: &Draft) -> u32 { draft.secret }\n",
            ),
        ]);
        let map = Map::load(tree.root()).unwrap();
        let mut files = FileSet::new(tree.root());
        let error = carve(&mut files, &map.service_dir, &items(&["Draft"])).unwrap_err();
        let message = error.to_string();
        assert!(
            message.contains("`secret`") && message.contains("`Draft`"),
            "{message}"
        );
    }

    #[test]
    fn a_carved_struct_whose_private_field_nobody_else_reads_moves() {
        let tree = TempTree::new(&[
            (MAP_FILE, MAP),
            (
                "crates/svc/service/mod.rs",
                "struct Draft {\n    secret: u32,\n}\npub fn keep() -> u32 { 1 }\n",
            ),
        ]);
        let map = Map::load(tree.root()).unwrap();
        let mut files = FileSet::new(tree.root());
        carve(&mut files, &map.service_dir, &items(&["Draft"])).unwrap();
    }

    #[test]
    fn a_carved_struct_whose_field_is_already_pub_crate_is_not_refused() {
        let tree = TempTree::new(&[
            (MAP_FILE, MAP),
            (
                "crates/svc/service/mod.rs",
                "struct Draft {\n    pub(crate) secret: u32,\n    pub(super) other: u32,\n}\npub fn keep(draft: &Draft) -> u32 { draft.secret + draft.other }\n",
            ),
        ]);
        let map = Map::load(tree.root()).unwrap();
        let mut files = FileSet::new(tree.root());
        carve(&mut files, &map.service_dir, &items(&["Draft"])).unwrap();
    }
}
