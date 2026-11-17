//! `strangle relocate <area>`: moves each file of the area into its stub `<group>/<stem>.rs`,
//! rewrites the paths that pointed into `service/`, and leaves the shim: the `pub use` of the
//! public items and an empty `forward!` region for `strangle forward` (spec D1 OD2, D6 step 3).
//! The `impl UnlockedVault` block moves as it is.

use std::path::Path;

use anyhow::{Context, Result, bail};
use clap::Args;
use proc_macro2::{Delimiter, Spacing, TokenStream, TokenTree};
use syn::Visibility;

use super::fileset::FileSet;
use super::forward::{FORWARD_BEGIN, FORWARD_END};
use super::home::{Home, homes};
use super::map::Map;
use super::source::{self, LineIndex, Splice};

/// What a stub starts with, so a real file is never overwritten.
const PLACEHOLDER: &str = "//! placeholder";

#[derive(Args)]
pub struct RelocateArgs {
    /// One area of `strangle.toml`.
    pub area: String,
}

pub fn run(root: &Path, args: &RelocateArgs) -> Result<()> {
    let map = Map::load(root)?;
    let mut files = FileSet::new(root);
    for line in relocate(&mut files, &map, &args.area)? {
        println!("relocate: {line}");
    }
    files.write()
}

/// Moves every stem of the area; returns one report line per stem.
fn relocate(files: &mut FileSet, map: &Map, area: &str) -> Result<Vec<String>> {
    let area = &map.areas[map.area_index(area)?];
    let mut report = Vec::new();
    for home in homes(map, area) {
        let shim = home.shim();
        let source = files.read(&shim)?.to_string();
        if source.is_empty() {
            report.push(format!("{shim} is not a single file: skipped"));
        } else if source.contains(FORWARD_BEGIN) {
            report.push(format!("{shim} is a shim already: skipped"));
        } else {
            move_file(files, &home, &source)?;
            report.push(format!("{shim} -> {}", home.pure_file()));
        }
    }
    Ok(report)
}

fn move_file(files: &mut FileSet, home: &Home, source: &str) -> Result<()> {
    let target = home.pure_file();
    if !files.read(&target)?.starts_with(PLACEHOLDER) {
        bail!("{target} is not a placeholder stub: run `strangle scaffold` first");
    }
    let shim = shim_text(home, source)?;
    files.set(&target, rewrite_paths(source, home)?)?;
    files.set(&home.shim(), shim)
}

/// The shim: a doc line, the `pub use` of the public items, and the empty forward region.
fn shim_text(home: &Home, source: &str) -> Result<String> {
    let file = syn::parse_file(source).with_context(|| format!("parse {}", home.shim()))?;
    let public: Vec<String> = file
        .items
        .iter()
        .filter_map(source::named)
        .filter(|named| matches!(named.visibility, Visibility::Public(_)))
        .map(|named| named.name)
        .collect();
    let mut text = format!(
        "//! Shim of the `{}` area: moved to `{}` (spec core-strangler, D1).\n",
        home.stem,
        home.pure_path()
    );
    match public.as_slice() {
        [] => {}
        [only] => text += &format!("\npub use {}::{only};\n", home.pure_path()),
        many => text += &format!("\npub use {}::{{{}}};\n", home.pure_path(), many.join(", ")),
    }
    text += &format!("\n{FORWARD_BEGIN}\n{FORWARD_END}\n");
    Ok(text)
}

/// The source with `super::` and `crate::service::<stem>` paths rewritten for the new home.
fn rewrite_paths(source: &str, home: &Home) -> Result<String> {
    let tokens: TokenStream = source
        .parse()
        .map_err(|error| anyhow::anyhow!("tokenize {}: {error}", home.shim()))?;
    let mut rewriter = Rewriter {
        index: LineIndex::new(source),
        home,
        splices: Vec::new(),
    };
    rewriter.walk(&tokens.into_iter().collect::<Vec<_>>(), 0)?;
    Ok(source::apply(source, rewriter.splices))
}

struct Rewriter<'text> {
    index: LineIndex<'text>,
    home: &'text Home,
    splices: Vec<Splice>,
}

