//! `strangle widen`: every non-`pub` method of every `impl UnlockedVault` under the service dir
//! becomes `pub(crate)`, so a method moved out of `service/` keeps access to its private siblings
//! (spec D1, OD5). One token changes per method; nothing else.

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Context, Result};
use proc_macro2::{Delimiter, Spacing, TokenStream, TokenTree};
use syn::{Fields, Item, Visibility};

use super::Workspace;
use super::fileset::FileSet;
use super::source::{self, LineIndex};

/// The struct whose private fields a moved method may read (spec D1, OD5).
pub const VAULT: &str = "UnlockedVault";

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
    widened += widen_vault_definition(&mut files, &workspace.map.service_dir)?;
    let changed = files.changed().len();
    files.write()?;
    println!("widen: {widened} method(s) and field(s) in {changed} file(s) are pub(crate) now");
    Ok(())
}

/// The fields of `UnlockedVault` in `service/mod.rs` get the rule of the methods, so a moved
/// method keeps reading them (`relocate` refuses until this ran).
fn widen_vault_definition(files: &mut FileSet, service_dir: &str) -> Result<usize> {
    let path = format!("{service_dir}/mod.rs");
    let text = files.read(&path)?.to_string();
    let wanted: BTreeSet<String> = unwidened_fields(&text, VAULT)
        .with_context(|| path.clone())?
        .into_iter()
        .collect();
    let (widened, names) = widen_fields(&text, VAULT, &wanted).with_context(|| path.clone())?;
    if !names.is_empty() {
        files.set(&path, widened)?;
    }
    Ok(names.len())
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

/// Names read as a field (`.name`, not a call) anywhere in the source, macro bodies included.
pub fn field_reads(text: &str) -> Result<BTreeSet<String>> {
    let tokens: TokenStream = text
        .parse()
        .map_err(|error| anyhow::anyhow!("tokenize: {error}"))?;
    let mut names = BTreeSet::new();
    collect_field_reads(&tokens.into_iter().collect::<Vec<_>>(), &mut names);
    Ok(names)
}

fn collect_field_reads(tokens: &[TokenTree], names: &mut BTreeSet<String>) {
    for (at, token) in tokens.iter().enumerate() {
        match token {
            TokenTree::Group(group) => {
                collect_field_reads(&group.stream().into_iter().collect::<Vec<_>>(), names);
            }
            TokenTree::Punct(dot) if dot.as_char() == '.' && dot.spacing() == Spacing::Alone => {
                let range_end = at > 0
                    && matches!(&tokens[at - 1], TokenTree::Punct(prev) if prev.as_char() == '.');
                let called = matches!(
                    tokens.get(at + 2),
                    Some(TokenTree::Group(group)) if group.delimiter() == Delimiter::Parenthesis
                );
                if let (false, false, Some(TokenTree::Ident(name))) =
                    (range_end, called, tokens.get(at + 1))
                {
                    names.insert(name.to_string());
                }
            }
            _ => {}
        }
    }
}

/// The named fields of `struct <name>` that no other module sees (no `pub` at all), in the
/// file's text: a `pub(crate)` or `pub(super)` field is already widened.
pub fn private_fields(text: &str, name: &str) -> Result<Vec<String>> {
    fields_where(text, name, |vis| matches!(vis, Visibility::Inherited))
}

/// The named fields of `struct <name>` that `widen` would still change: neither `pub` nor
/// `pub(crate)`. A method that leaves `service/` cannot read them.
pub fn unwidened_fields(text: &str, name: &str) -> Result<Vec<String>> {
    fields_where(text, name, |vis| {
        !matches!(vis, Visibility::Public(_)) && !is_crate_visible(vis)
    })
}

fn is_crate_visible(vis: &Visibility) -> bool {
    matches!(vis, Visibility::Restricted(restricted) if restricted.path.is_ident("crate"))
}

fn fields_where(
    text: &str,
    name: &str,
    wanted: impl Fn(&Visibility) -> bool,
) -> Result<Vec<String>> {
    let file = syn::parse_file(text).context("parse")?;
    Ok(struct_fields(&file, name)
        .filter(|field| wanted(&field.vis))
        .filter_map(|field| field.ident.as_ref().map(ToString::to_string))
        .collect())
}

fn struct_fields<'ast>(
    file: &'ast syn::File,
    name: &'ast str,
) -> impl Iterator<Item = &'ast syn::Field> {
    file.items
        .iter()
        .filter_map(move |item| match item {
            Item::Struct(node) if node.ident == name => Some(&node.fields),
            _ => None,
        })
        .flat_map(|fields| match fields {
            Fields::Named(named) => named.named.iter().collect::<Vec<_>>(),
            _ => Vec::new(),
        })
}

/// The source with the listed fields of `struct <name>` widened by the rule of `widen` (a
/// non-`pub` field becomes `pub(crate)`), and the names of the fields that changed.
pub fn widen_fields(
    text: &str,
    name: &str,
    wanted: &BTreeSet<String>,
) -> Result<(String, Vec<String>)> {
    let file = syn::parse_file(text).context("parse")?;
    let index = LineIndex::new(text);
    let mut splices = Vec::new();
    let mut widened = Vec::new();
    for field in struct_fields(&file, name) {
        let Some(ident) = field
            .ident
            .as_ref()
            .filter(|ident| wanted.contains(&ident.to_string()))
        else {
            continue;
        };
        if let Some(splice) = source::widen(&index, &field.vis, ident.span().start()) {
            splices.push(splice);
            widened.push(ident.to_string());
        }
    }
    Ok((source::apply(text, splices), widened))
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
    fn field_reads_see_fields_not_calls_ranges_or_macro_free_text() {
        let reads = field_reads(
            "fn a(v: &V) { let _ = v.doc.len() + v.count; for _ in 0..v.n {} println!(\"{}\", v.path); v.call(); }",
        )
        .unwrap();
        let names: Vec<&str> = reads.iter().map(String::as_str).collect();
        assert_eq!(names, ["count", "doc", "n", "path"]);
    }

    #[test]
    fn only_the_wanted_private_fields_are_widened() {
        let text = "pub struct V {\n    /// Doc.\n    doc: u8,\n    pub(super) count: u8,\n    pub ok: u8,\n    other: u8,\n}\n";
        assert_eq!(private_fields(text, "V").unwrap(), ["doc", "other"]);
        assert_eq!(
            unwidened_fields(text, "V").unwrap(),
            ["doc", "count", "other"]
        );
        let wanted: BTreeSet<String> = ["doc", "count", "ok"].map(String::from).into();
        let (widened, names) = widen_fields(text, "V", &wanted).unwrap();
        assert_eq!(names, ["doc", "count"]);
        assert_eq!(
            widened,
            "pub struct V {\n    /// Doc.\n    pub(crate) doc: u8,\n    pub(crate) count: u8,\n    pub ok: u8,\n    other: u8,\n}\n"
        );
    }
}
