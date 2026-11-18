//! The `syn` visitor behind `scan_source`: one pass over a file that fills its `FileFacts`.

use anyhow::{Context, Result};
use proc_macro2::{TokenStream, TokenTree};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::token::Comma;
use syn::visit::{self, Visit};
use syn::{
    Attribute, Expr, ExprCall, ExprMethodCall, Ident, ImplItemFn, ItemFn, ItemImpl, ItemMod, Macro,
    Meta, Type, Visibility,
};

use super::{Call, FileFacts, Method, ModDecl, TestFn, VAULT_RECEIVERS, VAULT_TYPE};

pub fn scan_source(source: &str) -> Result<FileFacts> {
    let file = syn::parse_file(source).context("parse")?;
    let mut scanner = Scanner::default();
    scanner.facts.lines = source.matches('\n').count();
    scanner.visit_file(&file);
    Ok(scanner.facts)
}

#[derive(Default)]
struct Scanner {
    facts: FileFacts,
    test_depth: usize,
    in_vault_impl: bool,
    command: Option<Vec<String>>,
}

impl Scanner {
    fn in_test(&self) -> bool {
        self.test_depth > 0
    }

    fn with_test(&mut self, test: bool, body: impl FnOnce(&mut Self)) {
        self.test_depth += usize::from(test);
        body(self);
        self.test_depth -= usize::from(test);
    }

    fn note_ident(&mut self, name: String) {
        if self.in_test() {
            self.facts.test_idents.insert(name);
        } else {
            self.facts.other_idents.insert(name);
        }
    }

    fn note_test_fn(&mut self, attrs: &[Attribute], name: &Ident) {
        if attrs.iter().any(is_test_marker) {
            self.facts.tests.push(TestFn {
                name: name.to_string(),
            });
        }
    }

    fn note_call(&mut self, name: String, line: usize, on_vault: bool) {
        self.facts.calls.push(Call {
            name,
            line,
            on_vault,
            in_test: self.in_test(),
        });
    }
}

impl<'ast> Visit<'ast> for Scanner {
    fn visit_item_mod(&mut self, node: &'ast ItemMod) {
        let marker = test_attr_line(&node.attrs);
        if node.content.is_none() {
            self.facts.mods.push(ModDecl {
                name: node.ident.to_string(),
                in_test: marker.is_some() || self.in_test(),
            });
        } else if let Some(line) = marker {
            let first = self
                .facts
                .first_test_line
                .map_or(line, |known| known.min(line));
            self.facts.first_test_line = Some(first);
        }
        self.with_test(marker.is_some(), |this| visit::visit_item_mod(this, node));
    }

    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        self.note_test_fn(&node.attrs, &node.sig.ident);
        let command = is_tauri_command(&node.attrs);
        self.with_test(test_attr_line(&node.attrs).is_some(), |this| {
            if command {
                this.command = Some(Vec::new());
            }
            visit::visit_item_fn(this, node);
            if let Some(names) = this.command.take() {
                this.facts.commands.push(names);
            }
        });
    }

    fn visit_item_impl(&mut self, node: &'ast ItemImpl) {
        let vault = node.trait_.is_none() && names_type(&node.self_ty, VAULT_TYPE);
        let outer = std::mem::replace(&mut self.in_vault_impl, vault);
        self.with_test(test_attr_line(&node.attrs).is_some(), |this| {
            visit::visit_item_impl(this, node);
        });
        self.in_vault_impl = outer;
    }

    fn visit_impl_item_fn(&mut self, node: &'ast ImplItemFn) {
        self.note_test_fn(&node.attrs, &node.sig.ident);
        let test = test_attr_line(&node.attrs).is_some();
        if self.in_vault_impl {
            let mutating = node.sig.receiver().is_some_and(|receiver| {
                receiver.reference.is_some() && receiver.mutability.is_some()
            });
            let method = Method {
                name: node.sig.ident.to_string(),
                public: matches!(node.vis, Visibility::Public(_)),
                mutating,
                line: node.sig.fn_token.span.start().line,
                in_test: test || self.in_test(),
            };
            self.facts.methods.push(method);
        }
        self.with_test(test, |this| visit::visit_impl_item_fn(this, node));
    }

    fn visit_expr_method_call(&mut self, node: &'ast ExprMethodCall) {
        // Receiver first, so the names of a command come out in source order.
        visit::visit_expr(self, &node.receiver);
        let name = node.method.to_string();
        let on_vault = matches!(&*node.receiver, Expr::Path(path)
            if VAULT_RECEIVERS.iter().any(|receiver| path.path.is_ident(receiver)));
        if let Some(names) = &mut self.command {
            names.push(name.clone());
        }
        self.note_ident(name.clone());
        self.note_call(name, node.method.span().start().line, on_vault);
        for argument in &node.args {
            visit::visit_expr(self, argument);
        }
    }

    fn visit_expr_call(&mut self, node: &'ast ExprCall) {
        if let Expr::Path(path) = &*node.func {
            let segments = &path.path.segments;
            if let (2, Some(first), Some(last)) =
                (segments.len(), segments.first(), segments.last())
                && first.ident == "Self"
            {
                self.note_call(last.ident.to_string(), last.ident.span().start().line, true);
            }
        }
        visit::visit_expr_call(self, node);
    }

    fn visit_macro(&mut self, node: &'ast Macro) {
        for name in idents_of(node.tokens.clone()) {
            self.note_ident(name);
        }
        match macro_arguments(node) {
            Some(arguments) => {
                for argument in &arguments {
                    visit::visit_expr(self, argument);
                }
            }
            None if cannot_hold_vault_call(node) => {}
            None => self.facts.unscanned.push(macro_line(node)),
        }
    }

    fn visit_ident(&mut self, node: &'ast Ident) {
        self.note_ident(node.to_string());
    }
}

