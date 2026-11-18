//! `strangle widen`: every non-`pub` method of every `impl UnlockedVault` under the service dir
//! becomes `pub(crate)`, so a method moved out of `service/` keeps access to its private siblings
//! (spec D1, OD5). One token changes per method; nothing else. Fields of `UnlockedVault` (`password`,
//! `key`, ...) are never touched here: the few a moved file reads are widened by hand in a named task.

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

/// The lower-snake binding a struct is conventionally held in (`Draft` -> `draft`).
pub fn binding_of(struct_name: &str) -> String {
    let mut binding = String::new();
    for (at, letter) in struct_name.chars().enumerate() {
        if letter.is_uppercase() && at > 0 {
            binding.push('_');
        }
        binding.extend(letter.to_lowercase());
    }
    binding
}

/// Names read as a field (`<receiver>.name`, not a call) anywhere in the source, macro bodies
/// included. Only a read whose token before the `.` is one of `receivers` counts: `other.key`
/// is not a read of the struct's `key`.
pub fn field_reads(text: &str, receivers: &[&str]) -> Result<BTreeSet<String>> {
    let tokens: TokenStream = text
        .parse()
        .map_err(|error| anyhow::anyhow!("tokenize: {error}"))?;
    let mut names = BTreeSet::new();
    collect_field_reads(
        &tokens.into_iter().collect::<Vec<_>>(),
        receivers,
        &mut names,
    );
    Ok(names)
}

fn collect_field_reads(tokens: &[TokenTree], receivers: &[&str], names: &mut BTreeSet<String>) {
    for (at, token) in tokens.iter().enumerate() {
        match token {
            TokenTree::Group(group) => {
                collect_field_reads(
                    &group.stream().into_iter().collect::<Vec<_>>(),
                    receivers,
                    names,
                );
            }
            TokenTree::Punct(dot) if dot.as_char() == '.' && dot.spacing() == Spacing::Alone => {
                let range_end = at > 0
                    && matches!(&tokens[at - 1], TokenTree::Punct(prev) if prev.as_char() == '.');
                let on_receiver = at > 0
                    && matches!(&tokens[at - 1], TokenTree::Ident(prev)
                        if receivers.contains(&prev.to_string().as_str()));
                let called = matches!(
                    tokens.get(at + 2),
                    Some(TokenTree::Group(group)) if group.delimiter() == Delimiter::Parenthesis
                );
                if let (false, false, true, Some(TokenTree::Ident(name))) =
                    (range_end, called, on_receiver, tokens.get(at + 1))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strangle::fixture::TempTree;
    use crate::strangle::map::{MAP_FILE, Map};
    use crate::strangle::scan::VAULT_RECEIVERS;

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
            &["v"],
        )
        .unwrap();
        let names: Vec<&str> = reads.iter().map(String::as_str).collect();
        assert_eq!(names, ["count", "doc", "n", "path"]);
    }

    #[test]
    fn field_reads_count_only_the_given_receivers() {
        let reads = field_reads(
            "fn a(&self, other: &O) { let _ = self.key + other.key + other.name + vault.doc; }",
            &VAULT_RECEIVERS,
        )
        .unwrap();
        let names: Vec<&str> = reads.iter().map(String::as_str).collect();
        assert_eq!(names, ["doc", "key"]);
        let others = field_reads("fn a(o: &O) { let _ = o.key; }", &VAULT_RECEIVERS).unwrap();
        assert!(others.is_empty());
    }

    #[test]
    fn a_binding_is_the_snake_case_of_the_struct() {
        assert_eq!(binding_of("Draft"), "draft");
        assert_eq!(binding_of("LoyaltyCard"), "loyalty_card");
    }

    #[test]
    fn field_visibility_helpers_tell_widened_from_private() {
        let text = "pub struct V {\n    /// Doc.\n    doc: u8,\n    pub(super) count: u8,\n    pub ok: u8,\n    other: u8,\n    pub(crate) wide: u8,\n}\n";
        assert_eq!(private_fields(text, "V").unwrap(), ["doc", "other"]);
        assert_eq!(
            unwidened_fields(text, "V").unwrap(),
            ["doc", "count", "other"]
        );
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
