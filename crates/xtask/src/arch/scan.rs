//! Reads one source text: which `crate::<module>` paths it names, and which modules it declares
//! under `#[cfg(test)]`. A line scanner over `std`; no parser.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

use super::Edge;

const CFG_TEST: &str = "#[cfg(test)]";
const CFG_TESTKIT: &str = "#[cfg(any(test, feature = \"testkit\"))]";
const CRATE_PATH: &str = "crate::";

/// The names `crate::<name>` may stand for: the modules of the tree and the table, and the names
/// `lib.rs` re-exports (`pub use mwt_x as name;`, `pub use mwt_x::{Item, ..};`), each with the module it
/// counts as. Anything else after `crate::` is reached through `pub use model::*`.
#[derive(Debug)]
pub struct Names {
    modules: BTreeSet<String>,
    reexports: BTreeMap<String, String>,
}

impl Names {
    pub fn new(modules: BTreeSet<String>, lib_text: &str) -> Result<Self> {
        let reexports = reexports(lib_text, &modules)?;
        Ok(Self { modules, reexports })
    }

    /// The module a name after `crate::` stands for.
    fn resolve(&self, name: &str) -> String {
        if self.modules.contains(name) {
            name.to_string()
        } else {
            self.reexports
                .get(name)
                .cloned()
                .unwrap_or_else(|| "model".to_string())
        }
    }
}

/// Every name a `pub use` of `lib.rs` brings in from another path, with its module: an alias of a
/// whole crate or a lowercase item that is a module counts as itself; any other item as the module
/// named like its crate (`mwt_crypto` -> `crypto`, the first segment of the path), else like itself in
/// lowercase (`Amount` -> `amount`), else as `lib`. An aliased item (`Amount as Money`) is keyed by its
/// alias and homed by its original name. A statement that is not a flat path with an optional
/// `{A, B as C}` group, `*`, or `as` is an error naming it.
fn reexports(lib_text: &str, modules: &BTreeSet<String>) -> Result<BTreeMap<String, String>> {
    let mut found = BTreeMap::new();
    for statement in strip_comments(lib_text).split(';') {
        let Some(start) = statement.find("pub use ") else {
            continue;
        };
        let rest = statement[start + "pub use ".len()..].trim();
        let import = read_use(rest)
            .with_context(|| format!("cannot read the statement `pub use {rest};`"))?;
        let krate = import.krate.strip_prefix("mwt_").unwrap_or(&import.krate);
        for (original, key) in import.items {
            let lowercase = original.to_lowercase();
            let home = if import.crate_alias {
                key.clone()
            } else {
                [original.as_str(), krate, lowercase.as_str()]
                    .into_iter()
                    .find(|candidate| modules.contains(*candidate))
                    .unwrap_or("lib")
                    .to_string()
            };
            found.insert(key, home);
        }
    }
    Ok(found)
}

/// What one `pub use` statement (the text after `pub use `) names.
struct Import {
    krate: String,
    /// `(original name, name it is reached by)` of each item; `*` is left out.
    items: Vec<(String, String)>,
    /// `pub use mwt_x as name;`: the whole crate under a new name.
    crate_alias: bool,
}

fn read_use(rest: &str) -> Result<Import> {
    let (path, items, crate_alias) = if let Some(open) = rest.find('{') {
        let inner = rest[open + 1..]
            .strip_suffix('}')
            .context("the group is not closed at the end")?;
        let path = rest[..open]
            .trim()
            .strip_suffix("::")
            .context("the group does not follow a path")?;
        (path, inner.split(',').collect::<Vec<_>>(), false)
    } else if let Some((path, last)) = rest.rsplit_once("::") {
        (path, vec![last], false)
    } else {
        let (name, alias) = split_alias(rest)?;
        return Ok(Import {
            krate: name.to_string(),
            items: vec![(name.to_string(), alias.to_string())],
            crate_alias: alias != name,
        });
    };
    let mut segments = path.split("::").map(str::trim);
    let krate = segments.next().unwrap_or_default();
    for segment in std::iter::once(krate).chain(segments) {
        ensure!(is_name(segment), "`{segment}` is not a path segment");
    }
    let mut read = Vec::new();
    for item in items
        .into_iter()
        .map(str::trim)
        .filter(|item| !item.is_empty())
    {
        if item == "*" {
            continue;
        }
        let (name, alias) = split_alias(item)?;
        read.push((name.to_string(), alias.to_string()));
    }
    Ok(Import {
        krate: krate.to_string(),
        items: read,
        crate_alias,
    })
}

/// `Name` or `Name as Alias`, as `(name, alias)`.
fn split_alias(item: &str) -> Result<(&str, &str)> {
    let (name, alias) = match item.split_once(" as ") {
        Some((name, alias)) => (name.trim(), alias.trim()),
        None => (item.trim(), item.trim()),
    };
    ensure!(
        is_name(name) && is_name(alias),
        "`{item}` is not a name, `name as alias` or `*`"
    );
    Ok((name, alias))
}

