//! The files a rewriting subcommand touches, held in memory: read on first use, edited, then either
//! written (`write`) or only compared (`changed`, for `--check`). One home for "declare a module once".

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

struct Entry {
    /// `None`: the file does not exist yet.
    original: Option<String>,
    current: String,
}

pub struct FileSet {
    root: PathBuf,
    entries: BTreeMap<String, Entry>,
    /// Paths to delete on `write`.
    removed: BTreeSet<String>,
}

impl FileSet {
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            entries: BTreeMap::new(),
            removed: BTreeSet::new(),
        }
    }

    pub fn exists(&mut self, path: &str) -> bool {
        !self.removed.contains(path)
            && self
                .entry(path)
                .is_ok_and(|entry| entry.original.is_some() || !entry.current.is_empty())
    }

    /// The current text; a missing file reads as empty.
    pub fn read(&mut self, path: &str) -> Result<&str> {
        Ok(&self.entry(path)?.current)
    }

    pub fn set(&mut self, path: &str, text: String) -> Result<()> {
        self.entry(path)?.current = text;
        Ok(())
    }

    /// Deletes the file on `write`.
    pub fn remove(&mut self, path: &str) -> Result<()> {
        self.set(path, String::new())?;
        self.removed.insert(path.to_string());
        Ok(())
    }

    pub fn create_if_missing(&mut self, path: &str, text: &str) -> Result<()> {
        if !self.exists(path) {
            self.set(path, text.to_string())?;
        }
        Ok(())
    }

    /// Adds `lines` after the last `mod` declaration of `file` (at the end when there is none),
    /// unless the file already declares module `name`.
    pub fn declare(&mut self, file: &str, name: &str, lines: &[String]) -> Result<()> {
        let text = self.read(file)?.to_string();
        let mut all: Vec<&str> = text.lines().collect();
        if all.iter().any(|line| declares(line, name)) {
            return Ok(());
        }
        let at = match all.iter().rposition(|line| mod_name(line).is_some()) {
            Some(index) => index + 1,
            None => {
                if all.last().is_some_and(|line| !line.trim().is_empty()) {
                    all.push("");
                }
                all.len()
            }
        };
        let mut joined: Vec<&str> = all[..at].to_vec();
        joined.extend(lines.iter().map(String::as_str));
        joined.extend(&all[at..]);
        let joined = joined.join("\n") + "\n";
        self.set(file, joined)
    }

    /// The paths whose text differs from the disk (or that do not exist yet).
    pub fn changed(&self) -> Vec<&str> {
        self.entries
            .iter()
            .filter(|(_, entry)| entry.original.as_deref() != Some(entry.current.as_str()))
            .filter(|(_, entry)| entry.original.is_some() || !entry.current.is_empty())
            .map(|(path, _)| path.as_str())
            .collect()
    }

    pub fn write(&self) -> Result<()> {
        for path in self.changed() {
            let file = self.root.join(path);
            if self.removed.contains(path) {
                std::fs::remove_file(&file).with_context(|| format!("remove {path}"))?;
                continue;
            }
            if let Some(dir) = file.parent() {
                std::fs::create_dir_all(dir)
                    .with_context(|| format!("create {}", dir.display()))?;
            }
            std::fs::write(&file, &self.entries[path].current)
                .with_context(|| format!("write {path}"))?;
        }
        Ok(())
    }

    fn entry(&mut self, path: &str) -> Result<&mut Entry> {
        if !self.entries.contains_key(path) {
            let original = match std::fs::read_to_string(self.root.join(path)) {
                Ok(text) => Some(text),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(error) => return Err(error).with_context(|| format!("read {path}")),
            };
            let current = original.clone().unwrap_or_default();
            self.entries
                .insert(path.to_string(), Entry { original, current });
        }
        Ok(self.entries.get_mut(path).expect("inserted above"))
    }
}

/// The name of `mod name;` (any visibility), when the line is such a declaration.
fn mod_name(line: &str) -> Option<&str> {
    let mut rest = line.trim();
    for prefix in ["pub(crate) ", "pub "] {
        rest = rest.strip_prefix(prefix).unwrap_or(rest);
    }
    rest.strip_prefix("mod ")?.strip_suffix(';').map(str::trim)
}

fn declares(line: &str, name: &str) -> bool {
    mod_name(line) == Some(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strangle::fixture::TempTree;

    fn lines(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| (*item).to_string()).collect()
    }

    #[test]
    fn a_declaration_goes_after_the_last_mod_and_only_once() {
        let tree = TempTree::new(&[(
            "a.rs",
            "//! doc\n\npub mod one;\nmod two;\n\npub use one::X;\n",
        )]);
        let mut files = FileSet::new(tree.root());
        files
            .declare("a.rs", "three", &lines(&["pub mod three;"]))
            .unwrap();
        files
            .declare("a.rs", "three", &lines(&["pub mod three;"]))
            .unwrap();
        files
            .declare("a.rs", "two", &lines(&["pub mod two;"]))
            .unwrap();
        assert_eq!(
            files.read("a.rs").unwrap(),
            "//! doc\n\npub mod one;\nmod two;\npub mod three;\n\npub use one::X;\n"
        );
    }

    #[test]
    fn without_a_mod_the_declaration_is_appended_after_a_blank_line() {
        let tree = TempTree::new(&[("a.rs", "//! doc\n")]);
        let mut files = FileSet::new(tree.root());
        files
            .declare("a.rs", "one", &lines(&["pub mod one;"]))
            .unwrap();
        assert_eq!(files.read("a.rs").unwrap(), "//! doc\n\npub mod one;\n");
        files
            .declare("missing.rs", "one", &lines(&["pub mod one;"]))
            .unwrap();
        assert_eq!(files.read("missing.rs").unwrap(), "pub mod one;\n");
    }

    #[test]
    fn a_removed_file_is_deleted_on_write() {
        let tree = TempTree::new(&[("a.rs", "x\n"), ("b.rs", "y\n")]);
        let mut files = FileSet::new(tree.root());
        files.remove("a.rs").unwrap();
        assert!(!files.exists("a.rs"));
        files.write().unwrap();
        assert!(!tree.root().join("a.rs").exists());
        assert!(tree.root().join("b.rs").exists());
    }

    #[test]
    fn only_edited_files_are_changed_and_written() {
        let tree = TempTree::new(&[("a.rs", "x\n"), ("b.rs", "y\n")]);
        let mut files = FileSet::new(tree.root());
        files.read("a.rs").unwrap();
        files.set("b.rs", "z\n".into()).unwrap();
        files.create_if_missing("c/d.rs", "new\n").unwrap();
        files.create_if_missing("b.rs", "ignored\n").unwrap();
        assert_eq!(files.changed(), ["b.rs", "c/d.rs"]);
        files.write().unwrap();
        assert_eq!(
            std::fs::read_to_string(tree.root().join("b.rs")).unwrap(),
            "z\n"
        );
        assert_eq!(
            std::fs::read_to_string(tree.root().join("c/d.rs")).unwrap(),
            "new\n"
        );
    }
}
