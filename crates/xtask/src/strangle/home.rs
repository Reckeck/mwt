//! Where one area file lives before and after the move: the shim in `service/`, the pure module
//! `<group>/<stem>.rs`, and the scenario module `usecases/<context>/<stem>.rs` (spec D1). One home
//! for those paths, shared by `relocate`, `forward`, `check` and the baseline.

use std::collections::BTreeSet;
use std::path::Path;

use super::Workspace;
use super::map::{Area, Map};
use super::scaffold::stems;

/// One file stem of an area, with every path it owns.
pub struct Home {
    /// The name of the new home (`overview`); it names the pure and scenario modules.
    pub stem: String,
    /// The map entry's own name (`reports`): the old file under `service/`, whatever the stem.
    source: String,
    pub group: String,
    pub context: String,
    service_dir: String,
    core_dir: String,
}

impl Home {
    /// `service/<source>.rs`: the old file, then the shim.
    pub fn shim(&self) -> String {
        format!("{}/{}.rs", self.service_dir, self.source)
    }

    /// The old module name under `service/` (`reports`).
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Whether the stem renames the map entry.
    pub fn is_renamed(&self) -> bool {
        self.source != self.stem
    }

    /// `<group>/<stem>.rs`: the pure functions (and the relocated file).
    pub fn pure_file(&self) -> String {
        format!("{}/{}/{}.rs", self.core_dir, self.group, self.stem)
    }

    /// `usecases/<context>/<stem>.rs`: the scenarios.
    pub fn scenario_file(&self) -> String {
        format!(
            "{}/usecases/{}/{}.rs",
            self.core_dir, self.context, self.stem
        )
    }

    /// The test files that belong to the stem, in the service dir and in the group.
    pub fn test_files(&self) -> [String; 2] {
        [
            format!("{}/{}_tests.rs", self.service_dir, self.source),
            format!("{}/{}/{}_tests.rs", self.core_dir, self.group, self.stem),
        ]
    }

    /// Every file of the stem; the ones that do not exist are skipped by the readers.
    pub fn files(&self) -> Vec<String> {
        let [service_tests, group_tests] = self.test_files();
        vec![
            self.shim(),
            self.pure_file(),
            self.scenario_file(),
            service_tests,
            group_tests,
        ]
    }

    /// `service/<source>/<child>.rs`: an out-of-line child module of the old file.
    pub fn child_source(&self, child: &str) -> String {
        format!("{}/{}/{child}.rs", self.service_dir, self.source)
    }

    /// `service/<source>/mod.rs`: the entry as a directory module.
    pub fn directory_module(&self) -> String {
        format!("{}/{}/mod.rs", self.service_dir, self.source)
    }

    /// `<group>/<stem>/<child>.rs`: where a child module lands next to the pure file.
    pub fn child_target(&self, child: &str) -> String {
        format!("{}/{}/{}/{child}.rs", self.core_dir, self.group, self.stem)
    }

    /// The directories of the stem whose every file belongs to it: the children of the old file
    /// and of the pure and scenario modules.
    fn directories(&self) -> [String; 3] {
        [
            format!("{}/{}/", self.service_dir, self.source),
            format!("{}/{}/{}/", self.core_dir, self.group, self.stem),
            format!("{}/usecases/{}/{}/", self.core_dir, self.context, self.stem),
        ]
    }

    /// `crate::<group>::<stem>`.
    pub fn pure_path(&self) -> String {
        format!("crate::{}::{}", self.group, self.stem)
    }

    /// `<group>::<stem>`: `pure_path` without its `crate::`.
    pub fn pure_path_tail(&self) -> String {
        format!("{}::{}", self.group, self.stem)
    }

    /// `crate::usecases::<context>::<stem>`.
    pub fn scenario_path(&self) -> String {
        format!("crate::usecases::{}::{}", self.context, self.stem)
    }
}

/// The directory that holds `service/`: the core crate's `src`.
pub(super) fn core_dir(map: &Map) -> String {
    Path::new(&map.service_dir)
        .parent()
        .map_or_else(String::new, |dir| dir.to_string_lossy().into_owned())
}

pub fn homes(map: &Map, area: &Area) -> Vec<Home> {
    let core = core_dir(map);
    stems(area)
        .into_iter()
        .map(|stem| Home {
            source: source_of(area, &stem),
            group: area.homes.get(&stem).unwrap_or(&area.group).clone(),
            context: area.context.clone(),
            service_dir: map.service_dir.clone(),
            core_dir: core.clone(),
            stem,
        })
        .collect()
}

/// The old file name of a stem: the map entry it renames, without `.rs` or a trailing `/`.
fn source_of(area: &Area, stem: &str) -> String {
    area.stem
        .iter()
        .find(|(_, renamed)| renamed.as_str() == stem)
        .map_or_else(
            || stem.to_string(),
            |(entry, _)| {
                entry
                    .trim_end_matches('/')
                    .trim_end_matches(".rs")
                    .to_string()
            },
        )
}

/// Every file of the area, sorted: the whole-file spans of the map (so the files under
/// `service/<dir>/` too), the paths of each stem, and the scanned files in the stems' directories.
pub fn area_files(workspace: &Workspace, area: &Area) -> Vec<String> {
    let map = &workspace.map;
    let index = map.area_index(&area.name).ok();
    let mut files: BTreeSet<String> = workspace
        .layout
        .spans()
        .iter()
        .filter(|span| span.whole && Some(span.area) == index)
        .map(|span| format!("{}/{}", map.service_dir, span.file))
        .collect();
    for home in homes(map, area) {
        files.extend(home.files());
        for directory in home.directories() {
            files.extend(
                workspace
                    .corpus
                    .under(&directory)
                    .map(|(path, _)| path.to_string()),
            );
        }
    }
    files.into_iter().collect()
}

/// The files a task of the area may change (spec D5): the area files, its `ipc_contract` stub,
/// the shared files its regions cut from, and the grants of every area task.
pub fn allowed_files(workspace: &Workspace, area: &Area) -> BTreeSet<String> {
    let map = &workspace.map;
    let mut allowed: BTreeSet<String> = area_files(workspace, area).into_iter().collect();
    if area.ipc_contract {
        allowed.insert(format!("{}/ipc_contract/{}.rs", core_dir(map), area.name));
    }
    allowed.extend(map.grants.iter().cloned());
    allowed.extend(
        area.regions
            .iter()
            .map(|region| format!("{}/{}", map.service_dir, region.file)),
    );
    allowed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_leaves_stems_resolve_to_their_target_structure_groups() {
        let map = Map::parse(include_str!("../../strangle.toml")).unwrap();
        let leaves = &map.areas[map.area_index("leaves").unwrap()];
        let groups: std::collections::BTreeMap<String, String> = homes(&map, leaves)
            .into_iter()
            .map(|home| (home.stem, home.group))
            .collect();
        for (stem, group) in [
            ("categories", "money"),
            ("ledger", "money"),
            ("validate", "money"),
            ("store_months", "money"),
            ("month_balance", "money"),
            ("search", "money"),
            ("envelopes", "planning"),
            ("vehicle", "household"),
            ("losses", "household"),
            ("relations", "household"),
        ] {
            assert_eq!(groups[stem], group, "{stem}");
        }
    }
}
