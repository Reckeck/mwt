//! `strangle stats`: the facts of spec table F2, per area, from the scanned sources.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use anyhow::Result;
use clap::Args;
use serde::Serialize;

use super::Workspace;
use super::map::{Area, Map};

/// Calls that every area makes and that say nothing about coupling.
const PLUMBING: [&str; 2] = ["persist", "transact"];

#[derive(Args)]
pub struct StatsArgs {
    /// One area of `strangle.toml`.
    #[arg(required_unless_present = "all")]
    pub area: Option<String>,
    /// Every area, as a table.
    #[arg(long, conflicts_with = "area")]
    pub all: bool,
    /// Machine-readable output.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Serialize)]
pub struct MethodInfo {
    pub name: String,
    pub mutating: bool,
}

#[derive(Debug, Default, Serialize)]
pub struct AreaStats {
    pub name: String,
    pub task: String,
    pub group: String,
    pub context: String,
    pub homes: BTreeMap<String, String>,
    pub files: usize,
    pub lines: usize,
    /// Lines before the first inline test module, rounded to ten.
    pub prod_lines: usize,
    pub pub_methods: usize,
    pub hidden_methods: usize,
    pub persist: usize,
    pub commands_first: usize,
    pub commands_any: usize,
    pub calls_out: usize,
    pub areas_out: usize,
    pub calls_in: usize,
    pub areas_in: usize,
    /// Public methods whose name appears in no test.
    pub untested: Vec<String>,
    pub pub_list: Vec<MethodInfo>,
}

#[derive(Debug, Default, Serialize)]
pub struct Totals {
    /// Public methods of every `impl UnlockedVault` in the service dir, mapped or not.
    pub pub_methods: usize,
    /// The part of `pub_methods` that no area owns.
    pub unmapped_pub_methods: usize,
    pub commands: usize,
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub areas: Vec<AreaStats>,
    pub totals: Totals,
}

pub fn run(workspace: &Workspace, args: &StatsArgs) -> Result<()> {
    let mut report = compute(workspace);
    if let Some(name) = &args.area {
        let index = workspace.map.area_index(name)?;
        report.areas = vec![report.areas.swap_remove(index)];
    }
    let text = match (args.json, args.area.is_some()) {
        (true, _) => serde_json::to_string_pretty(&report)? + "\n",
        (false, true) => detail(&report.areas[0]),
        (false, false) => table(&report),
    };
    print!("{text}");
    Ok(())
}

/// Per-area accumulators, filled by one pass over each kind of fact.
struct Pass<'map> {
    map: &'map Map,
    stats: Vec<AreaStats>,
    areas_out: Vec<BTreeSet<usize>>,
    areas_in: Vec<BTreeSet<usize>>,
}

pub fn compute(workspace: &Workspace) -> Report {
    let map = &workspace.map;
    let mut pass = Pass {
        map,
        stats: map.areas.iter().map(blank).collect(),
        areas_out: vec![BTreeSet::new(); map.areas.len()],
        areas_in: vec![BTreeSet::new(); map.areas.len()],
    };
    let mut totals = Totals::default();
    pass.sizes(workspace);
    let owners = pass.methods(workspace, &mut totals);
    pass.calls(workspace, &owners);
    pass.commands(workspace, &owners, &mut totals);
    pass.untested(workspace);
    for (index, stat) in pass.stats.iter_mut().enumerate() {
        stat.prod_lines = (stat.prod_lines + 5) / 10 * 10;
        stat.areas_out = pass.areas_out[index].len();
        stat.areas_in = pass.areas_in[index].len();
    }
    Report {
        areas: pass.stats,
        totals,
    }
}

fn blank(area: &Area) -> AreaStats {
    AreaStats {
        name: area.name.clone(),
        task: area.task.clone(),
        group: area.group.clone(),
        context: area.context.clone(),
        homes: area.homes.clone(),
        ..AreaStats::default()
    }
}

