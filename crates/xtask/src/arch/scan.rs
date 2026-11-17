//! Reads one source text: which `crate::<module>` paths it names, and which modules it declares
//! under `#[cfg(test)]`. A line scanner over `std`; no parser.

use std::collections::BTreeSet;

use super::Edge;

const CFG_TEST: &str = "#[cfg(test)]";
const CRATE_PATH: &str = "crate::";

/// Root items of `lib.rs` that are not modules, with the module they count as.
/// Anything else after `crate::` is reached through `pub use model::*`.
fn root_item_module(name: &str) -> &'static str {
    match name {
        "Amount" => "amount",
        "Error" | "Result" => "error",
        "SnapshotFormat" | "VaultFile" => "storage",
        _ if name.starts_with("VAULT_") => "lib",
        _ => "model",
    }
}

/// The module a name after `crate::` stands for.
fn resolve(name: &str, modules: &BTreeSet<String>) -> String {
    if modules.contains(name) {
        name.to_string()
    } else {
        root_item_module(name).to_string()
    }
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
    let lines: Vec<&str> = text.lines().collect();
    (0..lines.len())
        .filter(|&index| lines[index].trim() == CFG_TEST)
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

fn is_ident_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

fn leading_ident(text: &str) -> &str {
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
fn name_offsets(text: &str) -> Vec<usize> {
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

/// Every `crate::` path of the non-test part of `text`, as edges from `from_file` (module `from_module`).
pub fn read_edges(
    from_file: &str,
    from_module: &str,
    text: &str,
    modules: &BTreeSet<String>,
) -> Vec<Edge> {
    let cleaned = non_test_part(&strip_comments(text));
    name_offsets(&cleaned)
        .into_iter()
        .filter_map(|offset| {
            let name = leading_ident(&cleaned[offset..]);
            (!name.is_empty()).then(|| Edge {
                from_file: from_file.to_string(),
                line: cleaned[..offset].matches('\n').count() + 1,
                from_module: from_module.to_string(),
                to_module: resolve(name, modules),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn modules() -> BTreeSet<String> {
        ["model", "service", "storage", "amount"]
            .into_iter()
            .map(String::from)
            .collect()
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
        let text = "use crate::VaultDoc;\nlet _ = crate::Amount::ZERO;\nuse crate::VAULT_MAGIC;\nuse crate::VaultFile;\n";
        assert_eq!(
            pairs(text),
            vec![
                (1, "model".to_string()),
                (2, "amount".to_string()),
                (3, "lib".to_string()),
                (4, "storage".to_string())
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
    fn comments_do_not_count() {
        assert_eq!(
            pairs("// use crate::service::X;\n/// crate::storage\n"),
            vec![]
        );
    }
}
