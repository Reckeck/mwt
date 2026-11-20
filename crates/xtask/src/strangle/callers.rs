//! `strangle callers [<area>]`: `file:line` of every remaining call of a delegate of the area
//! (method names from `baseline.json`) and of `Books::vault()` (spec D3, Interfaces). The same
//! list the `deprecated-forwards` feature gives, without the feature.

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::Result;
use clap::Args;

use super::Workspace;
use super::baseline::Baseline;
use super::home::allowed_files;
use super::scan::UnscannedMacro;

/// The method whose calls are counted until `Books::vault()` is deleted.
const BOOKS_VAULT: &str = "vault";

#[derive(Args)]
pub struct CallersArgs {
    /// One area of `strangle.toml`; every area when absent.
    pub area: Option<String>,
    /// List each macro body that is not scanned instead of only counting them.
    #[arg(long)]
    pub notes: bool,
}

pub fn run(root: &Path, args: &CallersArgs) -> Result<()> {
    let workspace = Workspace::load(root)?;
    let baseline = Baseline::load(root)?;
    let (stdout, stderr) = report(&workspace, &baseline, args.area.as_deref(), args.notes)?;
    for line in stdout {
        println!("{line}");
    }
    for note in stderr {
        eprintln!("{note}");
    }
    Ok(())
}

const WHOLE_TREE_HINT: &str = "(list them with --notes without an area)";

/// The two streams of the command: stdout is only `file:line:` callers; the macro notes go to
/// stderr. With an area only the macros of the area's own files are counted or (`notes`) listed,
/// and the whole-tree count is added, since a macro outside the area may still hold its calls.
fn report(
    workspace: &Workspace,
    baseline: &Baseline,
    area: Option<&str>,
    notes: bool,
) -> Result<(Vec<String>, Vec<String>)> {
    let stdout = callers(workspace, baseline, area)?;
    let whole = workspace.corpus.unscanned_macros();
    let whole_count = whole.len();
    let scoped = unscanned_in_scope(workspace, area, whole)?;
    let mut stderr = if notes {
        scoped.iter().map(ToString::to_string).collect()
    } else if scoped.is_empty() {
        Vec::new()
    } else {
        vec![format!(
            "{} macro bodies not scanned (list them with `--notes`)",
            scoped.len()
        )]
    };
    if area.is_some() && whole_count > 0 {
        stderr.push(format!(
            "{whole_count} macro bodies not scanned in the whole tree {WHOLE_TREE_HINT}"
        ));
    }
    Ok((stdout, stderr))
}

fn unscanned_in_scope(
    workspace: &Workspace,
    area: Option<&str>,
    macros: Vec<UnscannedMacro>,
) -> Result<Vec<UnscannedMacro>> {
    let Some(name) = area else {
        return Ok(macros);
    };
    let index = workspace.map.area_index(name)?;
    let files = allowed_files(workspace, &workspace.map.areas[index]);
    Ok(macros
        .into_iter()
        .filter(|entry| files.contains(entry.path.as_str()))
        .collect())
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
    use crate::strangle::area_fixture::{SERVICE, original_tree};

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
            "fn user(vault: &mut UnlockedVault) {\n    vault.set_info(\"a\", info);\n}\n",
        )
        .unwrap();
        std::fs::write(
            tree.root().join(SERVICE),
            "fn inside(vault: &mut UnlockedVault) {\n    odd!(=> vault.hidden());\n}\n",
        )
        .unwrap();
        let workspace = Workspace::load(tree.root()).unwrap();
        let (stdout, stderr) = report(&workspace, &baseline, Some("loyalty"), false).unwrap();
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
        assert!(stderr[0].contains("macro bodies not scanned"));
    }

    #[test]
    fn unscanned_notes_of_an_area_cover_only_its_files_and_notes_lists_them() {
        let tree = original_tree();
        let workspace = Workspace::load(tree.root()).unwrap();
        let baseline = crate::strangle::baseline::capture(tree.root(), &workspace).unwrap();
        std::fs::write(
            tree.root().join("crates/core/src/user.rs"),
            "fn user(vault: &mut UnlockedVault) {\n    odd!(=> vault.hidden());\n}\n",
        )
        .unwrap();
        let workspace = Workspace::load(tree.root()).unwrap();
        let (_, scoped) = report(&workspace, &baseline, Some("loyalty"), false).unwrap();
        assert_eq!(
            scoped,
            [
                "1 macro bodies not scanned in the whole tree (list them with --notes without an area)"
            ],
            "the macro outside the area is counted in the whole-tree figure"
        );
        let (_, whole) = report(&workspace, &baseline, None, false).unwrap();
        assert!(
            whole[0].starts_with("1 macro bodies not scanned"),
            "{whole:?}"
        );
        let (_, listed) = report(&workspace, &baseline, None, true).unwrap();
        assert_eq!(
            listed,
            ["crates/core/src/user.rs:2: macro body not scanned"]
        );
        let (_, scoped_listed) = report(&workspace, &baseline, Some("loyalty"), true).unwrap();
        assert_eq!(
            scoped_listed,
            [
                "1 macro bodies not scanned in the whole tree (list them with --notes without an area)"
            ]
        );
    }
}