/// Macros whose body is a definition or data, never a vault call: `macro_rules!`, `json!`.
fn cannot_hold_vault_call(node: &Macro) -> bool {
    node.path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "macro_rules" || segment.ident == "json")
}

/// The body of a macro call read as `Punctuated<Expr, Comma>` (`format!`, `assert!`, `vec![a, b]`);
/// `None` when it does not parse that way.
pub(in crate::strangle) fn macro_arguments(node: &Macro) -> Option<Punctuated<Expr, Comma>> {
    node.parse_body_with(Punctuated::<Expr, Comma>::parse_terminated)
        .ok()
}

/// The line of the macro's name.
pub(in crate::strangle) fn macro_line(node: &Macro) -> usize {
    node.path.span().start().line
}

/// Every identifier in a token stream, groups included.
fn idents_of(tokens: TokenStream) -> Vec<String> {
    let mut found = Vec::new();
    for tree in tokens {
        match tree {
            TokenTree::Ident(ident) => found.push(ident.to_string()),
            TokenTree::Group(group) => found.extend(idents_of(group.stream())),
            TokenTree::Punct(_) | TokenTree::Literal(_) => {}
        }
    }
    found
}

pub(in crate::strangle) fn names_type(ty: &Type, name: &str) -> bool {
    matches!(ty, Type::Path(path) if path.path.segments.last().is_some_and(|segment| segment.ident == name))
}

/// The line of the first `#[cfg(test)]` or `#[test]`-like attribute, if any.
fn test_attr_line(attrs: &[Attribute]) -> Option<usize> {
    attrs
        .iter()
        .find(|attr| is_test_attr(attr))
        .map(|attr| attr.pound_token.span.start().line)
}

pub(in crate::strangle) fn is_test_attr(attr: &Attribute) -> bool {
    if attr.path().is_ident("cfg") {
        let Meta::List(list) = &attr.meta else {
            return false;
        };
        let names = idents_of(list.tokens.clone());
        return names.iter().any(|name| name == "test") && !names.iter().any(|name| name == "not");
    }
    is_test_marker(attr)
}

/// `#[test]`, `#[tokio::test]`: the attribute whose last path segment is `test`.
fn is_test_marker(attr: &Attribute) -> bool {
    attr.path()
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "test")
}

/// `#[tauri::command]`, or `#[ipc(..)]` (core-ipc: the attribute expands to `tauri::command`).
fn is_tauri_command(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        let names: Vec<String> = attr
            .path()
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect();
        names == ["tauri", "command"] || names.last().is_some_and(|last| last == "ipc")
    })
}