impl Rewriter<'_> {
    /// `depth` counts the inline `mod x { }` blocks around the tokens: a `super` that stays inside
    /// them points at the file's own module and is left alone.
    fn walk(&mut self, tokens: &[TokenTree], depth: usize) -> Result<()> {
        let mut at = 0;
        while at < tokens.len() {
            at = match &tokens[at] {
                TokenTree::Ident(ident) if ident == "mod" => {
                    self.inline_module(tokens, at, depth)?
                }
                TokenTree::Ident(ident) if ident == "super" => self.super_run(tokens, at, depth)?,
                TokenTree::Ident(ident) if ident == "crate" => self.crate_path(tokens, at),
                TokenTree::Group(group) => {
                    self.walk(&group.stream().into_iter().collect::<Vec<_>>(), depth)?;
                    at + 1
                }
                _ => at + 1,
            };
        }
        Ok(())
    }

    /// `mod name { .. }`: the body is one level deeper. Returns the next position.
    fn inline_module(&mut self, tokens: &[TokenTree], at: usize, depth: usize) -> Result<usize> {
        if let (Some(TokenTree::Ident(_)), Some(TokenTree::Group(body))) =
            (tokens.get(at + 1), tokens.get(at + 2))
            && body.delimiter() == Delimiter::Brace
        {
            self.walk(&body.stream().into_iter().collect::<Vec<_>>(), depth + 1)?;
            return Ok(at + 3);
        }
        Ok(at + 1)
    }

    /// `super::super::x` at `at`: when it climbs out of the file, it becomes `crate::service` (or
    /// `crate`, one level more). Returns the next position.
    fn super_run(&mut self, tokens: &[TokenTree], at: usize, depth: usize) -> Result<usize> {
        let mut last = at;
        let mut count = 1;
        while is_path_sep(tokens, last + 1) && is_ident(tokens.get(last + 3), "super") {
            last += 3;
            count += 1;
        }
        if !is_path_sep(tokens, last + 1) || count <= depth {
            return Ok(last + 1);
        }
        let next_is_stem = is_ident(tokens.get(last + 3), &self.home.stem);
        let text = match (count - depth, next_is_stem) {
            (1, true) => format!("crate::{}", self.home.group),
            (1, false) => "crate::service".to_string(),
            (2, _) => "crate".to_string(),
            _ => bail!("{}: `super` climbs above the crate", self.home.shim()),
        };
        self.splice(&tokens[at], &tokens[last], text);
        Ok(last + 1)
    }

    /// `crate::service::<stem>` at `at` becomes `crate::<group>::<stem>`.
    fn crate_path(&mut self, tokens: &[TokenTree], at: usize) -> usize {
        if is_path_sep(tokens, at + 1)
            && is_ident(tokens.get(at + 3), "service")
            && is_path_sep(tokens, at + 4)
            && is_ident(tokens.get(at + 6), &self.home.stem)
        {
            self.splice(&tokens[at + 3], &tokens[at + 3], self.home.group.clone());
        }
        at + 1
    }

    fn splice(&mut self, first: &TokenTree, last: &TokenTree, text: String) {
        self.splices.push(Splice {
            start: self.index.offset(first.span().start()),
            end: self.index.offset(last.span().end()),
            text,
        });
    }
}

fn is_ident(token: Option<&TokenTree>, name: &str) -> bool {
    matches!(token, Some(TokenTree::Ident(ident)) if ident == name)
}

/// `::` is two joined `:` punctuation tokens at `at` and `at + 1`.
fn is_path_sep(tokens: &[TokenTree], at: usize) -> bool {
    matches!(
        (tokens.get(at), tokens.get(at + 1)),
        (Some(TokenTree::Punct(first)), Some(TokenTree::Punct(second)))
            if first.as_char() == ':' && first.spacing() == Spacing::Joint && second.as_char() == ':'
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strangle::area_fixture::{ORIGINAL, PURE, SERVICE, original_tree};
    use crate::strangle::fixture::TempTree;

    fn relocated(tree: &TempTree) -> Result<Vec<String>> {
        let map = Map::load(tree.root())?;
        let mut files = FileSet::new(tree.root());
        let report = relocate(&mut files, &map, "loyalty")?;
        files.write()?;
        Ok(report)
    }

    fn read(tree: &TempTree, path: &str) -> String {
        std::fs::read_to_string(tree.root().join(path)).unwrap()
    }

    #[test]
    fn the_file_moves_into_its_stub_with_its_paths_rewritten() {
        let tree = original_tree();
        relocated(&tree).unwrap();
        let expected = ORIGINAL
            .replacen("use super::*;", "use crate::service::*;", 1)
            .replace("use super::super::golden;", "use crate::service::golden;")
            .replace(
                "crate::service::loyalty::Card",
                "crate::household::loyalty::Card",
            );
        assert_eq!(read(&tree, PURE), expected);
        // The `use super::*;` inside `mod tests` still means the file's own module.
        assert!(read(&tree, PURE).contains("mod tests {\n    use super::*;"));
    }

    #[test]
    fn the_shim_keeps_the_public_items_and_an_empty_forward_region() {
        let tree = original_tree();
        relocated(&tree).unwrap();
        assert_eq!(
            read(&tree, SERVICE),
            "//! Shim of the `loyalty` area: moved to `crate::household::loyalty` (spec core-strangler, D1).

pub use crate::household::loyalty::Card;

// forward:begin
// forward:end
"
        );
    }

    #[test]
    fn a_second_run_changes_nothing() {
        let tree = original_tree();
        relocated(&tree).unwrap();
        let (shim, pure) = (read(&tree, SERVICE), read(&tree, PURE));
        let report = relocated(&tree).unwrap();
        assert!(report[0].contains("shim already"), "{report:?}");
        assert_eq!((read(&tree, SERVICE), read(&tree, PURE)), (shim, pure));
    }

    #[test]
    fn a_target_that_is_not_a_stub_is_never_overwritten() {
        let tree = original_tree();
        std::fs::write(tree.root().join(PURE), "pub fn mine() {}\n").unwrap();
        let error = relocated(&tree).unwrap_err();
        assert!(error.to_string().contains("not a placeholder"), "{error}");
        assert_eq!(read(&tree, SERVICE), ORIGINAL);
    }

    #[test]
    fn every_shape_of_super_path_is_rewritten_by_how_far_it_climbs() {
        let tree = original_tree();
        let map = Map::load(tree.root()).unwrap();
        let homes = homes(&map, &map.areas[0]);
        let source = "pub(super) fn a() { assert!(super::b()); let _ = super::super::c; }
use super::loyalty::T;
mod inner { use super::d; use super::super::e; fn f() { super::super::super::g; } }
";
        assert_eq!(
            rewrite_paths(source, &homes[0]).unwrap(),
            "pub(super) fn a() { assert!(crate::service::b()); let _ = crate::c; }
use crate::household::loyalty::T;
mod inner { use super::d; use crate::service::e; fn f() { crate::g; } }
"
        );
        let climbing = "fn a() { super::super::super::g; }";
        assert!(rewrite_paths(climbing, &homes[0]).is_err());
    }
}
