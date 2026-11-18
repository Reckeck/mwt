//! `strangle callers [<area>]`: `file:line` of every remaining call of a delegate of the area
//! (method names from `baseline.json`) and of `Books::vault()` (spec D3, Interfaces). The same
//! list the `deprecated-forwards` feature gives, without the feature.

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::Result;
use clap::Args;

use super::Workspace;
use super::baseline::Baseline;

/// The method whose calls are counted until `Books::vault()` is deleted.
const BOOKS_VAULT: &str = "vault";

#[derive(Args)]
pub struct CallersArgs {
    /// One area of `strangle.toml`; every area when absent.
    pub area: Option<String>,
}

pub fn run(root: &Path, args: &CallersArgs) -> Result<()> {
    let workspace = Workspace::load(root)?;
    let baseline = Baseline::load(root)?;
    let (stdout, stderr) = report(&workspace, &baseline, args.area.as_deref())?;
    for line in stdout {
        println!("{line}");
    }
    if let Some(note) = stderr {
        eprintln!("{note}");
    }
    Ok(())
}

/// The two streams of the command: stdout is only `file:line:` callers; the macro notes are one
/// count for stderr (the list is in `strangle stats`).
fn report(
    workspace: &Workspace,
    baseline: &Baseline,
    area: Option<&str>,
) -> Result<(Vec<String>, Option<String>)> {
    let stdout = callers(workspace, baseline, area)?;
    let unscanned = workspace.corpus.unscanned_notes().len();
    let stderr = (unscanned > 0).then(|| {
        format!("{unscanned} macro bodies not scanned (see `strangle stats` for the list)")
    });
    Ok((stdout, stderr))
}

/// One `path:line: name()` entry per call, sorted.
fn callers(workspace: &Workspace, baseline: &Baseline, area: Option<&str>) -> Result<Vec<String>> {
    let mut names: BTreeSet<&str> = BTreeSet::new();
    match area {
        Some(name) => names.extend(delegate_names(baseline, name)?),
        None => {
            for name in baseline.areas.keys() {
                names.extend(delegate_names(baseline, name)?);
            }
        }
    }
    let mut found = BTreeSet::new();
    for (path, scanned) in workspace.corpus.under("") {
        for call in &scanned.facts.calls {
            if names.contains(call.name.as_str()) {
                found.insert(format!("{path}:{}: {}()", call.line, call.name));
            } else if call.name == BOOKS_VAULT {
                found.insert(format!("{path}:{}: Books::vault()", call.line));
            }
        }
    }
    Ok(found.into_iter().collect())
}

fn delegate_names<'baseline>(
    baseline: &'baseline Baseline,
    area: &str,
) -> Result<impl Iterator<Item = &'baseline str>> {
    Ok(baseline
        .area(area)?
        .methods
        .iter()
        .map(|method| method.name.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strangle::area_fixture::original_tree;

    #[test]
    fn calls_of_delegates_and_of_books_vault_are_listed_with_their_line() {
        let tree = original_tree();
        let workspace = Workspace::load(tree.root()).unwrap();
        let baseline = crate::strangle::baseline::capture(tree.root(), &workspace).unwrap();
        std::fs::write(
            tree.root().join("crates/core/src/user.rs"),
            "fn user(vault: &mut UnlockedVault, books: Books) {\n    vault.set_info(\"a\", info);\n    let other = books.vault();\n    vault.unrelated();\n}\n",
        )
        .unwrap();
        let workspace = Workspace::load(tree.root()).unwrap();
        let listed = callers(&workspace, &baseline, Some("loyalty")).unwrap();
        assert_eq!(
            listed,
            [
                "crates/core/src/user.rs:2: set_info()",
                "crates/core/src/user.rs:3: Books::vault()",
            ]
        );
        assert!(callers(&workspace, &baseline, Some("nope")).is_err());
    }

    #[test]
    fn listed_lines_are_only_file_line_callers_even_with_unscanned_macros() {
        let tree = original_tree();
        let workspace = Workspace::load(tree.root()).unwrap();
        let baseline = crate::strangle::baseline::capture(tree.root(), &workspace).unwrap();
        std::fs::write(
            tree.root().join("crates/core/src/user.rs"),
            "fn user(vault: &mut UnlockedVault) {\n    odd!(=> vault.hidden());\n    vault.set_info(\"a\", info);\n}\n",
        )
        .unwrap();
        let workspace = Workspace::load(tree.root()).unwrap();
        let (stdout, stderr) = report(&workspace, &baseline, Some("loyalty")).unwrap();
        assert!(!stdout.is_empty());
        for line in &stdout {
            let (location, _) = line.split_once(": ").unwrap();
            let (file, number) = location.rsplit_once(':').unwrap();
            assert!(
                file.ends_with(".rs") && number.parse::<usize>().is_ok(),
                "{line}"
            );
            assert!(!line.contains("macro"), "{line}");
        }
        assert!(stderr.unwrap().contains("macro bodies not scanned"));
    }
}