fn is_name(text: &str) -> bool {
    !text.is_empty() && text.chars().all(is_ident_char)
}

/// The text without `//` comments (strings are respected); line breaks stay, so offsets keep their lines.
fn strip_comments(text: &str) -> String {
    text.lines()
        .map(strip_line_comment)
        .collect::<Vec<_>>()
        .join("\n")
}

fn strip_line_comment(line: &str) -> &str {
    let mut in_string = false;
    let mut escaped = false;
    let mut previous = '\0';
    for (offset, ch) in line.char_indices() {
        if in_string {
            match (escaped, ch) {
                (true, _) => escaped = false,
                (false, '\\') => escaped = true,
                (false, '"') => in_string = false,
                _ => {}
            }
        } else if ch == '"' {
            in_string = true;
        } else if ch == '/' && previous == '/' {
            return &line[..offset - 1];
        }
        previous = ch;
    }
    line
}

/// What follows a top-level `#[cfg(test)]`: a `mod name;` declaration (returns the name) or anything else.
fn declared_module(following: &[&str]) -> Option<String> {
    let item = following
        .iter()
        .map(|line| line.trim())
        .find(|line| !line.is_empty() && !line.starts_with("#["))?;
    let rest = item
        .strip_prefix("pub(crate) mod ")
        .or_else(|| item.strip_prefix("pub mod "))
        .or_else(|| item.strip_prefix("mod "))?;
    rest.strip_suffix(';').map(|name| name.trim().to_string())
}

/// Modules the text declares as `#[cfg(test)] mod name;`.
pub fn test_modules(text: &str) -> Vec<String> {
    modules_under(text, CFG_TEST)
}

/// Modules the text declares as `#[cfg(any(test, feature = "testkit"))] mod name;`: test support
/// that other crates can compile, checked as the `testkit` crate.
pub fn testkit_modules(text: &str) -> Vec<String> {
    modules_under(text, CFG_TESTKIT)
}

fn modules_under(text: &str, attribute: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().collect();
    (0..lines.len())
        .filter(|&index| lines[index].trim() == attribute)
        .filter_map(|index| declared_module(&lines[index + 1..]))
        .collect()
}

/// Index of the last line of the item that starts at `lines[0]`: it ends at a `;` outside braces
/// or at the brace that closes its first `{`.
fn item_end(lines: &[&str]) -> usize {
    let mut depth = 0usize;
    for (index, line) in lines.iter().enumerate() {
        for ch in line.chars() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return index;
                    }
                }
                ';' if depth == 0 => return index,
                _ => {}
            }
        }
    }
    lines.len().saturating_sub(1)
}

/// The text without the items under a top-level `#[cfg(test)]` (a `mod name;` declaration is kept:
/// the module lives in another file). Skipped lines become empty, so line numbers stay.
fn non_test_part(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut kept = Vec::with_capacity(lines.len());
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        if line.trim_end() == CFG_TEST && declared_module(&lines[index + 1..]).is_none() {
            let end = index + 1 + item_end(&lines[index + 1..]);
            kept.extend(std::iter::repeat_n(
                "",
                end.min(lines.len() - 1) + 1 - index,
            ));
            index = end + 1;
        } else {
            kept.push(line);
            index += 1;
        }
    }
    kept.join("\n")
}

pub(super) fn is_ident_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

pub(super) fn leading_ident(text: &str) -> &str {
    let end = text.find(|ch| !is_ident_char(ch)).unwrap_or(text.len());
    &text[..end]
}

/// Offsets of the first identifier of every top-level element of a `{ … }` group that starts at `open`.
fn group_items(text: &str, open: usize) -> Vec<usize> {
    let mut items = Vec::new();
    let mut depth = 0usize;
    let mut expecting = false;
    for (offset, ch) in text[open..].char_indices() {
        match ch {
            '{' => {
                depth += 1;
                expecting = depth == 1;
            }
            '}' => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            ',' => expecting = depth == 1,
            _ if expecting && is_ident_char(ch) => {
                items.push(open + offset);
                expecting = false;
            }
            _ => {}
        }
    }
    items
}

/// Offsets where the module name of each `crate::` path in the text begins.
pub(super) fn name_offsets(text: &str) -> Vec<usize> {
    let mut found = Vec::new();
    for (start, _) in text.match_indices(CRATE_PATH) {
        if text[..start].chars().next_back().is_some_and(is_ident_char) {
            continue;
        }
        let after = start + CRATE_PATH.len();
        let rest = text[after..].trim_start();
        let name_start = text.len() - rest.len();
        if rest.starts_with('{') {
            found.extend(group_items(text, name_start));
        } else {
            found.push(name_start);
        }
    }
    found
}

/// The text without comments and without the items under a top-level `#[cfg(test)]`; line numbers stay.
pub(super) fn production_text(text: &str) -> String {
    non_test_part(&strip_comments(text))
}

