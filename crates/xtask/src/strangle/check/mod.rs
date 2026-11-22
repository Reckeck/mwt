//! `strangle check`: holds a moved area against `baseline.json` (spec D6 step 6, Interfaces):
//!
//! 1. the baseline's public methods are the `forward!` entries of the shims;
//! 2. no `impl UnlockedVault` method with logic is left in the area's files;
//! 3. no `persist()` call is left in the area's files;
//! 4. every baseline test name is still in the area (new tests are welcome, none may be dropped);
//! 5. (`--diff <base>`) the changed files are inside the task's allowed set.
//!
//! `check --all` also holds the tree against `remaining.json` (module `remaining`).
//!
//! `check <area>` runs every rule on that area. `check --all` runs rules 1-4 only on the areas that
//! have moved (a shim with the forward markers, `stubs = true`) and rule 5 once, per task (module
//! `scope`: the diff must fit one task, or the one named by `--task`): it is the CI gate, so an
//! area that has not moved is not red.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};
use clap::Args;

use super::Workspace;
use super::baseline::{self, AreaBaseline, Baseline};
use super::forward::forwarded_entries;
use super::home::{allowed_files, homes};
use super::map::Area;
use super::remaining;
use super::scope;

mod rules;
#[cfg(test)]
mod tests;

use rules::{no_logic, no_persist, outside_allowed, parity, test_names};

#[derive(Args)]
pub struct CheckArgs {
    /// One area of `strangle.toml`.
    #[arg(required_unless_present_any = ["all", "write_baseline"])]
    pub area: Option<String>,
    /// Every area that has moved.
    #[arg(long, conflicts_with = "area")]
    pub all: bool,
    /// Also fail when a file outside the allowed set changed since this revision (`git diff`).
    #[arg(long, value_name = "BASE", conflicts_with = "write_baseline")]
    pub diff: Option<String>,
    /// With `--all --diff`: the task the changed files must belong to (else: any single task).
    #[arg(long, requires = "all")]
    pub task: Option<String>,
    /// Write `baseline.json` from the tree as it is now (once, before the areas move).
    #[arg(long, conflicts_with_all = ["area", "all"])]
    pub write_baseline: bool,
}

pub fn run(root: &Path, args: &CheckArgs) -> Result<()> {
    let workspace = Workspace::load(root)?;
    if args.write_baseline {
        let baseline = baseline::capture(root, &workspace)?;
        baseline.write(root)?;
        let methods: usize = baseline.areas.values().map(|area| area.methods.len()).sum();
        println!(
            "check: wrote {} ({} areas, {methods} methods)",
            baseline::BASELINE_FILE,
            baseline.areas.len()
        );
        return Ok(());
    }
    let baseline = Baseline::load(root)?;
    let changed = args
        .diff
        .as_deref()
        .map(|base| changed_files(root, base))
        .transpose()?;
    let mut findings = findings(
        root,
        &workspace,
        &baseline,
        args.area.as_deref(),
        changed.as_deref(),
        args.task.as_deref(),
    )?;
    if args.all {
        findings.extend(remaining::findings(root, &workspace.map)?);
    }
    if findings.is_empty() {
        println!("check: ok");
        return Ok(());
    }
    bail!(
        "check: {} finding(s)\n  {}",
        findings.len(),
        findings.join("\n  ")
    )
}

/// Every finding for one area (`only`) or for the moved areas, and rule 5 for the changed files.
pub(in crate::strangle) fn findings(
    root: &Path,
    workspace: &Workspace,
    baseline: &Baseline,
    only: Option<&str>,
    changed: Option<&[String]>,
    task: Option<&str>,
) -> Result<Vec<String>> {
    let areas: Vec<&Area> = match only {
        Some(name) => vec![&workspace.map.areas[workspace.map.area_index(name)?]],
        None => workspace
            .map
            .areas
            .iter()
            .filter(|area| has_moved(root, workspace, area))
            .collect(),
    };
    let mut found = Vec::new();
    for area in &areas {
        let subject = Subject {
            root,
            workspace,
            area,
            baseline: baseline.area(&area.name)?,
        };
        found.extend(check_area(&subject)?);
    }
    if let Some(changed) = changed {
        match only {
            Some(name) => {
                let allowed: BTreeSet<String> = areas
                    .iter()
                    .flat_map(|area| allowed_files(workspace, area))
                    .collect();
                found.extend(outside_allowed(name, changed, &allowed));
            }
            None => found.extend(scope::findings(workspace, changed, task)),
        }
    }
    Ok(found)
}

/// An area has moved when it gets stubs and one of its shims carries the forward markers.
fn has_moved(root: &Path, workspace: &Workspace, area: &Area) -> bool {
    area.stubs
        && homes(&workspace.map, area).iter().any(|home| {
            std::fs::read_to_string(root.join(home.shim()))
                .is_ok_and(|text| forwarded_entries(&text).is_some())
        })
}

/// What a rule looks at.
struct Subject<'run> {
    root: &'run Path,
    workspace: &'run Workspace,
    area: &'run Area,
    baseline: &'run AreaBaseline,
}

type Rule = fn(&Subject) -> Result<Vec<String>>;

const RULES: [(&str, Rule); 4] = [
    ("parity", parity),
    ("logic", no_logic),
    ("persist", no_persist),
    ("tests", test_names),
];

/// Every finding of every rule, as `<area>: <rule>: <detail>`.
fn check_area(subject: &Subject) -> Result<Vec<String>> {
    let mut findings = Vec::new();
    for (name, rule) in RULES {
        for detail in rule(subject)? {
            findings.push(format!("{}: {name}: {detail}", subject.area.name));
        }
    }
    Ok(findings)
}

/// The files changed from `base` to `HEAD`, by path from the repository root (`git` also reads a
/// colocated jj repository).
fn changed_files(root: &Path, base: &str) -> Result<Vec<String>> {
    let output = Command::new("git")
        .args(["diff", "--name-only", &format!("{base}...HEAD")])
        .current_dir(root)
        .output()
        .context("run `git diff`")?;
    if !output.status.success() {
        bail!(
            "git diff: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_string)
        .collect())
}
