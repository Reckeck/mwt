//! `strangle rewire <area> <files>`: best effort. Replaces a call `x.name(args)` of a moved method
//! by the call of its new function, cut by text span, then runs `rustfmt` on the changed files
//! (spec D3, Interfaces). Only a call on a receiver proven to be an `UnlockedVault` is touched:
//! `self` inside `impl UnlockedVault`, or a parameter typed `&UnlockedVault` / `&mut UnlockedVault`.
//! Every other call of a moved name is skipped and listed. A moved call inside the arguments of a
//! rewritten one stays for the next run.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};
use clap::Args;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{Expr, ExprMethodCall, FnArg, ImplItemFn, ItemFn, ItemImpl, Macro, Pat, Type};

use super::baseline::Baseline;
use super::fileset::FileSet;
use super::forward::delegate_target;
use super::home::{DOC_MARKER, core_dir, homes};
use super::map::Map;
use super::scan::{macro_arguments, macro_line};
use super::source::{LineIndex, Splice, apply, is_vault_impl};

#[derive(Args)]
pub struct RewireArgs {
    /// One area of `strangle.toml`.
    pub area: String,
    /// Source files to rewrite, relative to the workspace root.
    #[arg(required = true)]
    pub files: Vec<String>,
}

pub fn run(root: &Path, args: &RewireArgs) -> Result<()> {
    let map = Map::load(root)?;
    let baseline = Baseline::load(root)?;
    let mut files = FileSet::new(root);
    let report = rewire(root, &mut files, &map, &baseline, args)?;
    for line in &report.skipped {
        println!("rewire: skipped {line}");
    }
    println!("rewire: {} call(s) replaced", report.replaced);
    let changed: Vec<String> = files.changed().iter().map(ToString::to_string).collect();
    files.write()?;
    format(root, &changed)
}

#[derive(Default)]
struct Report {
    replaced: usize,
    skipped: Vec<String>,
}

/// A moved method: where its new function lives and what its first argument is.
struct Target {
    /// `crate::<module>::<name>`.
    path: String,
    /// `@books`: the function takes `&recv.books()`; `@doc`: not rewritten, listed as skipped;
    /// none: it takes the vault itself.
    marker: Option<&'static str>,
}

fn rewire(
    root: &Path,
    files: &mut FileSet,
    map: &Map,
    baseline: &Baseline,
    args: &RewireArgs,
) -> Result<Report> {
    let area = &map.areas[map.area_index(&args.area)?];
    let targets = targets_of(files, map, area, baseline)?;
    let mut report = Report::default();
    for path in &args.files {
        let source = files.read(path)?.to_string();
        if source.is_empty() {
            bail!("{path}: no such file");
        }
        let prefix = crate_prefix(root, map, path)?;
        let file = syn::parse_file(&source).with_context(|| format!("parse {path}"))?;
        let mut rewriter = Rewriter::new(&source, &targets, &prefix);
        rewriter.visit_file(&file);
        report.replaced += rewriter.splices.len();
        report
            .skipped
            .extend(rewriter.skipped.iter().map(|skip| format!("{path}:{skip}")));
        let updated = apply(&source, rewriter.splices);
        files.set(path, updated)?;
    }
    Ok(report)
}

/// The moved methods of the area by name, found in the pure and scenario files of its stems.
fn targets_of(
    files: &mut FileSet,
    map: &Map,
    area: &super::map::Area,
    baseline: &Baseline,
) -> Result<BTreeMap<String, Target>> {
    let methods = &baseline.area(&area.name)?.methods;
    let mut targets = BTreeMap::new();
    for home in homes(map, area) {
        for (path, module) in [
            (home.pure_file(), home.pure_path()),
            (home.scenario_file(), home.scenario_path()),
        ] {
            let source = files.read(&path)?.to_string();
            let file = syn::parse_file(&source).with_context(|| format!("parse {path}"))?;
            for node in file.items.iter().filter_map(fn_item) {
                let Some(receiver) = delegate_target(node, methods) else {
                    continue;
                };
                let name = node.sig.ident.to_string();
                targets.entry(name.clone()).or_insert(Target {
                    path: format!("{module}::{name}"),
                    marker: receiver.marker,
                });
            }
        }
    }
    Ok(targets)
}

