//! Rule `no-f64-in-money`: money code does its arithmetic in `Amount`/`Decimal`, never in `f64`.
//! `clippy::disallowed_types` can name `f64`, but its list is workspace-wide and cannot be scoped
//! to one module, so this text rule reads `crates/core/src/money` instead.
//!
//! A line with `f64` in non-test code fails unless it carries the marker `// allowed: display only`
//! (a share or percentage that only feeds a chart, never a balance).

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

const MONEY_DIR: &str = "crates/core/src/money";
const MARKER: &str = "// allowed: display only";
const CFG_TEST: &str = "#[cfg(test)]";

/// One `f64` in money code that the rule does not accept.
#[derive(Debug, PartialEq, Eq)]
pub struct Finding {
    pub file: String,
    pub line: usize,
}

impl Finding {
    pub fn message(&self) -> String {
        format!(
            "error: {}:{}: `f64` in money code (use Amount/Decimal, or mark the line `{MARKER}`)",
            self.file, self.line
        )
    }
}

fn is_test_file(name: &str) -> bool {
    name == "tests.rs" || name.ends_with("_tests.rs") || name.starts_with("test_")
}

/// The line with the contents of string and char literals dropped, so that a `//` or a brace
/// inside a literal is not read as code. A lifetime (`'a`) is not a char literal and stays.
fn without_literals(line: &str) -> String {
    let chars: Vec<char> = line.chars().collect();
    let mut kept = String::new();
    let mut index = 0;
    while index < chars.len() {
        let ch = chars[index];
        if ch == '"' {
            index += 1;
            while index < chars.len() && chars[index] != '"' {
                index += if chars[index] == '\\' { 2 } else { 1 };
            }
            index += 1;
        } else if ch == '\'' {
            let close = match (chars.get(index + 1), chars.get(index + 2)) {
                (Some('\\'), _) => chars[index + 2..]
                    .iter()
                    .position(|&next| next == '\'')
                    .map(|offset| index + 2 + offset),
                (Some(_), Some('\'')) => Some(index + 2),
                _ => None,
            };
            if let Some(close) = close {
                index = close + 1;
            } else {
                kept.push(ch);
                index += 1;
            }
        } else {
            kept.push(ch);
            index += 1;
        }
    }
    kept
}

/// The code part of a line: what comes before a `//` comment, literals dropped.
fn code_of(line: &str) -> String {
    let stripped = without_literals(line);
    match stripped.split_once("//") {
        Some((code, _)) => code.to_string(),
        None => stripped,
    }
}

/// Net `{` minus `}` of a line's code part.
fn brace_delta(line: &str) -> i32 {
    code_of(line).chars().fold(0, |depth, ch| match ch {
        '{' => depth + 1,
        '}' => depth - 1,
        _ => depth,
    })
}

/// Line indices (0-based) of the items that follow a `#[cfg(test)]` line: a one-line item
/// ending in `;`, or a braced block up to its closing brace.
fn test_item_lines(lines: &[&str]) -> Vec<bool> {
    let mut skipped = vec![false; lines.len()];
    let mut index = 0;
    while index < lines.len() {
        if lines[index].trim() != CFG_TEST {
            index += 1;
            continue;
        }
        skipped[index] = true;
        let mut depth = 0;
        index += 1;
        while index < lines.len() {
            skipped[index] = true;
            depth += brace_delta(lines[index]);
            let code = code_of(lines[index]);
            index += 1;
            if depth <= 0 && (code.trim_end().ends_with(';') || code.contains('}')) {
                break;
            }
        }
    }
    skipped
}

/// Lines of one source text that use `f64` outside tests and without the marker.
fn findings_in(file: &str, text: &str) -> Vec<Finding> {
    let lines: Vec<&str> = text.lines().collect();
    let skipped = test_item_lines(&lines);
    lines
        .iter()
        .enumerate()
        .filter(|(index, _)| !skipped[*index])
        .filter(|(_, line)| code_of(line).contains("f64") && !line.contains(MARKER))
        .map(|(index, _)| Finding {
            file: file.to_string(),
            line: index + 1,
        })
        .collect()
}

