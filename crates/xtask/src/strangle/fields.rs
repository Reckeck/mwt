//! Field helpers shared by `relocate` and `carve`: which named fields of a struct stay hidden from
//! the rest of the crate, and which fields a piece of code reads (spec D1, OD5).

use std::collections::BTreeSet;

use anyhow::{Context, Result};
use proc_macro2::{Delimiter, Spacing, TokenStream, TokenTree};
use syn::{Fields, Item, Visibility};

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

/// The named fields of `struct <name>` that are not visible outside the module that defines the
/// struct: no `pub` at all, `pub(self)` or `pub(in self)`. Any wider form (`pub`, `pub(crate)`,
/// `pub(super)`, `pub(in path)`) reaches code outside that module's subtree, which is where a moved or carved
/// item lands, so those fields are never reported. Known limit: `pub(in self::child)` and other
/// subtree paths count as visible (not hidden), the conservative side.
pub fn hidden_fields(text: &str, name: &str) -> Result<Vec<String>> {
    let file = syn::parse_file(text).context("parse")?;
    Ok(struct_fields(&file, name)
        .filter(|field| !is_visible_outside_module(&field.vis))
        .filter_map(|field| field.ident.as_ref().map(ToString::to_string))
        .collect())
}

fn is_visible_outside_module(vis: &Visibility) -> bool {
    match vis {
        Visibility::Public(_) => true,
        Visibility::Inherited => false,
        Visibility::Restricted(restricted) => !restricted.path.is_ident("self"),
    }
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
    use crate::strangle::scan::VAULT_RECEIVERS;

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
    fn only_fields_invisible_outside_the_module_are_hidden() {
        let text = "pub struct V {\n    /// Doc.\n    doc: u8,\n    pub(super) count: u8,\n    pub ok: u8,\n    other: u8,\n    pub(crate) wide: u8,\n    pub(self) own: u8,\n}\n";
        assert_eq!(hidden_fields(text, "V").unwrap(), ["doc", "other", "own"]);
    }

    #[test]
    fn pub_in_self_is_hidden_and_pub_in_a_subtree_path_is_not() {
        let text = "struct V {\n    pub(in self) own: u8,\n    pub(in self::child) sub: u8,\n    pub(in crate::a) far: u8,\n}\n";
        assert_eq!(hidden_fields(text, "V").unwrap(), ["own"]);
    }
}