fn fn_item(item: &syn::Item) -> Option<&ItemFn> {
    match item {
        syn::Item::Fn(node) => Some(node),
        _ => None,
    }
}

/// How the new function is named from a file: `crate` inside the core crate, else the crate's name.
fn crate_prefix(root: &Path, map: &Map, file: &str) -> Result<String> {
    let core = core_dir(map);
    if file.starts_with(&format!("{core}/")) {
        return Ok("crate".into());
    }
    let manifest = root.join(&core).join("../Cargo.toml");
    let text = std::fs::read_to_string(&manifest)
        .with_context(|| format!("read {}", manifest.display()))?;
    let table: toml::Table = text.parse().context("parse the core Cargo.toml")?;
    let name = table["package"]["name"]
        .as_str()
        .context("the core Cargo.toml has no package name")?;
    Ok(name.replace('-', "_"))
}

/// Finds the calls to rewrite, remembering which receivers are provably an `UnlockedVault`.
struct Rewriter<'run> {
    source: &'run str,
    index: LineIndex<'run>,
    targets: &'run BTreeMap<String, Target>,
    prefix: &'run str,
    in_vault_impl: bool,
    /// Parameters of the enclosing functions typed `&UnlockedVault` / `&mut UnlockedVault`.
    vault_params: Vec<String>,
    splices: Vec<Splice>,
    /// `line: name(): reason`.
    skipped: Vec<String>,
}

impl<'run> Rewriter<'run> {
    fn new(source: &'run str, targets: &'run BTreeMap<String, Target>, prefix: &'run str) -> Self {
        Self {
            source,
            index: LineIndex::new(source),
            targets,
            prefix,
            in_vault_impl: false,
            vault_params: Vec::new(),
            splices: Vec::new(),
            skipped: Vec::new(),
        }
    }

    fn is_vault_receiver(&self, receiver: &Expr) -> bool {
        let Expr::Path(path) = receiver else {
            return false;
        };
        let Some(ident) = path.path.get_ident() else {
            return false;
        };
        (ident == "self" && self.in_vault_impl)
            || self.vault_params.iter().any(|param| ident == param)
    }

    fn text_of(&self, node: &impl Spanned) -> &'run str {
        let span = node.span();
        &self.source[self.index.offset(span.start())..self.index.offset(span.end())]
    }

    fn replacement(&self, target: &Target, node: &ExprMethodCall) -> String {
        let receiver = self.text_of(&*node.receiver);
        let first = match target.marker {
            Some(_) => format!("&{receiver}.books()"),
            None => receiver.to_string(),
        };
        let module_path = target.path.replacen("crate", self.prefix, 1);
        let rest: Vec<&str> = node.args.iter().map(|arg| self.text_of(arg)).collect();
        let arguments = std::iter::once(first.as_str())
            .chain(rest)
            .collect::<Vec<_>>();
        format!("{module_path}({})", arguments.join(", "))
    }

    fn with_params(
        &mut self,
        inputs: &syn::punctuated::Punctuated<FnArg, syn::Token![,]>,
        body: impl FnOnce(&mut Self),
    ) {
        let before = self.vault_params.len();
        self.vault_params
            .extend(inputs.iter().filter_map(vault_param));
        body(self);
        self.vault_params.truncate(before);
    }
}

/// The name of a parameter typed `&UnlockedVault` or `&mut UnlockedVault`.
fn vault_param(arg: &FnArg) -> Option<String> {
    let FnArg::Typed(typed) = arg else {
        return None;
    };
    let (Pat::Ident(name), Type::Reference(reference)) = (&*typed.pat, &*typed.ty) else {
        return None;
    };
    super::scan::names_type(&reference.elem, super::scan::VAULT_TYPE)
        .then(|| name.ident.to_string())
}

