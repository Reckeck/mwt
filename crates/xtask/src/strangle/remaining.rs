//! `strangle remaining`: the numeric ratchet over the whole tree (spec D2). Three metrics, each per
//! file, counted in the text of every `.rs` file under `remaining_dirs` (comments and tests
//! included), minus the `remaining_exclude` list of `strangle.toml` (the tool and its fixtures):
//!
//! - `vault`: whole-word occurrences of `UnlockedVault`;
//! - `forward`: the `=>` lines inside `forward! { .. }` blocks (one per delegate);
//! - `service_paths`: occurrences of `crate::service::`, `trade_core::service::` and
//!   `super::service::` (the segment `service::` after one of those three roots), outside the
//!   service dir itself. `use trade_core::{service::X}` groups are not counted.
//!
//! `remaining.json` holds the counts. `check --all` holds the tree against it (`findings`): a file
//! above its record, a new file above 0, and a file below its record are all red. `--write`
//! records the tree and refuses to raise any number.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result, bail};
use clap::Args;
use serde::{Deserialize, Serialize};

use super::map::Map;

mod count;
#[cfg(test)]
mod tests;

/// Where the record lives, relative to the workspace root.
pub const REMAINING_FILE: &str = "crates/xtask/strangle/remaining.json";

/// How many files `remaining` lists per metric.
const TOP: usize = 10;

#[derive(Args)]
pub struct RemainingArgs {
    /// Record the tree in `remaining.json`; refuses to raise any number.
    #[arg(long)]
    pub write: bool,
}

/// One metric: the count per file, files with 0 left out.
pub type PerFile = BTreeMap<String, usize>;

#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Remaining {
    pub vault: PerFile,
    pub forward: PerFile,
    pub service_paths: PerFile,
}

impl Remaining {
    /// The metrics with their names, in print order.
    fn metrics(&self) -> [(&'static str, &PerFile); 3] {
        [
            ("vault", &self.vault),
            ("forward", &self.forward),
            ("service_paths", &self.service_paths),
        ]
    }

    pub fn load(root: &Path) -> Result<Self> {
        let path = root.join(REMAINING_FILE);
        let text = std::fs::read_to_string(&path).with_context(|| {
            format!(
                "read {} (write it with `strangle remaining --write`)",
                path.display()
            )
        })?;
        serde_json::from_str(&text).with_context(|| format!("parse {}", path.display()))
    }

    fn write(&self, root: &Path) -> Result<()> {
        let path = root.join(REMAINING_FILE);
        let text = serde_json::to_string_pretty(self)? + "\n";
        std::fs::write(&path, text).with_context(|| format!("write {}", path.display()))
    }
}

pub fn run(root: &Path, args: &RemainingArgs) -> Result<()> {
    let measured = count::measure(root, &Map::load(root)?)?;
    if args.write {
        return write(root, &measured);
    }
    for (name, files) in measured.metrics() {
        print_metric(name, files);
    }
    Ok(())
}

fn print_metric(name: &str, files: &PerFile) {
    println!(
        "{name}: {} in {} file(s)",
        files.values().sum::<usize>(),
        files.len()
    );
    let mut ranked: Vec<(&String, &usize)> = files.iter().collect();
    ranked.sort_by(|left, right| right.1.cmp(left.1).then(left.0.cmp(right.0)));
    for (path, count) in ranked.into_iter().take(TOP) {
        println!("  {count:>5}  {path}");
    }
}

fn write(root: &Path, measured: &Remaining) -> Result<()> {
    if root.join(REMAINING_FILE).exists() {
        let raised = raised(measured, &Remaining::load(root)?);
        if !raised.is_empty() {
            bail!(
                "remaining: --write refuses to raise a number\n  {}",
                raised.join("\n  ")
            );
        }
    }
    measured.write(root)?;
    println!("remaining: wrote {REMAINING_FILE}");
    Ok(())
}

/// The files whose measured count is above the record (a new file's record is 0).
fn raised(measured: &Remaining, recorded: &Remaining) -> Vec<String> {
    deviations(measured, recorded)
        .into_iter()
        .filter(|deviation| deviation.measured > deviation.recorded)
        .map(|deviation| deviation.to_string())
        .collect()
}

/// The findings of `check --all`: every file that differs from the record.
pub fn findings(root: &Path, map: &Map) -> Result<Vec<String>> {
    let recorded = Remaining::load(root)?;
    let measured = count::measure(root, map)?;
    Ok(deviations(&measured, &recorded)
        .into_iter()
        .map(|deviation| format!("remaining: {deviation}"))
        .collect())
}

struct Deviation<'data> {
    metric: &'static str,
    path: &'data str,
    measured: usize,
    recorded: usize,
}

impl std::fmt::Display for Deviation<'_> {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            metric,
            path,
            measured,
            recorded,
        } = self;
        if self.recorded == 0 {
            write!(
                out,
                "{metric}: {path}: new file with {measured} (not recorded)"
            )
        } else if measured > recorded {
            write!(
                out,
                "{metric}: {path}: increased from {recorded} to {measured}"
            )
        } else {
            write!(
                out,
                "{metric}: {path}: lower ({measured} < {recorded}): run `cargo xtask strangle remaining --write`"
            )
        }
    }
}

fn deviations<'data>(
    measured: &'data Remaining,
    recorded: &'data Remaining,
) -> Vec<Deviation<'data>> {
    let mut found = Vec::new();
    for ((metric, now), (_, before)) in measured.metrics().into_iter().zip(recorded.metrics()) {
        let paths: std::collections::BTreeSet<&String> = now.keys().chain(before.keys()).collect();
        for path in paths {
            let (now, before) = (
                now.get(path).copied().unwrap_or(0),
                before.get(path).copied().unwrap_or(0),
            );
            if now != before {
                found.push(Deviation {
                    metric,
                    path,
                    measured: now,
                    recorded: before,
                });
            }
        }
    }
    found
}
