//! The scanned files of the workspace and which of them are test code.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result};

use super::{FileFacts, scan_source};

/// A scanned file and whether it is test code as a whole.
#[derive(Debug)]
pub struct Scanned {
    pub facts: FileFacts,
    pub is_test: bool,
}

/// A macro call whose body the text scan does not look into.
#[derive(Debug, PartialEq, Eq)]
pub struct UnscannedMacro {
    pub path: String,
    pub line: usize,
}

impl std::fmt::Display for UnscannedMacro {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(out, "{}:{}: macro body not scanned", self.path, self.line)
    }
}

/// Every `.rs` file of the scanned directories, by path relative to the workspace root.
#[derive(Debug, Default)]
pub struct Corpus {
    files: BTreeMap<String, Scanned>,
}

impl Corpus {
    pub fn scan(root: &Path, dirs: &[String]) -> Result<Self> {
        let mut files = BTreeMap::new();
        for dir in dirs {
            collect(root, dir, &mut files)?;
        }
        let mut corpus = Self { files };
        corpus.mark_test_files();
        Ok(corpus)
    }

    #[cfg(test)]
    pub fn from_sources(sources: &[(&str, &str)]) -> Result<Self> {
        let mut files = BTreeMap::new();
        for (path, source) in sources {
            let facts = scan_source(source).with_context(|| format!("scan {path}"))?;
            files.insert(
                (*path).to_string(),
                Scanned {
                    facts,
                    is_test: false,
                },
            );
        }
        let mut corpus = Self { files };
        corpus.mark_test_files();
        Ok(corpus)
    }

    pub fn get(&self, path: &str) -> Option<&Scanned> {
        self.files.get(path)
    }

    /// One entry per macro call whose calls are not seen, in file and line order.
    pub fn unscanned_macros(&self) -> Vec<UnscannedMacro> {
        self.files
            .iter()
            .flat_map(|(path, scanned)| {
                scanned
                    .facts
                    .unscanned
                    .iter()
                    .map(move |line| UnscannedMacro {
                        path: path.clone(),
                        line: *line,
                    })
            })
            .collect()
    }

    /// The files whose path starts with `prefix` (a directory with its trailing `/`).
    pub fn under<'corpus>(
        &'corpus self,
        prefix: &str,
    ) -> impl Iterator<Item = (&'corpus str, &'corpus Scanned)> + use<'corpus> {
        let prefix = prefix.to_string();
        self.files
            .iter()
            .filter(move |(path, _)| path.starts_with(&prefix))
            .map(|(path, scanned)| (path.as_str(), scanned))
    }

    /// Identifiers that appear in test code anywhere in the corpus.
    pub fn test_idents(&self) -> BTreeSet<&str> {
        let mut found = BTreeSet::new();
        for scanned in self.files.values() {
            let sets = [
                Some(&scanned.facts.test_idents),
                scanned.is_test.then_some(&scanned.facts.other_idents),
            ];
            found.extend(sets.into_iter().flatten().flatten().map(String::as_str));
        }
        found
    }

    /// A file is test code when its name says so, when a `#[cfg(test)] mod x;` declares it, or when
    /// a test file declares it. Repeats until no file is added.
    fn mark_test_files(&mut self) {
        for (path, scanned) in &mut self.files {
            scanned.is_test = is_test_name(path);
        }
        loop {
            let mut found = Vec::new();
            for (path, scanned) in &self.files {
                for decl in &scanned.facts.mods {
                    if decl.in_test || scanned.is_test {
                        found.extend(
                            module_files(path, &decl.name)
                                .into_iter()
                                .filter(|child| self.files.contains_key(child)),
                        );
                    }
                }
            }
            let before = self
                .files
                .values()
                .filter(|scanned| scanned.is_test)
                .count();
            for child in found {
                if let Some(scanned) = self.files.get_mut(&child) {
                    scanned.is_test = true;
                }
            }
            if self
                .files
                .values()
                .filter(|scanned| scanned.is_test)
                .count()
                == before
            {
                return;
            }
        }
    }
}

fn collect(root: &Path, dir: &str, files: &mut BTreeMap<String, Scanned>) -> Result<()> {
    let path = root.join(dir);
    let Ok(entries) = std::fs::read_dir(&path) else {
        return Ok(());
    };
    for entry in entries {
        let entry = entry.with_context(|| format!("list {}", path.display()))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let relative = format!("{dir}/{name}");
        if entry.file_type()?.is_dir() {
            collect(root, &relative, files)?;
        } else if name.ends_with(".rs") {
            let source = std::fs::read_to_string(entry.path())
                .with_context(|| format!("read {relative}"))?;
            let facts = scan_source(&source).with_context(|| format!("scan {relative}"))?;
            files.insert(
                relative,
                Scanned {
                    facts,
                    is_test: false,
                },
            );
        }
    }
    Ok(())
}

fn is_test_name(path: &str) -> bool {
    let stem = path
        .rsplit('/')
        .next()
        .unwrap_or(path)
        .trim_end_matches(".rs");
    matches!(stem, "tests" | "testing" | "test_support" | "golden_tests")
        || stem.ends_with("_tests")
        || path.split('/').any(|part| part == "tests")
}

/// Where `mod name;` declared in `path` can live.
fn module_files(path: &str, name: &str) -> [String; 2] {
    let (dir, file) = path.rsplit_once('/').unwrap_or(("", path));
    let stem = file.trim_end_matches(".rs");
    let base = if matches!(stem, "mod" | "lib" | "main") {
        dir.to_string()
    } else {
        format!("{dir}/{stem}")
    };
    [format!("{base}/{name}.rs"), format!("{base}/{name}/mod.rs")]
}
