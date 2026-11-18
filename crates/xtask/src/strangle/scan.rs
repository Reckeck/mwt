//! The one `syn` scanner: facts of a Rust source (methods of `impl UnlockedVault`, calls, tauri
//! commands, test code, inline test modules, `mod x;` declarations) and the set of scanned files.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

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

/// The type whose impl blocks the areas are made of.
pub(super) const VAULT_TYPE: &str = "UnlockedVault";

/// A method of `impl UnlockedVault`.
#[derive(Debug, Clone)]
pub struct Method {
    pub name: String,
    /// Plain `pub`; `pub(crate)`, `pub(super)` and private are all hidden.
    pub public: bool,
    /// The receiver is `&mut self`: a command, not a query.
    pub mutating: bool,
    pub line: usize,
    pub in_test: bool,
}

/// A method call `x.name(..)` or `Self::name(..)`.
#[derive(Debug, Clone)]
pub struct Call {
    pub name: String,
    pub line: usize,
    /// The receiver is `self` or `vault`, or the path starts with `Self::`.
    pub on_vault: bool,
    pub in_test: bool,
}

/// A `mod name;` declaration (the module lives in another file).
#[derive(Debug, Clone)]
pub struct ModDecl {
    pub name: String,
    pub in_test: bool,
}

/// A function marked `#[test]` (or `#[tokio::test]` and the like).
#[derive(Debug, Clone)]
pub struct TestFn {
    pub name: String,
}

#[derive(Debug, Default)]
pub struct FileFacts {
    /// Newlines, as `wc -l` counts them.
    pub lines: usize,
    /// The line of the `#[cfg(test)]` of the first inline `mod x { .. }`.
    pub first_test_line: Option<usize>,
    pub mods: Vec<ModDecl>,
    pub methods: Vec<Method>,
    pub tests: Vec<TestFn>,
    pub calls: Vec<Call>,
    /// Per `#[tauri::command]` function: the method names its body calls, in source order.
    pub commands: Vec<Vec<String>>,
    /// Every identifier written in test code.
    pub test_idents: BTreeSet<String>,
    /// Every identifier written outside test code (all of it matter when the whole file is a test).
    pub other_idents: BTreeSet<String>,
    /// Lines of macro calls whose body is not a comma-separated list of expressions: their calls
    /// are not seen.
    pub unscanned: Vec<usize>,
}

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
            if path.path.is_ident("self") || path.path.is_ident("vault"));
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
            None => self.facts.unscanned.push(macro_line(node)),
        }
    }

    fn visit_ident(&mut self, node: &'ast Ident) {
        self.note_ident(node.to_string());
    }
}

/// The body of a macro call read as `Punctuated<Expr, Comma>` (`format!`, `assert!`, `vec![a, b]`);
/// `None` when it does not parse that way.
pub(super) fn macro_arguments(node: &Macro) -> Option<Punctuated<Expr, Comma>> {
    node.parse_body_with(Punctuated::<Expr, Comma>::parse_terminated)
        .ok()
}

