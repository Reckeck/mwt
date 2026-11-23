//! Finds the `service::` paths of one file and the text that replaces each, by the shim map: in use
//! trees (one grouped `use` line per module afterwards) and in every other path (types, expressions, patterns,
//! macro arguments). A path whose name has no safe owner stays and is listed.

use std::collections::{HashMap, HashSet};

use proc_macro2::LineColumn;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{File, Item, ItemMod, ItemUse, Macro, Path};

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
    /// The inline modules around the item being visited.
    modules: Vec<String>,
    uses: Vec<UseItem>,
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
            modules: Vec::new(),
            uses: Vec::new(),
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

    /// The shim name of a path and where it ends: the segments after `service` go down into the shim
    /// modules (`service::ids::x_id`) and the first one that is no module is the name.
    fn locate(&self, segments: &[String]) -> Option<(usize, Resolution)> {
        let mut end = self.name_at(segments)?;
        let mut module: Vec<String> = Vec::new();
        while end + 1 < segments.len() && self.shims.has_module(&module, &segments[end]) {
            module.push(segments[end].clone());
            end += 1;
        }
        Some((end, self.shims.resolve_in(&module, &segments[end])))
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
        let Some((at, resolution)) = self.locate(&segments) else {
            return;
        };
        let line = node.span().start().line;
        let written = segments[..=at].join("::");
        match resolution {
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

    /// What precedes `use` on every new line after the first: the attributes except doc comments, and
    /// the visibility.
    fn repeated_head(&self, node: &ItemUse) -> String {
        let mut head = String::new();
        for attribute in node
            .attrs
            .iter()
            .filter(|attr| !attr.path().is_ident("doc"))
        {
            head.push_str(self.text_between(attribute.span().start(), attribute.span().end()));
            head.push('\n');
        }
        let after_attributes = node
            .attrs
            .last()
            .map_or(node.span().start(), |attribute| attribute.span().end());
        let visibility = self.text_between(after_attributes, node.use_token.span.start());
        head.push_str(visibility.trim_start());
        head
    }

    /// Records one `use` item with a text per leaf; `finish` turns the items into splices.
    fn rewrite_use(&mut self, node: &ItemUse, in_module: bool) {
        let mut entries = Vec::new();
        let mut changed = false;
        let line = node.span().start().line;
        for leaf in &leaves(&node.tree) {
            let written = leaf_text(leaf, &leaf.path);
            match self.locate(&leaf.path) {
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
                    entries.push(Entry::new(text, leaf.glob));
                    changed = true;
                }
                Some((at, Err(skip))) => {
                    self.refuse(line, &leaf.path[..=at].join("::"), &leaf.path[at], &skip);
                    entries.push(Entry::new(written, leaf.glob));
                }
                None => entries.push(Entry::new(written, leaf.glob)),
            }
        }
        let start = node.span().start();
        let first_head = self.text_between(start, node.use_token.span.start());
        let mergeable = in_module && node.attrs.is_empty();
        let scope = if mergeable {
            format!("{}:{}", self.modules.join("::"), first_head.trim())
        } else {
            format!("item:{}", self.uses.len())
        };
        self.uses.push(UseItem {
            start: self.index.offset(start),
            end: self.index.offset(node.span().end()),
            first_head: first_head.to_string(),
            repeated_head: self.repeated_head(node),
            scope,
            changed,
            dirty: changed,
            entries,
        });
    }

    /// The range widened to the whole lines it stands on, when only blanks share them.
    fn whole_lines(&self, start: usize, end: usize) -> (usize, usize) {
        let before = &self.source[..start];
        let line_start = before.trim_end_matches([' ', '\t']).len();
        let at_line_start = line_start == 0 || before[..line_start].ends_with('\n');
        let after = &self.source[end..];
        match after.strip_prefix('\n') {
            Some(_) if at_line_start => (line_start, end + 1),
            _ => (start, end),
        }
    }

    /// Turns the collected `use` items into splices. A leaf that goes to a module another `use` item
    /// of the file (same visibility, no attributes, top level) already names joins the first such
    /// item; the leaves of one module in one item form one grouped `use` line. Items nothing touched
    /// stay as written.
    fn finish(&mut self) {
        let mut uses = std::mem::take(&mut self.uses);
        let mut active = HashSet::new();
        let mut owners: HashMap<(String, String), usize> = HashMap::new();
        for (number, item) in uses.iter().enumerate() {
            for key in item.keys() {
                if item.changed {
                    active.insert(key.clone());
                }
                owners.entry(key).or_insert(number);
            }
        }
        let mut moved: Vec<(usize, Entry)> = Vec::new();
        for (number, item) in uses.iter_mut().enumerate() {
            let scope = item.scope.clone();
            let mut kept = Vec::new();
            for entry in std::mem::take(&mut item.entries) {
                let key = entry.module.clone().map(|module| (scope.clone(), module));
                match key {
                    Some(key) if active.contains(&key) && owners[&key] != number => {
                        moved.push((owners[&key], entry));
                        item.dirty = true;
                    }
                    _ => kept.push(entry),
                }
            }
            item.entries = kept;
        }
        for (owner, entry) in moved {
            let item = &mut uses[owner];
            if !item.entries.iter().any(|have| have.whole == entry.whole) {
                item.entries.push(entry);
            }
            item.dirty = true;
        }
        for item in uses.iter().filter(|item| item.dirty) {
            let text = item.text();
            let (start, end) = if text.is_empty() {
                self.whole_lines(item.start, item.end)
            } else {
                (item.start, item.end)
            };
            self.report.splices.push(Splice { start, end, text });
        }
    }
}

