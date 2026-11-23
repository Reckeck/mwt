//! The permanent ban (core-dismantle D2, D10): the facade's old name and the `forward` macro
//! never come back. Every `crates/*/src/**/*.rs` file is read as plain text, comments and tests
//! included. The two words are built from halves so this file does not contain them itself.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::scan;

/// The old name of the facade type, as a whole word.
const OLD_FACADE_NAME: &str = concat!("Unlocked", "Vault");
/// The delegation macro of the strangler phase, as a token.
const FORWARD_MACRO: &str = concat!("for", "ward!");

/// The strangle tool names both words and goes in the next commit; this skip goes with it.
const TOOL_UNTIL_NEXT_COMMIT: &str = "xtask/src/strangle";

/// A banned word found in a file: `file` is relative to `crates/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Banned {
    pub file: String,
    pub line: usize,
    pub word: &'static str,
}

impl Banned {
    pub fn line_text(&self) -> String {
        format!(
            "error: crates/{}:{}: `{}` is banned (the facade's old name and the forwarding macro never come back)",
            self.file, self.line, self.word
        )
    }
}

/// The banned words in one file's text, by line.
pub fn banned_in(file: &str, text: &str) -> Vec<Banned> {
    let mut found = Vec::new();
    for (index, line) in text.lines().enumerate() {
        for (offset, _) in line.match_indices(OLD_FACADE_NAME) {
            let before = line[..offset].chars().next_back();
            let after = line[offset + OLD_FACADE_NAME.len()..].chars().next();
            if !before.is_some_and(scan::is_ident_char) && !after.is_some_and(scan::is_ident_char) {
                found.push(banned(file, index, OLD_FACADE_NAME));
            }
        }
        for (offset, _) in line.match_indices(FORWARD_MACRO) {
            if !line[..offset]
                .chars()
                .next_back()
                .is_some_and(scan::is_ident_char)
            {
                found.push(banned(file, index, FORWARD_MACRO));
            }
        }
    }
    found
}

fn banned(file: &str, index: usize, word: &'static str) -> Banned {
    Banned {
        file: file.to_string(),
        line: index + 1,
        word,
    }
}

/// Every `.rs` file under `dir`, recursively, in name order.
fn rust_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    let mut entries = fs::read_dir(dir)
        .with_context(|| format!("read {}", dir.display()))?
        .collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            rust_files(&path, files)?;
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            files.push(path);
        }
    }
    Ok(())
}

/// The banned words of every `crates/*/src/**/*.rs` under the repository `root`.
pub fn check(root: &Path) -> Result<Vec<Banned>> {
    let crates = root.join("crates");
    let mut members = fs::read_dir(&crates)
        .with_context(|| format!("read {}", crates.display()))?
        .collect::<std::io::Result<Vec<_>>>()?;
    members.sort_by_key(fs::DirEntry::file_name);
    let mut files = Vec::new();
    for member in members {
        let src = member.path().join("src");
        if src.is_dir() {
            rust_files(&src, &mut files)?;
        }
    }
    let mut found = Vec::new();
    for file in files {
        let text = fs::read_to_string(&file).with_context(|| format!("read {}", file.display()))?;
        let relative = file.strip_prefix(&crates).unwrap_or(&file);
        if relative.starts_with(TOOL_UNTIL_NEXT_COMMIT) {
            continue;
        }
        found.extend(banned_in(
            &relative.to_string_lossy().replace('\\', "/"),
            &text,
        ));
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn old_name() -> String {
        ["Unlocked", "Vault"].concat()
    }

    fn macro_call() -> String {
        ["for", "ward! {}"].concat()
    }

    #[test]
    fn the_old_facade_name_is_a_violation_even_in_a_comment() {
        let text = format!("fn f() {{}}\n// the {} type\n", old_name());
        let found = banned_in("core/src/a.rs", &text);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].line, 2);
        assert!(found[0].line_text().contains("crates/core/src/a.rs:2"));
    }

    #[test]
    fn a_longer_name_is_not_the_old_facade_name() {
        let text = format!("struct My{0}; struct {0}Ext;\n", old_name());
        assert!(banned_in("core/src/a.rs", &text).is_empty());
    }

    #[test]
    fn the_forwarding_macro_is_a_violation() {
        let found = banned_in("core/src/a.rs", &format!("{}\n", macro_call()));
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].line, 1);
        assert!(banned_in("core/src/a.rs", "fn forwarded() {}\n").is_empty());
    }

    #[test]
    fn the_tree_is_green() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let found = check(&root).expect("scan the tree");
        assert!(found.is_empty(), "{found:?}");
    }
}
