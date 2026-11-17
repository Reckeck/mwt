//! Rule 5 of `strangle check --all`: the changed files belong to one task (spec D5). A file is
//! allowed for the tasks whose area files, regions, `ipc_contract` stub, grants or foundation files
//! name it; the diff passes when one task allows every file it changes in `service/`, the group
//! dirs and `usecases/`, and in the `guarded` paths. Other files are left to the task's own `files:` list.

use std::collections::BTreeSet;

use super::Workspace;
use super::home::{allowed_files, core_dir};
use super::map::Foundation;

/// The findings for `changed`: every file no task allows, and a diff that no single task covers.
/// With `task` the diff must fit that task alone.
pub fn findings(workspace: &Workspace, changed: &[String], task: Option<&str>) -> Vec<String> {
    let owners = Owners::of(workspace);
    let mut found = Vec::new();
    let mut shared: Option<BTreeSet<&str>> = task.map(|name| BTreeSet::from([name]));
    let mut claims = Vec::new();
    for path in changed.iter().filter(|path| owners.in_scope(path)) {
        let tasks = owners.tasks_of(path);
        if tasks.is_empty() || task.is_some_and(|name| !tasks.contains(name)) {
            found.push(format!("all: files: {path} is outside the allowed set"));
            continue;
        }
        shared = Some(match shared {
            Some(common) => common.intersection(&tasks).copied().collect(),
            None => tasks.clone(),
        });
        claims.push(format!("{path} ({})", join(&tasks)));
    }
    if shared.as_ref().is_some_and(BTreeSet::is_empty) {
        found.push(format!(
            "all: files: no single task allows every changed file: {}",
            claims.join(", ")
        ));
    }
    found
}

fn join(tasks: &BTreeSet<&str>) -> String {
    tasks.iter().copied().collect::<Vec<_>>().join("/")
}

/// Which tasks may change which file, and which files the rule looks at.
struct Owners<'run> {
    areas: Vec<(&'run str, BTreeSet<String>)>,
    foundations: &'run [Foundation],
    scope: Vec<String>,
}

impl<'run> Owners<'run> {
    fn of(workspace: &'run Workspace) -> Self {
        let map = &workspace.map;
        let core = core_dir(map);
        let mut scope = vec![format!("{}/", map.service_dir), format!("{core}/usecases/")];
        scope.extend(
            map.areas
                .iter()
                .map(|area| format!("{core}/{}/", area.group)),
        );
        scope.extend(
            map.areas
                .iter()
                .flat_map(|area| area.homes.values())
                .map(|group| format!("{core}/{group}/")),
        );
        scope.extend(map.guarded.iter().cloned());
        Self {
            areas: map
                .areas
                .iter()
                .map(|area| (area.task.as_str(), allowed_files(workspace, area)))
                .collect(),
            foundations: &map.foundations,
            scope,
        }
    }

    fn in_scope(&self, path: &str) -> bool {
        self.scope.iter().any(|prefix| path.starts_with(prefix))
    }

    fn tasks_of(&self, path: &str) -> BTreeSet<&str> {
        let by_area = self
            .areas
            .iter()
            .filter(|(_, files)| files.contains(path))
            .map(|(task, _)| *task);
        let by_foundation = self
            .foundations
            .iter()
            .filter(|foundation| {
                foundation.files.iter().any(|entry| {
                    if entry.ends_with('/') {
                        path.starts_with(entry)
                    } else {
                        path == entry
                    }
                })
            })
            .map(|foundation| foundation.task.as_str());
        by_area.chain(by_foundation).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn real_workspace() -> Workspace {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        Workspace::load(&root).unwrap()
    }

    fn changed(paths: &[&str]) -> Vec<String> {
        paths.iter().map(|path| (*path).to_string()).collect()
    }

    #[test]
    fn a_diff_over_two_areas_is_red_without_a_task() {
        let found = findings(
            &real_workspace(),
            &changed(&[
                "crates/core/src/service/loyalty.rs",
                "crates/core/src/service/rates.rs",
            ]),
            None,
        );
        assert!(
            found
                .iter()
                .any(|line| line.contains("no single task allows every changed file")),
            "{found:?}"
        );
    }

    #[test]
    fn a_guarded_file_in_an_area_diff_is_red() {
        let found = findings(
            &real_workspace(),
            &changed(&[
                "crates/core/src/service/loyalty.rs",
                "crates/core/src/lib.rs",
                "Cargo.lock",
                "README.md",
            ]),
            None,
        );
        assert!(
            found.iter().any(|line| line.contains("no single task")),
            "{found:?}"
        );
        let named = findings(
            &real_workspace(),
            &changed(&["crates/core/src/lib.rs", "crates/app/src/main.rs"]),
            Some("T26"),
        );
        assert_eq!(named.len(), 2, "{named:?}");
    }

    #[test]
    fn a_guarded_file_is_green_for_its_foundation_task() {
        let found = findings(
            &real_workspace(),
            &changed(&["crates/core/src/lib.rs", "Cargo.lock", "README.md"]),
            Some("T6"),
        );
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn a_pilot_command_file_is_green_for_t13_and_red_for_an_area_task() {
        let file = changed(&["crates/app/src/commands/planning.rs"]);
        let owner = findings(&real_workspace(), &file, Some("T13"));
        assert!(owner.is_empty(), "{owner:?}");
        let other = findings(&real_workspace(), &file, Some("T26"));
        assert_eq!(other.len(), 1, "{other:?}");
    }

    #[test]
    fn a_diff_inside_one_area_is_green() {
        let found = findings(
            &real_workspace(),
            &changed(&["crates/core/src/service/loyalty.rs"]),
            None,
        );
        assert!(found.is_empty(), "{found:?}");
    }
}