impl Pass<'_> {
    /// files, lines and prod lines: per span, a test file has no prod lines.
    fn sizes(&mut self, workspace: &Workspace) {
        for span in workspace.layout.spans() {
            let Some(scanned) = workspace.service_file(&span.file) else {
                continue;
            };
            let last = span.to.min(scanned.facts.lines);
            let stat = &mut self.stats[span.area];
            stat.files += usize::from(span.whole);
            stat.lines += (last + 1).saturating_sub(span.from);
            if !scanned.is_test {
                let prod_end = scanned.facts.first_test_line.map_or(last, |line| line - 1);
                stat.prod_lines += (last.min(prod_end) + 1).saturating_sub(span.from);
            }
        }
    }

    /// pub and hidden methods and `persist()`; returns the owner area of every method name.
    fn methods(&mut self, workspace: &Workspace, totals: &mut Totals) -> HashMap<String, usize> {
        let mut owners = HashMap::new();
        for (file, scanned) in workspace.service_files() {
            let prod = !scanned.is_test;
            for method in scanned
                .facts
                .methods
                .iter()
                .filter(|method| prod && !method.in_test)
            {
                totals.pub_methods += usize::from(method.public);
                let Some(area) = workspace.layout.area_of(file, method.line) else {
                    totals.unmapped_pub_methods += usize::from(method.public);
                    continue;
                };
                owners.entry(method.name.clone()).or_insert(area);
                let stat = &mut self.stats[area];
                if method.public {
                    stat.pub_methods += 1;
                    stat.pub_list.push(MethodInfo {
                        name: method.name.clone(),
                        mutating: method.mutating,
                    });
                } else {
                    stat.hidden_methods += 1;
                }
            }
        }
        owners
    }

    /// persist(), and the calls that cross from one area into another.
    fn calls(&mut self, workspace: &Workspace, owners: &HashMap<String, usize>) {
        for (file, scanned) in workspace
            .service_files()
            .filter(|(_, scanned)| !scanned.is_test)
        {
            for call in scanned.facts.calls.iter().filter(|call| !call.in_test) {
                let Some(from) = workspace.layout.area_of(file, call.line) else {
                    continue;
                };
                if call.name == "persist" {
                    self.stats[from].persist += 1;
                }
                let Some(&to) = owners.get(&call.name) else {
                    continue;
                };
                if call.on_vault && to != from && !PLUMBING.contains(&call.name.as_str()) {
                    self.stats[from].calls_out += 1;
                    self.stats[to].calls_in += 1;
                    self.areas_out[from].insert(to);
                    self.areas_in[to].insert(from);
                }
            }
        }
    }

    /// Commands whose body names a method of the area: the first one decides, any one counts.
    fn commands(
        &mut self,
        workspace: &Workspace,
        owners: &HashMap<String, usize>,
        totals: &mut Totals,
    ) {
        let prefix = format!("{}/", self.map.command_dir);
        for (_, scanned) in workspace
            .corpus
            .under(&prefix)
            .filter(|(_, scanned)| !scanned.is_test)
        {
            for names in &scanned.facts.commands {
                totals.commands += 1;
                let hit: Vec<usize> = names
                    .iter()
                    .filter_map(|name| owners.get(name).copied())
                    .collect();
                if let Some(&first) = hit.first() {
                    self.stats[first].commands_first += 1;
                }
                for area in hit.into_iter().collect::<BTreeSet<_>>() {
                    self.stats[area].commands_any += 1;
                }
            }
        }
    }

    fn untested(&mut self, workspace: &Workspace) {
        let tested = workspace.corpus.test_idents();
        for stat in &mut self.stats {
            stat.untested = stat
                .pub_list
                .iter()
                .filter(|method| !tested.contains(method.name.as_str()))
                .map(|method| method.name.clone())
                .collect();
        }
    }
}

const HEADER: &str = "area                 task   files  lines  prod  pub hidden persist cmd-first cmd-any  out areas   in areas untested";

fn row(stat: &AreaStats) -> String {
    format!(
        "{:<20} {:<6} {:>5} {:>6} {:>5} {:>4} {:>6} {:>7} {:>9} {:>7} {:>4} {:>5} {:>4} {:>5} {:>8}",
        stat.name,
        stat.task,
        stat.files,
        stat.lines,
        stat.prod_lines,
        stat.pub_methods,
        stat.hidden_methods,
        stat.persist,
        stat.commands_first,
        stat.commands_any,
        stat.calls_out,
        stat.areas_out,
        stat.calls_in,
        stat.areas_in,
        stat.untested.len(),
    )
}