/// The line of the macro's name.
pub(super) fn macro_line(node: &Macro) -> usize {
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

pub(super) fn names_type(ty: &Type, name: &str) -> bool {
    matches!(ty, Type::Path(path) if path.path.segments.last().is_some_and(|segment| segment.ident == name))
}

/// The line of the first `#[cfg(test)]` or `#[test]`-like attribute, if any.
fn test_attr_line(attrs: &[Attribute]) -> Option<usize> {
    attrs
        .iter()
        .find(|attr| is_test_attr(attr))
        .map(|attr| attr.pound_token.span.start().line)
}

pub(super) fn is_test_attr(attr: &Attribute) -> bool {
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

/// A scanned file and whether it is test code as a whole.
#[derive(Debug)]
pub struct Scanned {
    pub facts: FileFacts,
    pub is_test: bool,
}

/// Every `.rs` file of the scanned directories, by path relative to the workspace root.
#[derive(Debug, Default)]
pub struct Corpus {
    files: BTreeMap<String, Scanned>,
}

impl Corpus {
    pub fn scan(root: &Path, dirs: &[String]) -> Result<Self> {
        let mut files = BTreeMap::new();
        for dir in dirs {
            collect(root, dir, &mut files)?;
        }
        let mut corpus = Self { files };
        corpus.mark_test_files();
        Ok(corpus)
    }

    #[cfg(test)]
    pub fn from_sources(sources: &[(&str, &str)]) -> Result<Self> {
        let mut files = BTreeMap::new();
        for (path, source) in sources {
            let facts = scan_source(source).with_context(|| format!("scan {path}"))?;
            files.insert(
                (*path).to_string(),
                Scanned {
                    facts,
                    is_test: false,
                },
            );
        }
        let mut corpus = Self { files };
        corpus.mark_test_files();
        Ok(corpus)
    }

    pub fn get(&self, path: &str) -> Option<&Scanned> {
        self.files.get(path)
    }

    /// The files whose path starts with `prefix` (a directory with its trailing `/`).
    pub fn under<'corpus>(
        &'corpus self,
        prefix: &str,
    ) -> impl Iterator<Item = (&'corpus str, &'corpus Scanned)> + use<'corpus> {
        let prefix = prefix.to_string();
        self.files
            .iter()
            .filter(move |(path, _)| path.starts_with(&prefix))
            .map(|(path, scanned)| (path.as_str(), scanned))
    }

    /// Identifiers that appear in test code anywhere in the corpus.
    pub fn test_idents(&self) -> BTreeSet<&str> {
        let mut found = BTreeSet::new();
        for scanned in self.files.values() {
            let sets = [
                Some(&scanned.facts.test_idents),
                scanned.is_test.then_some(&scanned.facts.other_idents),
            ];
            found.extend(sets.into_iter().flatten().flatten().map(String::as_str));
        }
        found
    }

    /// A file is test code when its name says so, when a `#[cfg(test)] mod x;` declares it, or when
    /// a test file declares it. Repeats until no file is added.
    fn mark_test_files(&mut self) {
        for (path, scanned) in &mut self.files {
            scanned.is_test = is_test_name(path);
        }
        loop {
            let mut found = Vec::new();
            for (path, scanned) in &self.files {
                for decl in &scanned.facts.mods {
                    if decl.in_test || scanned.is_test {
                        found.extend(
                            module_files(path, &decl.name)
                                .into_iter()
                                .filter(|child| self.files.contains_key(child)),
                        );
                    }
                }
            }
            let before = self
                .files
                .values()
                .filter(|scanned| scanned.is_test)
                .count();
            for child in found {
                if let Some(scanned) = self.files.get_mut(&child) {
                    scanned.is_test = true;
                }
            }
            if self
                .files
                .values()
                .filter(|scanned| scanned.is_test)
                .count()
                == before
            {
                return;
            }
        }
    }
}

fn collect(root: &Path, dir: &str, files: &mut BTreeMap<String, Scanned>) -> Result<()> {
    let path = root.join(dir);
    let Ok(entries) = std::fs::read_dir(&path) else {
        return Ok(());
    };
    for entry in entries {
        let entry = entry.with_context(|| format!("list {}", path.display()))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let relative = format!("{dir}/{name}");
        if entry.file_type()?.is_dir() {
            collect(root, &relative, files)?;
        } else if name.ends_with(".rs") {
            let source = std::fs::read_to_string(entry.path())
                .with_context(|| format!("read {relative}"))?;
            let facts = scan_source(&source).with_context(|| format!("scan {relative}"))?;
            files.insert(
                relative,
                Scanned {
                    facts,
                    is_test: false,
                },
            );
        }
    }
    Ok(())
}

fn is_test_name(path: &str) -> bool {
    let stem = path
        .rsplit('/')
        .next()
        .unwrap_or(path)
        .trim_end_matches(".rs");
    matches!(stem, "tests" | "testing" | "test_support" | "golden_tests")
        || stem.ends_with("_tests")
        || path.split('/').any(|part| part == "tests")
}