/// One leaf of a `use` item as text; `module` is what precedes the last segment (none for a glob).
struct Entry {
    module: Option<String>,
    /// The whole leaf: `a::b::Name as Alias`.
    whole: String,
    /// What follows the module: `Name as Alias`.
    name: String,
}

impl Entry {
    fn new(whole: String, glob: bool) -> Self {
        let split = if glob { None } else { whole.rsplit_once("::") };
        Self {
            module: split.map(|(module, _)| module.to_string()),
            name: split.map_or_else(String::new, |(_, name)| name.to_string()),
            whole,
        }
    }
}

/// A `use` item of the file, with the text of its leaves.
struct UseItem {
    start: usize,
    end: usize,
    /// What precedes `use` on the first line: docs, attributes, visibility.
    first_head: String,
    /// What precedes `use` on every further line.
    repeated_head: String,
    /// Leaves may move between items with the same scope: top level, no attributes, same visibility;
    /// any other item is a scope of its own.
    scope: String,
    changed: bool,
    /// The text of the item differs from the source.
    dirty: bool,
    entries: Vec<Entry>,
}

impl UseItem {
    fn keys(&self) -> impl Iterator<Item = (String, String)> + '_ {
        self.entries
            .iter()
            .filter_map(|entry| entry.module.clone())
            .map(|module| (self.scope.clone(), module))
    }

    /// One `use` line per module, the leaves of a module grouped in braces; empty when no leaf is left.
    fn text(&self) -> String {
        let mut done: HashSet<&str> = HashSet::new();
        let mut lines = Vec::new();
        for entry in &self.entries {
            let Some(module) = entry.module.as_deref() else {
                lines.push(entry.whole.clone());
                continue;
            };
            if !done.insert(module) {
                continue;
            }
            let names: Vec<&str> = self
                .entries
                .iter()
                .filter(|other| other.module.as_deref() == Some(module))
                .map(|other| other.name.as_str())
                .collect();
            lines.push(match names.as_slice() {
                [single] => format!("{module}::{single}"),
                many => format!("{module}::{{{}}}", many.join(", ")),
            });
        }
        let body: Vec<String> = lines
            .iter()
            .enumerate()
            .map(|(number, text)| {
                let head = if number == 0 {
                    &self.first_head
                } else {
                    &self.repeated_head
                };
                format!("{head}use {text};")
            })
            .collect();
        body.join("\n")
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

impl Rewriter<'_> {
    /// The items of a file or an inline module: their `use` items share a scope.
    fn visit_module_items(&mut self, items: &[Item]) {
        for item in items {
            match item {
                Item::Use(node) => self.rewrite_use(node, true),
                other => self.visit_item(other),
            }
        }
    }
}

impl<'ast> Visit<'ast> for Rewriter<'_> {
    fn visit_file(&mut self, node: &'ast File) {
        for attribute in &node.attrs {
            self.visit_attribute(attribute);
        }
        self.visit_module_items(&node.items);
        self.finish();
    }

    fn visit_item_use(&mut self, node: &'ast ItemUse) {
        self.rewrite_use(node, false);
    }

    fn visit_path(&mut self, node: &'ast Path) {
        self.rewrite_path(node);
        visit::visit_path(self, node);
    }

    fn visit_item_mod(&mut self, node: &'ast ItemMod) {
        let inline = usize::from(node.content.is_some());
        self.inline_depth += inline;
        for attribute in &node.attrs {
            self.visit_attribute(attribute);
        }
        if let Some((_, items)) = &node.content {
            self.modules.push(node.ident.to_string());
            self.visit_module_items(items);
            self.modules.pop();
        }
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