pub fn table(report: &Report) -> String {
    let mut text = format!("{HEADER}\n");
    for stat in &report.areas {
        text.push_str(&row(stat));
        text.push('\n');
    }
    let totals = &report.totals;
    text.push_str(&format!(
        "total: {} pub methods over service/ ({} in no area), {} commands\n",
        totals.pub_methods, totals.unmapped_pub_methods, totals.commands
    ));
    text
}

/// One area: its row, then the public methods with their receiver and the ones no test names.
pub fn detail(stat: &AreaStats) -> String {
    let mut text = format!(
        "{HEADER}\n{}\n\ngroup {} (usecases/{}), task {}\n",
        row(stat),
        stat.group,
        stat.context,
        stat.task
    );
    text.push_str("\npublic methods (&self = query, &mut self = command):\n");
    for method in &stat.pub_list {
        let receiver = if method.mutating {
            "&mut self"
        } else {
            "&self"
        };
        text.push_str(&format!("  {receiver:<9} {}\n", method.name));
    }
    text.push_str(&format!("\nnamed in no test ({}):\n", stat.untested.len()));
    for name in &stat.untested {
        text.push_str(&format!("  {name}\n"));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strangle::fixture;

    #[test]
    fn counts_follow_the_area_map_on_a_fixture_tree() {
        let workspace = fixture::workspace();
        let report = compute(&workspace);
        let alpha = &report.areas[0];
        assert_eq!(alpha.name, "alpha");
        assert_eq!(
            (alpha.files, alpha.pub_methods, alpha.hidden_methods),
            (2, 2, 2)
        );
        assert_eq!(alpha.persist, 1);
        assert_eq!(alpha.untested, ["quiet"]);
        assert_eq!((alpha.calls_out, alpha.areas_out), (1, 1));
        assert_eq!((alpha.commands_first, alpha.commands_any), (1, 1));
        let beta = &report.areas[1];
        assert_eq!((beta.calls_in, beta.areas_in), (1, 1));
        assert_eq!((beta.commands_first, beta.commands_any), (1, 2));
        assert_eq!(report.totals.commands, 2);
        assert_eq!(report.totals.pub_methods, 4);
        assert_eq!(report.totals.unmapped_pub_methods, 1);
    }

    #[test]
    fn lines_and_prod_lines_stop_at_the_inline_test_module() {
        let report = compute(&fixture::workspace());
        let alpha = &report.areas[0];
        // alpha.rs is 14 lines with tests from line 10; alpha_tests.rs is all test.
        assert_eq!(alpha.lines, 14 + 3);
        assert_eq!(alpha.prod_lines, 10);
    }

    #[test]
    fn the_region_of_a_shared_file_is_counted_on_its_own() {
        let report = compute(&fixture::workspace());
        let beta = &report.areas[1];
        assert_eq!(
            (beta.files, beta.pub_methods, beta.hidden_methods),
            (0, 1, 0)
        );
    }

    #[test]
    fn json_output_parses_back() {
        let report = compute(&fixture::workspace());
        let json = serde_json::to_string_pretty(&report).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["areas"][0]["name"], "alpha");
        assert_eq!(value["totals"]["commands"], 2);
    }

    #[test]
    fn the_table_has_a_row_per_area_and_a_total() {
        let text = table(&compute(&fixture::workspace()));
        assert!(text.lines().any(|line| line.starts_with("alpha ")));
        assert!(text.contains("total: 4 pub methods"));
        let detail_text = detail(&compute(&fixture::workspace()).areas[0]);
        assert!(detail_text.contains("&mut self"));
        assert!(detail_text.contains("quiet"));
    }

    /// The spec counted 496 public methods over `service/` and 402 commands. Moves only shrink the
    /// first number (a `forward!` entry is not a method); new commits may grow either by a little.
    #[test]
    fn the_real_tree_is_close_to_the_numbers_of_the_spec() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let workspace = Workspace::load(&root).unwrap();
        let report = compute(&workspace);
        assert!(
            (1..=496 + 25).contains(&report.totals.pub_methods),
            "{}",
            report.totals.pub_methods
        );
        assert!(
            (402..=402 + 25).contains(&report.totals.commands),
            "{}",
            report.totals.commands
        );
        assert_eq!(report.totals.unmapped_pub_methods, 0);
        // A moved area (loyalty) leaves a shim with no commands of its own: look at the tree as a whole.
        assert!(
            report
                .areas
                .iter()
                .any(|area| area.lines > 0 && area.commands_any >= 4)
        );
    }
}
