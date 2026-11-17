//! A function signature as normalized text, cut from the source by span: the one place that knows
//! how to write `pub fn name(params) -> R` back out. `baseline` records it, `forward` rebuilds the
//! method signature from it.

use syn::spanned::Spanned;
use syn::{Attribute, Expr, ExprLit, Lit, Meta, ReturnType, Signature};

use super::source::LineIndex;

/// The pieces of a signature, each as one line of text.
pub struct Parts {
    pub name: String,
    /// `async `, `const `, `unsafe ` in front of `fn`, or empty.
    qualifiers: String,
    /// `<T: Clone>` or empty.
    generics: String,
    /// One text per parameter, the receiver included.
    pub params: Vec<String>,
    /// `-> R` or empty.
    output: String,
    /// `where ...` or empty.
    where_clause: String,
}

impl Parts {
    pub fn new(source: &str, index: &LineIndex, sig: &Signature) -> Self {
        let qualifiers = [
            sig.constness.as_ref().map(|_| "const "),
            sig.asyncness.as_ref().map(|_| "async "),
            sig.unsafety.as_ref().map(|_| "unsafe "),
        ]
        .into_iter()
        .flatten()
        .collect();
        Self {
            name: sig.ident.to_string(),
            qualifiers,
            generics: if sig.generics.params.is_empty() {
                String::new()
            } else {
                span_text(&sig.generics, source, index)
            },
            params: sig
                .inputs
                .iter()
                .map(|param| span_text(param, source, index))
                .collect(),
            output: match &sig.output {
                ReturnType::Default => String::new(),
                typed @ ReturnType::Type(..) => span_text(typed, source, index),
            },
            where_clause: sig
                .generics
                .where_clause
                .as_ref()
                .map_or_else(String::new, |clause| span_text(clause, source, index)),
        }
    }

    /// `pub fn name<G>(first, second) -> R where ..` with the given parameters.
    pub fn render(&self, visibility: &str, params: &[String]) -> String {
        let mut text = format!(
            "{visibility}{}fn {}{}({})",
            self.qualifiers,
            self.name,
            self.generics,
            params.join(", ")
        );
        for tail in [&self.output, &self.where_clause] {
            if !tail.is_empty() {
                text.push(' ');
                text.push_str(tail);
            }
        }
        text
    }
}

/// The source text of a node, whitespace collapsed to single spaces.
fn span_text(node: &impl Spanned, source: &str, index: &LineIndex) -> String {
    let span = node.span();
    let raw = &source[index.offset(span.start())..index.offset(span.end())];
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The `///` lines of an item, as the text after the three slashes (`" docs"`).
pub fn doc_lines(attrs: &[Attribute]) -> Vec<String> {
    attrs
        .iter()
        .filter_map(|attr| match &attr.meta {
            Meta::NameValue(pair) if pair.path.is_ident("doc") => match &pair.value {
                Expr::Lit(ExprLit {
                    lit: Lit::Str(text),
                    ..
                }) => Some(text.value()),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts_of(source: &str) -> Parts {
        let file = syn::parse_file(source).unwrap();
        let syn::Item::Fn(node) = &file.items[0] else {
            panic!("a fn")
        };
        Parts::new(source, &LineIndex::new(source), &node.sig)
    }

    #[test]
    fn a_multi_line_signature_is_one_normalized_line() {
        let parts = parts_of(
            "pub async fn load<T: Clone>(\n    vault: &mut V,\n    id: &str,\n) -> Result<T>\nwhere\n    T: Send,\n{ todo!() }",
        );
        assert_eq!(parts.params, ["vault: &mut V", "id: &str"]);
        assert_eq!(
            parts.render("pub ", &parts.params),
            "pub async fn load<T: Clone>(vault: &mut V, id: &str) -> Result<T> where T: Send,"
        );
    }

    #[test]
    fn doc_comments_come_out_as_text() {
        let file = syn::parse_file("/// One.\n///Two.\nfn x() {}").unwrap();
        let syn::Item::Fn(node) = &file.items[0] else {
            panic!("a fn")
        };
        assert_eq!(doc_lines(&node.attrs), [" One.", "Two."]);
    }
}
