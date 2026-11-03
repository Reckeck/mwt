//! Task groups of `xtask check`, the one table every selection reads (CI jobs, mise tasks, `--exclude`).

use anyhow::{Result, bail};

use crate::platforms;

/// A group is `@name`; a member is a task name or another group. Every task of `check_tasks` must be
/// in some group (a test fails otherwise), so a new task cannot be dropped from a CI selection.
pub const GROUPS: [(&str, &[&str]); 5] = [
    ("@static", &["biome", "vue-tsc", "unit", "checks"]),
    ("@browser", &["e2e", "layout", "links"]),
    ("@frontend", &["@static", "@browser"]),
    (
        "@rust",
        &["test", "gen-types", "clippy", "fmt", "ci-scripts"],
    ),
    (
        "@platforms",
        &["platform:windows", "platform:android", "platform:macos"],
    ),
];

fn members(group: &str) -> Option<&'static [&'static str]> {
    GROUPS
        .iter()
        .find(|(name, _)| *name == group)
        .map(|(_, members)| *members)
}

/// The task names of a group, nested groups flattened.
pub fn expand_group(group: &str) -> Option<Vec<String>> {
    let mut out = Vec::new();
    for member in members(group)? {
        if member.starts_with('@') {
            out.extend(expand_group(member)?);
        } else {
            out.push((*member).to_string());
        }
    }
    Some(out)
}

/// The text under `check --help`.
pub fn help() -> String {
    let mut out = String::from("Groups:\n");
    for (name, members) in GROUPS {
        out.push_str(&format!("  {name:<11} {}\n", members.join(", ")));
    }
    out.push_str("Platforms: linux (= clippy), windows, android, macos (= platform:<name>)");
    out
}

fn expand_item(item: &str, known: &[String], out: &mut Vec<String>) -> Result<()> {
    if item.starts_with('@') {
        let Some(names) = expand_group(item) else {
            bail!("{}", unknown(item, known));
        };
        out.extend(names);
    } else if known.iter().any(|name| name == item) {
        out.push(item.to_string());
    } else if let Some(name) = platforms::task_name(item) {
        out.push(name);
    } else {
        bail!("{}", unknown(item, known));
    }
    Ok(())
}

fn unknown(item: &str, known: &[String]) -> String {
    let groups: Vec<&str> = GROUPS.iter().map(|(name, _)| *name).collect();
    format!(
        "unknown task or group `{item}`\n  tasks:  {}\n  groups: {}\n  platforms: {}",
        known.join(", "),
        groups.join(", "),
        platforms::PLATFORMS.join(", ")
    )
}

/// The task names a command line asks for, before dependencies are pulled in.
///
/// `items` (names, `@groups`, platform names) are united; empty means every task. `exclude` is then
/// taken away from that list. Dependencies of what remains are still pulled in later (`graph::select`),
/// so excluding `test` does not drop it while `gen-types` is kept.
pub fn choose(known: &[String], items: &[String], exclude: &[String]) -> Result<Vec<String>> {
    let mut wanted = Vec::new();
    for item in items {
        expand_item(item, known, &mut wanted)?;
    }
    if items.is_empty() {
        wanted = known.to_vec();
    }
    let mut removed = Vec::new();
    for item in exclude {
        expand_item(item, known, &mut removed)?;
    }
    let mut chosen: Vec<String> = Vec::new();
    for name in wanted {
        if !removed.contains(&name) && !chosen.contains(&name) {
            chosen.push(name);
        }
    }
    if chosen.is_empty() {
        bail!("nothing is left to run after --exclude");
    }
    Ok(chosen)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::checks::{BrowserOpts, check_tasks};
    use crate::graph::select;

    fn names() -> Vec<String> {
        check_tasks(Path::new("."), false, &BrowserOpts::default())
            .iter()
            .map(|task| task.name.clone())
            .collect()
    }

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| (*item).to_string()).collect()
    }

    #[test]
    fn every_task_is_in_a_group() {
        let grouped: Vec<String> = GROUPS
            .iter()
            .filter_map(|(name, _)| expand_group(name))
            .flatten()
            .collect();
        for name in names() {
            assert!(grouped.contains(&name), "task `{name}` is in no group");
        }
    }

    #[test]
    fn groups_name_only_known_tasks_and_groups() {
        let known = names();
        for (group, members) in GROUPS {
            for member in members {
                let ok = if member.starts_with('@') {
                    self::members(member).is_some()
                } else {
                    known.iter().any(|name| name == member)
                };
                assert!(ok, "group {group} names unknown `{member}`");
            }
        }
    }

    #[test]
    fn items_unite_and_platform_names_work() {
        let got = choose(
            &names(),
            &strings(&["@static", "e2e", "windows", "linux"]),
            &[],
        )
        .unwrap();
        assert_eq!(
            got,
            [
                "biome",
                "vue-tsc",
                "unit",
                "checks",
                "e2e",
                "platform:windows",
                "clippy"
            ]
        );
    }

    #[test]
    fn exclude_removes_from_the_selection_and_dependencies_come_back() {
        let known = names();
        let got = choose(&known, &[], &strings(&["@frontend", "@platforms"])).unwrap();
        assert_eq!(got, ["test", "gen-types", "clippy", "ci-scripts", "fmt"]);
        // gen-types needs test: excluding test alone does not drop it from the run.
        let picked = choose(&known, &strings(&["@rust"]), &strings(&["test"])).unwrap();
        assert!(!picked.contains(&"test".to_string()));
        let tasks = select(
            check_tasks(Path::new("."), false, &BrowserOpts::default()),
            &picked,
        )
        .unwrap();
        assert!(tasks.iter().any(|task| task.name == "test"));
    }

    #[test]
    fn unknown_and_empty_selections_are_errors() {
        let known = names();
        let error = choose(&known, &strings(&["@nope"]), &[])
            .unwrap_err()
            .to_string();
        assert!(error.contains("@nope") && error.contains("@static") && error.contains("biome"));
        assert!(choose(&known, &strings(&["nope"]), &[]).is_err());
        assert!(choose(&known, &strings(&["fmt"]), &strings(&["fmt"])).is_err());
    }

    /// Mirrors the selections of ci.yml (check-linux, frontend-static, frontend-e2e, frontend-layout, the
    /// platform jobs).
    #[test]
    fn the_ci_selections_cover_every_task() {
        let known = names();
        let mut covered = choose(&known, &[], &strings(&["@frontend", "@platforms"])).unwrap();
        covered.extend(choose(&known, &strings(&["@static"]), &[]).unwrap());
        covered.extend(
            choose(
                &known,
                &strings(&["@browser"]),
                &strings(&["layout", "links"]),
            )
            .unwrap(),
        );
        covered.extend(choose(&known, &strings(&["layout", "links"]), &[]).unwrap());
        covered.extend(choose(&known, &strings(&["android", "macos", "windows"]), &[]).unwrap());
        for name in &known {
            assert!(covered.contains(name), "`{name}` is run by no CI job");
        }
    }
}