impl<'ast> Visit<'ast> for Rewriter<'_> {
    fn visit_item_impl(&mut self, node: &'ast ItemImpl) {
        let outer = std::mem::replace(&mut self.in_vault_impl, is_vault_impl(node));
        visit::visit_item_impl(self, node);
        self.in_vault_impl = outer;
    }

    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        self.with_params(&node.sig.inputs, |this| visit::visit_item_fn(this, node));
    }

    fn visit_impl_item_fn(&mut self, node: &'ast ImplItemFn) {
        self.with_params(&node.sig.inputs, |this| {
            visit::visit_impl_item_fn(this, node)
        });
    }

    fn visit_macro(&mut self, node: &'ast Macro) {
        match macro_arguments(node) {
            Some(arguments) => {
                for argument in &arguments {
                    self.visit_expr(argument);
                }
            }
            None => self.skipped.push(format!(
                "{}: macro body is not a list of expressions: calls inside are not scanned",
                macro_line(node)
            )),
        }
    }

    fn visit_expr_method_call(&mut self, node: &'ast ExprMethodCall) {
        let name = node.method.to_string();
        let Some(target) = self.targets.get(&name) else {
            return visit::visit_expr_method_call(self, node);
        };
        let line = node.method.span().start().line;
        if target.marker == Some(DOC_MARKER) {
            self.skipped.push(format!(
                "{line}: {name}(): `@doc` target is not rewritten (no `&VaultDoc` accessor)"
            ));
            visit::visit_expr_method_call(self, node);
        } else if self.is_vault_receiver(&node.receiver) {
            let span = node.span();
            self.splices.push(Splice {
                start: self.index.offset(span.start()),
                end: self.index.offset(span.end()),
                text: self.replacement(target, node),
            });
            // The arguments are copied as written; a moved call inside them waits for the next run.
        } else {
            self.skipped.push(format!(
                "{line}: {name}(): receiver is not provably an UnlockedVault"
            ));
            visit::visit_expr_method_call(self, node);
        }
    }
}