fn source_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    let mut entries = fs::read_dir(dir)
        .with_context(|| format!("read {}", dir.display()))?
        .collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if entry.file_type()?.is_dir() {
            source_files(&path, files)?;
        } else if name.ends_with(".rs") && !is_test_file(&name) {
            files.push(path);
        }
    }
    Ok(())
}

/// Scans the money module under the repository `root`.
pub fn check(root: &Path) -> Result<Vec<Finding>> {
    let mut files = Vec::new();
    source_files(&root.join(MONEY_DIR), &mut files)?;
    let mut found = Vec::new();
    for file in files {
        let text = fs::read_to_string(&file).with_context(|| format!("read {}", file.display()))?;
        let name = file
            .strip_prefix(root)
            .unwrap_or(&file)
            .to_string_lossy()
            .replace('\\', "/");
        found.extend(findings_in(&name, &text));
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_f64_in_code_is_found_with_its_line() {
        let found = findings_in("money/x.rs", "fn a() {}\nlet rate: f64 = 1.0;\n");
        assert_eq!(
            found,
            [Finding {
                file: "money/x.rs".into(),
                line: 2
            }]
        );
    }

    #[test]
    fn conversions_named_after_f64_count_too() {
        assert_eq!(findings_in("x.rs", "let v = a.to_f64();").len(), 1);
    }

    #[test]
    fn the_display_only_marker_and_comments_are_accepted() {
        let text = "pub pct: f64, // allowed: display only\n/// no f64 here\n";
        assert!(findings_in("x.rs", text).is_empty());
    }

    #[test]
    fn test_code_is_not_scanned() {
        let text = "fn a() {}\n#[cfg(test)]\nmod tests { fn b(_: f64) {} }\n";
        assert!(findings_in("x.rs", text).is_empty());
    }

    #[test]
    fn code_after_a_test_item_is_still_scanned() {
        let text = "#[cfg(test)]\nmod x_tests;\nfn a(_: f64) {}\n#[cfg(test)]\nmod t {\n    fn b(_: f64) {}\n}\nfn c(_: f64) {}\n";
        let lines: Vec<usize> = findings_in("x.rs", text)
            .iter()
            .map(|found| found.line)
            .collect();
        assert_eq!(lines, [3, 8]);
    }

    #[test]
    fn a_url_string_does_not_hide_later_f64() {
        let text = "let url = \"https://x\"; let rate: f64 = 1.0;\n";
        let lines: Vec<usize> = findings_in("x.rs", text)
            .iter()
            .map(|found| found.line)
            .collect();
        assert_eq!(lines, [1]);
    }

    #[test]
    fn a_brace_char_in_test_code_does_not_hide_later_f64() {
        let text = "#[cfg(test)]\nmod t {\n    const OPEN: char = '{';\n}\nfn a(_: f64) {}\n";
        let lines: Vec<usize> = findings_in("x.rs", text)
            .iter()
            .map(|found| found.line)
            .collect();
        assert_eq!(lines, [5]);
    }

    #[test]
    fn a_deliberate_f64_in_a_money_file_turns_the_check_red() {
        let root = std::env::temp_dir().join(format!("xtask-money-f64-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join(MONEY_DIR)).unwrap();
        fs::write(
            root.join(MONEY_DIR).join("fixture.rs"),
            "static X: f64 = 1.0;\n",
        )
        .unwrap();
        let found = check(&root).unwrap();
        fs::remove_dir_all(&root).unwrap();
        assert_eq!(found.len(), 1);
        assert!(found[0].message().contains("fixture.rs:1"));
    }

    #[test]
    fn the_money_module_of_the_repository_is_clean() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let found = check(&root).unwrap();
        assert!(found.is_empty(), "{found:#?}");
    }
}
