//! Finds the `service::` paths of one file and the text that replaces each, by the shim map: in use
//! trees (one `use` line per name afterwards) and in every other path (types, expressions, patterns,
//! macro arguments). A path whose name has no safe owner stays and is listed.

use proc_macro2::LineColumn;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{ItemMod, ItemUse, Macro, Path};

use super::shim::{Leaf, Resolution, ShimMap, leaves};
use crate::strangle::scan::macro_arguments;
use crate::strangle::source::{LineIndex, Splice};

/// The crate name of the core crate for a file outside it.
pub const CORE_CRATE: &str = "trade_core";

/// Where a file stands, which decides what its paths mean.
pub struct FileContext {
    /// The file belongs to the core crate (`crate::service` is the shim dir).
    pub in_core: bool,
    /// Module depth below `service/` (`foo.rs` is 1, `a/b.rs` is 2, `mod.rs` is 0); `None` outside it.
    pub service_depth: Option<usize>,
}

#[derive(Default)]
pub struct FileReport {
    pub splices: Vec<Splice>,
    /// `line: before -> after`, one per rewritten path or use name.
    pub rewrites: Vec<String>,
    /// `line: path: reason`.
    pub skipped: Vec<String>,
}

pub struct Rewriter<'run> {
    source: &'run str,
    index: LineIndex<'run>,
    shims: &'run ShimMap,
    context: &'run FileContext,
    inline_depth: usize,
    pub report: FileReport,
}

impl<'run> Rewriter<'run> {
    pub fn new(source: &'run str, shims: &'run ShimMap, context: &'run FileContext) -> Self {
        Self {
            source,
            index: LineIndex::new(source),
            shims,
            context,
            inline_depth: 0,
            report: FileReport::default(),
        }
    }

    /// Where the shim name starts in `segments`, when they name a shim path.
    fn name_at(&self, segments: &[String]) -> Option<usize> {
        let first = segments.first()?.as_str();
        let at = match first {
            "crate" if self.context.in_core && segments.get(1)? == "service" => 2,
            "trade_core" if segments.get(1)? == "service" => 2,
            "super" => {
                let depth = self.context.service_depth? + self.inline_depth;
                let supers = segments.iter().take_while(|part| *part == "super").count();
                if supers != depth {
                    return None;
                }
                supers
            }
            _ => return None,
        };
        (segments.len() > at).then_some(at)
    }

    /// The owner path as written from this file.
    fn owner_text(&self, owner: &[String]) -> String {
        let crate_name = if self.context.in_core {
            "crate"
        } else {
            CORE_CRATE
        };
        let mut parts: Vec<&str> = owner.iter().map(String::as_str).collect();
        if parts.first() == Some(&"crate") {
            parts[0] = crate_name;
        }
        parts.join("::")
    }

    fn resolve(&self, name: &str) -> Resolution {
        self.shims.resolve(name)
    }

    fn skip(&mut self, line: usize, path: &str, reason: &str) {
        self.report
            .skipped
            .push(format!("{line}: {path}: {reason}"));
    }

    fn text_between(&self, start: LineColumn, end: LineColumn) -> &'run str {
        &self.source[self.index.offset(start)..self.index.offset(end)]
    }

    /// Records a skip of `name` unless a `super::` name is not a shim name at all.
    fn refuse(&mut self, line: usize, written: &str, name: &str, skip: &super::shim::Skip) {
        let quiet = written.starts_with("super") && *skip == super::shim::Skip::Unknown;
        if !quiet && !name.is_empty() {
            self.skip(line, written, &skip.reason());
        }
    }

    fn rewrite_path(&mut self, node: &Path) {
        let segments: Vec<String> = node
            .segments
            .iter()
            .map(|part| part.ident.to_string())
            .collect();
        let Some(at) = self.name_at(&segments) else {
            return;
        };
        let line = node.span().start().line;
        let written = segments[..=at].join("::");
        match self.resolve(&segments[at]) {
            Ok(owner) => {
                let text = self.owner_text(&owner);
                let first = node.segments[0].ident.span();
                let last = node.segments[at].ident.span();
                self.report
                    .rewrites
                    .push(format!("{line}: {written} -> {text}"));
                self.report.splices.push(Splice {
                    start: self.index.offset(first.start()),
                    end: self.index.offset(last.end()),
                    text,
                });
            }
            Err(skip) => self.refuse(line, &written, &segments[at], &skip),
        }
    }

    /// One new `use` line (or the old text) per leaf of an item that names a shim path.
    fn rewrite_use(&mut self, node: &ItemUse) {
        let all = leaves(&node.tree);
        let mut lines = Vec::new();
        let mut changed = false;
        let line = node.span().start().line;
        for leaf in &all {
            let written = leaf_text(leaf, &leaf.path);
            let outcome = self
                .name_at(&leaf.path)
                .map(|at| (at, self.resolve(&leaf.path[at])));
            match outcome {
                Some((at, Ok(owner))) => {
                    let mut path = vec![self.owner_text(&owner)];
                    path.extend(leaf.path[at + 1..].iter().cloned());
                    let kept = (leaf.alias.is_none() && !leaf.glob && leaf.path.len() == at + 1)
                        .then(|| leaf.path[at].clone())
                        .filter(|name| owner.last() != Some(name));
                    let alias = leaf.alias.clone().or(kept);
                    let text = leaf_text(
                        &Leaf {
                            path: path.clone(),
                            alias,
                            glob: leaf.glob,
                        },
                        &path,
                    );
                    self.report
                        .rewrites
                        .push(format!("{line}: {written} -> {text}"));
                    lines.push(text);
                    changed = true;
                }
                Some((at, Err(skip))) => {
                    self.refuse(line, &leaf.path[..=at].join("::"), &leaf.path[at], &skip);
                    lines.push(written);
                }
                None => lines.push(written),
            }
        }
        if !changed {
            return;
        }
        let start = node.span().start();
        let head = self.text_between(start, node.use_token.span.start());
        let body: Vec<String> = lines
            .iter()
            .map(|text| format!("{head}use {text};"))
            .collect();
        self.report.splices.push(Splice {
            start: self.index.offset(start),
            end: self.index.offset(node.span().end()),
            text: body.join("\n"),
        });
    }
}

fn leaf_text(leaf: &Leaf, path: &[String]) -> String {
    let mut text = path.join("::");
    if leaf.glob {
        text.push_str("::*");
    }
    if let Some(alias) = &leaf.alias {
        text.push_str(" as ");
        text.push_str(alias);
    }
    text
}

impl<'ast> Visit<'ast> for Rewriter<'_> {
    fn visit_item_use(&mut self, node: &'ast ItemUse) {
        self.rewrite_use(node);
    }

    fn visit_path(&mut self, node: &'ast Path) {
        self.rewrite_path(node);
        visit::visit_path(self, node);
    }

    fn visit_item_mod(&mut self, node: &'ast ItemMod) {
        let inline = usize::from(node.content.is_some());
        self.inline_depth += inline;
        visit::visit_item_mod(self, node);
        self.inline_depth -= inline;
    }

    fn visit_macro(&mut self, node: &'ast Macro) {
        match macro_arguments(node) {
            Some(arguments) => {
                for argument in &arguments {
                    self.visit_expr(argument);
                }
            }
            None if node.tokens.to_string().contains("service") => {
                let line = node.path.span().start().line;
                self.skip(
                    line,
                    "macro body",
                    "not a list of expressions: service:: paths inside are not rewritten",
                );
            }
            None => {}
        }
    }
}
