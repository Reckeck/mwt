//! Source surgery on top of `syn` spans: text splices at byte offsets, whole-line spans of items,
//! the methods of `impl UnlockedVault`, and the one rule that makes an item `pub(crate)`.
//! Shared by `widen` and `carve`.

use proc_macro2::{LineColumn, Span};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{ImplItemFn, Item, ItemImpl, Signature, Visibility};

use super::scan::{VAULT_TYPE, names_type};

/// Replaces `start..end` of a text with `text` (an insertion when the range is empty).
pub struct Splice {
    pub start: usize,
    pub end: usize,
    pub text: String,
}

/// Applies non-overlapping splices; the order they come in does not matter.
pub fn apply(source: &str, mut splices: Vec<Splice>) -> String {
    splices.sort_by_key(|splice| std::cmp::Reverse(splice.start));
    let mut result = source.to_string();
    for splice in splices {
        result.replace_range(splice.start..splice.end, &splice.text);
    }
    result
}

/// Byte offsets of the line starts of one text, to turn a span position into an offset.
pub struct LineIndex<'text> {
    text: &'text str,
    starts: Vec<usize>,
}

impl<'text> LineIndex<'text> {
    pub fn new(text: &'text str) -> Self {
        let breaks = text.match_indices('\n').map(|(index, _)| index + 1);
        Self {
            text,
            starts: std::iter::once(0).chain(breaks).collect(),
        }
    }

    pub fn offset(&self, at: LineColumn) -> usize {
        let start = self.starts[at.line - 1];
        let rest = &self.text[start..];
        let line = &rest[..rest.find('\n').unwrap_or(rest.len())];
        let column = line
            .char_indices()
            .map(|(index, _)| index)
            .chain(std::iter::once(line.len()))
            .nth(at.column)
            .unwrap_or(line.len());
        start + column
    }
}

/// First and last line (1-based, inclusive) of a node, attributes and docs included.
pub fn line_range(node: &impl Spanned) -> (usize, usize) {
    let span = node.span();
    (span.start().line, span.end().line)
}

pub fn is_vault_impl(node: &ItemImpl) -> bool {
    node.trait_.is_none() && names_type(&node.self_ty, VAULT_TYPE)
}

/// The methods of one impl block, when it is an inherent `impl UnlockedVault`.
pub fn vault_methods_of(node: &ItemImpl) -> impl Iterator<Item = &ImplItemFn> {
    node.items
        .iter()
        .filter_map(|item| match item {
            syn::ImplItem::Fn(method) => Some(method),
            _ => None,
        })
        .filter(|_| is_vault_impl(node))
}

/// Every method of every inherent `impl UnlockedVault` of the file, nested modules included.
pub fn vault_methods(file: &syn::File) -> Vec<&ImplItemFn> {
    #[derive(Default)]
    struct Collector<'ast> {
        found: Vec<&'ast ImplItemFn>,
    }
    impl<'ast> Visit<'ast> for Collector<'ast> {
        fn visit_item_impl(&mut self, node: &'ast ItemImpl) {
            self.found.extend(vault_methods_of(node));
            visit::visit_item_impl(self, node);
        }
    }
    let mut collector = Collector::default();
    collector.visit_file(file);
    collector.found
}

/// The splice that makes an item `pub(crate)`: `None` when it is `pub` or `pub(crate)` already.
/// `keyword` is where the item proper starts (after the visibility, e.g. `fn`, `struct`).
pub fn widen(index: &LineIndex, visibility: &Visibility, keyword: LineColumn) -> Option<Splice> {
    match visibility {
        Visibility::Public(_) => None,
        Visibility::Restricted(restricted) if restricted.path.is_ident("crate") => None,
        Visibility::Restricted(restricted) => Some(Splice {
            start: index.offset(restricted.pub_token.span.start()),
            end: index.offset(restricted.paren_token.span.close().end()),
            text: "pub(crate)".into(),
        }),
        Visibility::Inherited => {
            let at = index.offset(keyword);
            Some(Splice {
                start: at,
                end: at,
                text: "pub(crate) ".into(),
            })
        }
    }
}

/// Where the item proper of a function starts: its first qualifier or `fn`.
pub fn signature_start(sig: &Signature) -> LineColumn {
    let spans = [
        sig.constness.map(|token| token.span),
        sig.asyncness.map(|token| token.span),
        sig.unsafety.map(|token| token.span),
        sig.abi.as_ref().map(|abi| abi.extern_token.span),
        Some(sig.fn_token.span),
    ];
    spans
        .into_iter()
        .flatten()
        .map(|span: Span| span.start())
        .min_by_key(|at| (at.line, at.column))
        .expect("`fn` is always there")
}

/// A top-level item that can be carved: its name, visibility and where its keyword starts.
pub struct Named<'ast> {
    pub name: String,
    pub visibility: &'ast Visibility,
    pub keyword: LineColumn,
}

pub fn named(item: &Item) -> Option<Named<'_>> {
    let (ident, visibility, keyword) = match item {
        Item::Fn(node) => (&node.sig.ident, &node.vis, signature_start(&node.sig)),
        Item::Struct(node) => (&node.ident, &node.vis, node.struct_token.span.start()),
        Item::Enum(node) => (&node.ident, &node.vis, node.enum_token.span.start()),
        Item::Union(node) => (&node.ident, &node.vis, node.union_token.span.start()),
        Item::Type(node) => (&node.ident, &node.vis, node.type_token.span.start()),
        Item::Const(node) => (&node.ident, &node.vis, node.const_token.span.start()),
        Item::Static(node) => (&node.ident, &node.vis, node.static_token.span.start()),
        Item::Trait(node) => (&node.ident, &node.vis, node.trait_token.span.start()),
        _ => return None,
    };
    Some(Named {
        name: ident.to_string(),
        visibility,
        keyword,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn widened(source: &str) -> String {
        let file = syn::parse_file(source).unwrap();
        let index = LineIndex::new(source);
        let splices = vault_methods(&file)
            .into_iter()
            .filter_map(|method| widen(&index, &method.vis, signature_start(&method.sig)))
            .collect();
        apply(source, splices)
    }

    #[test]
    fn qualifiers_and_restricted_visibilities_are_handled() {
        let source = "impl UnlockedVault {\n    async fn a(&self) {}\n    const fn b() {}\n    pub(super) fn c(&self) {}\n    pub(in crate::x) fn d(&self) {}\n    pub(crate) fn e(&self) {}\n    pub fn f(&self) {}\n    /// Doc.\n    #[inline]\n    unsafe fn g(&self) {}\n}\nimpl Other { fn h() {} }\n";
        assert_eq!(
            widened(source),
            "impl UnlockedVault {\n    pub(crate) async fn a(&self) {}\n    pub(crate) const fn b() {}\n    pub(crate) fn c(&self) {}\n    pub(crate) fn d(&self) {}\n    pub(crate) fn e(&self) {}\n    pub fn f(&self) {}\n    /// Doc.\n    #[inline]\n    pub(crate) unsafe fn g(&self) {}\n}\nimpl Other { fn h() {} }\n"
        );
    }

    #[test]
    fn columns_count_characters_not_bytes() {
        assert_eq!(
            widened("impl UnlockedVault { fn \u{e9}(&self) {} fn b(&self) {} }\n"),
            "impl UnlockedVault { pub(crate) fn \u{e9}(&self) {} pub(crate) fn b(&self) {} }\n"
        );
    }
}