/// Runs `rustfmt` on the files that changed.
fn format(root: &Path, paths: &[String]) -> Result<()> {
    if paths.is_empty() {
        return Ok(());
    }
    let status = Command::new("rustfmt")
        .args(["--edition", "2024"])
        .args(paths)
        .current_dir(root)
        .status()
        .context("run rustfmt")?;
    if !status.success() {
        bail!("rustfmt failed on {}", paths.join(", "));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strangle::area_fixture::moved_tree;
    use crate::strangle::area_fixture::original_tree;

    const USER: &str = "crates/core/src/user.rs";

    const USER_SOURCE: &str = "fn command(vault: &mut UnlockedVault, other: Thing) {
    vault.set_info(\"a\",   info);
    let flag = vault.clash(\"a\");
    other.set_info(\"b\", info);
}
impl UnlockedVault {
    fn inside(&mut self) { self.set_info(\"c\", info); }
}
impl Other {
    fn outside(&self) { self.set_info(\"d\", info); }
}
";

    fn fixture() -> crate::strangle::fixture::TempTree {
        let original = original_tree();
        let workspace = crate::strangle::Workspace::load(original.root()).unwrap();
        let baseline = crate::strangle::baseline::capture(original.root(), &workspace).unwrap();
        let tree = moved_tree(|_, text| text);
        baseline.write(tree.root()).unwrap();
        std::fs::write(tree.root().join(USER), USER_SOURCE).unwrap();
        tree
    }

    #[test]
    fn proven_calls_are_replaced_and_formatted_and_the_rest_are_listed() {
        let tree = fixture();
        let args = RewireArgs {
            area: "loyalty".into(),
            files: vec![USER.into()],
        };
        run(tree.root(), &args).unwrap();
        let text = std::fs::read_to_string(tree.root().join(USER)).unwrap();
        assert!(
            text.contains("crate::usecases::household::loyalty::set_info(vault, \"a\", info);"),
            "{text}"
        );
        assert!(
            text.contains("crate::household::loyalty::clash(&vault.books(), \"a\")"),
            "{text}"
        );
        assert!(
            text.contains("crate::usecases::household::loyalty::set_info(self, \"c\", info)"),
            "{text}"
        );
        assert!(text.contains("other.set_info(\"b\", info);"), "{text}");
        assert!(text.contains("self.set_info(\"d\", info);"), "{text}");
    }

    #[test]
    fn a_doc_target_is_listed_as_skipped_not_rewritten() {
        let original = original_tree();
        let workspace = crate::strangle::Workspace::load(original.root()).unwrap();
        let mut baseline = crate::strangle::baseline::capture(original.root(), &workspace).unwrap();
        baseline.areas.get_mut("loyalty").unwrap().methods.push(
            crate::strangle::baseline::BaselineMethod {
                name: "peek".into(),
                signature: String::new(),
            },
        );
        let tree = moved_tree(|path, text| {
            if path.ends_with("household/loyalty.rs") {
                format!("{text}\npub fn peek(doc: &VaultDoc) -> usize {{ 0 }}\n")
            } else {
                text
            }
        });
        let source = "fn command(vault: &UnlockedVault) {\n    vault.peek();\n}\n";
        std::fs::write(tree.root().join(USER), source).unwrap();
        let map = Map::load(tree.root()).unwrap();
        let mut files = FileSet::new(tree.root());
        let args = RewireArgs {
            area: "loyalty".into(),
            files: vec![USER.into()],
        };
        let report = rewire(tree.root(), &mut files, &map, &baseline, &args).unwrap();
        assert_eq!(report.replaced, 0);
        assert_eq!(report.skipped.len(), 1, "{:?}", report.skipped);
        assert!(report.skipped[0].contains("peek(): `@doc` target is not rewritten"));
    }

    #[test]
    fn calls_inside_macro_bodies_are_rewired_and_unparsable_bodies_are_listed() {
        let tree = fixture();
        let source = "fn command(vault: &mut UnlockedVault) {
    let text = format!(\"{}\", vault.clash(\"a\"));
    assert!(vault.clash(\"b\"), \"m\");
    let list = vec![vault.clash(\"c\")];
    odd!(=> vault.clash(\"d\"));
}
";
        std::fs::write(tree.root().join(USER), source).unwrap();
        let map = Map::load(tree.root()).unwrap();
        let baseline = Baseline::load(tree.root()).unwrap();
        let mut files = FileSet::new(tree.root());
        let args = RewireArgs {
            area: "loyalty".into(),
            files: vec![USER.into()],
        };
        let report = rewire(tree.root(), &mut files, &map, &baseline, &args).unwrap();
        assert_eq!(report.replaced, 3);
        let text = files.read(USER).unwrap();
        assert!(text.contains("clash(&vault.books(), \"a\")"), "{text}");
        assert!(text.contains("vault.clash(\"d\")"), "{text}");
        assert_eq!(report.skipped.len(), 1, "{:?}", report.skipped);
        assert!(report.skipped[0].starts_with(&format!("{USER}:5: macro body")));
    }

    #[test]
    fn unproven_receivers_are_reported_as_skipped() {
        let tree = fixture();
        let map = Map::load(tree.root()).unwrap();
        let baseline = Baseline::load(tree.root()).unwrap();
        let mut files = FileSet::new(tree.root());
        let args = RewireArgs {
            area: "loyalty".into(),
            files: vec![USER.into()],
        };
        let report = rewire(tree.root(), &mut files, &map, &baseline, &args).unwrap();
        assert_eq!(report.replaced, 3);
        assert_eq!(report.skipped.len(), 2, "{:?}", report.skipped);
        assert!(report.skipped[0].starts_with(&format!("{USER}:4: set_info()")));
        assert!(report.skipped[1].starts_with(&format!("{USER}:10: set_info()")));
    }
}