/// Where `mod name;` declared in `path` can live.
fn module_files(path: &str, name: &str) -> [String; 2] {
    let (dir, file) = path.rsplit_once('/').unwrap_or(("", path));
    let stem = file.trim_end_matches(".rs");
    let base = if matches!(stem, "mod" | "lib" | "main") {
        dir.to_string()
    } else {
        format!("{dir}/{stem}")
    };
    [format!("{base}/{name}.rs"), format!("{base}/{name}/mod.rs")]
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r##"
use crate::UnlockedVault;

impl UnlockedVault {
    pub fn plain(&self) -> u32 {
        let text = "} } { not a block";
        let raw = r#"pub fn fake(&self) {"#;
        self.helper(text, raw)
    }
    pub async fn later(&mut self) {
        self.persist().await;
        vault.other();
    }
    pub(crate) fn crate_wide(&self) {}
    pub(super) fn super_wide(&self) {}
    fn helper(&self, _a: &str, _b: &str) -> u32 { Self::tail() }
    fn tail() -> u32 { 1 }
}

impl Other {
    pub fn not_ours(&self) {}
}

#[cfg(test)]
mod declared;

#[cfg(test)]
mod tests {
    impl UnlockedVault {
        pub fn only_in_tests(&self) {}
    }
    #[test]
    fn pins() {
        assert_eq!(v.plain(), 1);
    }
}
"##;

    fn find<'facts>(facts: &'facts FileFacts, name: &str) -> &'facts Method {
        facts
            .methods
            .iter()
            .find(|method| method.name == name)
            .unwrap()
    }

    #[test]
    fn braces_in_strings_do_not_confuse_the_scanner() {
        let facts = scan_source(FIXTURE).unwrap();
        assert!(
            facts
                .methods
                .iter()
                .all(|method| method.name != "fake" && method.name != "not_ours")
        );
        assert!(find(&facts, "plain").public);
        assert!(!find(&facts, "plain").mutating);
        assert!(
            facts
                .calls
                .iter()
                .any(|call| call.name == "helper" && call.on_vault && !call.in_test)
        );
    }

    #[test]
    fn async_crate_and_super_methods_are_seen_with_their_visibility() {
        let facts = scan_source(FIXTURE).unwrap();
        assert!(find(&facts, "later").public && find(&facts, "later").mutating);
        for hidden in ["crate_wide", "super_wide", "helper", "tail"] {
            assert!(!find(&facts, hidden).public, "{hidden}");
        }
        assert!(facts.calls.iter().any(|call| call.name == "persist"));
        assert!(
            facts
                .calls
                .iter()
                .any(|call| call.name == "other" && call.on_vault)
        );
        assert!(
            facts
                .calls
                .iter()
                .any(|call| call.name == "tail" && call.on_vault)
        );
    }

    #[test]
    fn the_inline_test_module_and_the_declared_test_module_are_found() {
        let facts = scan_source(FIXTURE).unwrap();
        assert!(find(&facts, "only_in_tests").in_test);
        assert!(!find(&facts, "plain").in_test);
        let inline_line = FIXTURE
            .lines()
            .position(|line| line == "mod tests {")
            .unwrap();
        assert_eq!(facts.first_test_line, Some(inline_line));
        let declared: Vec<_> = facts
            .mods
            .iter()
            .map(|decl| (decl.name.as_str(), decl.in_test))
            .collect();
        assert_eq!(declared, [("declared", true)]);
        assert!(facts.test_idents.contains("plain"));
        assert!(!facts.other_idents.contains("assert_eq"));
    }

    #[test]
    fn calls_inside_macro_bodies_are_seen_and_unparsable_bodies_are_listed() {
        let source = "fn f(vault: V) {\n    let a = format!(\"{}\", vault.in_format());\n    assert!(vault.in_assert(), \"m\");\n    let b = vec![vault.in_vec(), 1];\n    odd!(=> vault.hidden());\n}\n";
        let facts = scan_source(source).unwrap();
        for name in ["in_format", "in_assert", "in_vec"] {
            assert!(
                facts
                    .calls
                    .iter()
                    .any(|call| call.name == name && call.on_vault),
                "{name}"
            );
        }
        assert!(facts.calls.iter().all(|call| call.name != "hidden"));
        assert_eq!(facts.unscanned, [5]);
    }

    #[test]
    fn tauri_commands_list_the_methods_they_call_in_order() {
        let source = r"
            #[tauri::command]
            fn show(vault: Vault) { vault.first().second(vault.third()); }
            fn plain(vault: Vault) { vault.skipped(); }
        ";
        let facts = scan_source(source).unwrap();
        assert_eq!(facts.commands, [["first", "second", "third"]]);
    }

    #[test]
    fn an_ipc_attribute_marks_a_command_like_tauri_command() {
        let source = r"
            #[ipc(write)]
            fn save(vault: Vault) { vault.persisted(); }
            #[crate::ipc]
            fn read(vault: Vault) { vault.peek(); }
            #[other(write)]
            fn plain(vault: Vault) { vault.skipped(); }
        ";
        let facts = scan_source(source).unwrap();
        assert_eq!(facts.commands, [["persisted"], ["peek"]]);
    }

    #[test]
    fn test_files_are_found_by_declaration_name_and_inheritance() {
        let corpus = Corpus::from_sources(&[
            ("svc/mod.rs", "#[cfg(test)] mod golden;\nmod live;"),
            ("svc/golden.rs", "mod nested;"),
            ("svc/golden/nested.rs", "fn deep() {}"),
            ("svc/live.rs", "fn prod() {}"),
            ("svc/live_tests.rs", "fn named() {}"),
            ("svc/peer/mod.rs", "#[cfg(test)] mod cases;"),
            ("svc/peer/cases.rs", "fn one() {}"),
        ])
        .unwrap();
        let tests: Vec<&str> = [
            "svc/mod.rs",
            "svc/golden.rs",
            "svc/golden/nested.rs",
            "svc/live.rs",
            "svc/live_tests.rs",
            "svc/peer/cases.rs",
        ]
        .into_iter()
        .filter(|path| corpus.get(path).unwrap().is_test)
        .collect();
        assert_eq!(
            tests,
            [
                "svc/golden.rs",
                "svc/golden/nested.rs",
                "svc/live_tests.rs",
                "svc/peer/cases.rs"
            ]
        );
    }
}