/// Every `crate::` path of the non-test part of `text`, as edges from `from_file` (module `from_module`).
pub fn read_edges(from_file: &str, from_module: &str, text: &str, names: &Names) -> Vec<Edge> {
    let cleaned = production_text(text);
    name_offsets(&cleaned)
        .into_iter()
        .filter_map(|offset| {
            let name = leading_ident(&cleaned[offset..]);
            (!name.is_empty()).then(|| Edge {
                from_file: from_file.to_string(),
                line: cleaned[..offset].matches('\n').count() + 1,
                from_module: from_module.to_string(),
                to_module: names.resolve(name),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIB: &str = "pub use model::*;\npub use mwt_base::{Amount, Result, amount};\npub use mwt_crypto::{SnapshotFormat, VAULT_MAGIC};\npub use mwt_money as money;\npub use storage::VaultFile;\n";

    fn modules() -> Names {
        let modules = [
            "model", "service", "storage", "amount", "crypto", "money", "workflow",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        Names::new(modules, LIB).unwrap()
    }

    fn pairs(text: &str) -> Vec<(usize, String)> {
        read_edges("model/x.rs", "model", text, &modules())
            .into_iter()
            .map(|edge| (edge.line, edge.to_module))
            .collect()
    }

    #[test]
    fn a_back_edge_is_reported_with_file_and_line() {
        let edges = read_edges(
            "model/x.rs",
            "model",
            "use std::fmt;\n\nuse crate::service::Draft;\n",
            &modules(),
        );
        assert_eq!(
            edges,
            vec![Edge {
                from_file: "model/x.rs".into(),
                line: 3,
                from_module: "model".into(),
                to_module: "service".into(),
            }]
        );
    }

    #[test]
    fn a_use_group_with_several_modules_is_read() {
        let text = "use crate::{\n    service,\n    storage::{a, b},\n    amount::Amount,\n};\n";
        assert_eq!(
            pairs(text),
            vec![
                (2, "service".to_string()),
                (3, "storage".to_string()),
                (4, "amount".to_string())
            ]
        );
    }

    #[test]
    fn a_name_that_is_not_a_module_counts_as_model() {
        let text = "use crate::VaultDoc;\nlet _ = crate::Amount::ZERO;\nuse crate::VAULT_MAGIC;\nuse crate::VaultFile;\nuse crate::SnapshotFormat;\nuse crate::Result;\n";
        assert_eq!(
            pairs(text),
            vec![
                (1, "model".to_string()),
                (2, "amount".to_string()),
                (3, "crypto".to_string()),
                (4, "storage".to_string()),
                (5, "crypto".to_string()),
                (6, "lib".to_string())
            ]
        );
    }

    #[test]
    fn code_after_cfg_test_is_skipped() {
        let text = "use crate::amount::Amount;\n#[cfg(test)]\nmod tests {\n    use crate::service::X;\n}\n";
        assert_eq!(pairs(text), vec![(1, "amount".to_string())]);
    }

    #[test]
    fn production_code_after_a_cfg_test_helper_is_read() {
        let text = "#[cfg(test)]\nfn helper() {}\nuse crate::service::X;\n";
        assert_eq!(pairs(text), vec![(3, "service".to_string())]);
    }

    #[test]
    fn a_module_declared_under_cfg_test_is_skipped() {
        let text = "#[cfg(test)]\nmod fixtures;\nmod real;\nuse crate::service::X;\n";
        assert_eq!(test_modules(text), vec!["fixtures".to_string()]);
        assert_eq!(pairs(text), vec![(4, "service".to_string())]);
    }

    #[test]
    fn a_module_declared_under_the_testkit_feature_is_listed_apart() {
        let text = "#[cfg(any(test, feature = \"testkit\"))]\npub mod fixtures;\n#[cfg(test)]\nmod tests;\n";
        assert_eq!(testkit_modules(text), vec!["fixtures".to_string()]);
        assert_eq!(test_modules(text), vec!["tests".to_string()]);
    }

    fn names_from(lib: &str) -> Result<Names> {
        let modules = ["model", "amount", "base", "money"]
            .into_iter()
            .map(String::from)
            .collect();
        Names::new(modules, lib)
    }

    #[test]
    fn a_group_after_a_nested_path_maps_each_item_to_its_crate() {
        let names = names_from("pub use mwt_model::records::{A, B};\n").unwrap();
        assert_eq!(names.resolve("A"), "model");
        assert_eq!(names.resolve("B"), "model");
    }

    #[test]
    fn an_aliased_item_is_keyed_by_its_alias() {
        let names = names_from("pub use mwt_base::{Amount as Money, Error};\n").unwrap();
        assert_eq!(names.resolve("Money"), "base");
        assert_eq!(names.resolve("Error"), "base");
    }

    #[test]
    fn a_statement_that_cannot_be_read_is_an_error_naming_it() {
        let error = names_from("pub use mwt_base::{A, nested::{B}};\n").unwrap_err();
        assert!(
            format!("{error:#}").contains("pub use mwt_base::{A, nested::{B}};"),
            "{error:#}"
        );
    }

    #[test]
    fn comments_do_not_count() {
        assert_eq!(
            pairs("// use crate::service::X;\n/// crate::storage\n"),
            vec![]
        );
    }
}
