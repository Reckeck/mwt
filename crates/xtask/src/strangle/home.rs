//! Where one area file lives before and after the move: the shim in `service/`, the pure module
//! `<group>/<stem>.rs`, and the scenario module `usecases/<context>/<stem>.rs` (spec D1). One home
//! for those paths, shared by `relocate`, `forward`, `check` and the baseline.

use std::collections::BTreeSet;
use std::path::Path;

use super::Workspace;
use super::map::{Area, Map, entry_name};
use super::scaffold::stems;

/// The scenario module group under the core crate (`usecases/<context>/<stem>.rs`).
pub const USECASES: &str = "usecases";

/// The delegate-target marker for a `VaultDoc` receiver: the `@doc` argument.
pub const DOC_MARKER: &str = "@doc";

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
    /// The directory of the group's modules: the map's `group_dirs` entry, else `<core_dir>/<group>`.
    group_dir: String,
    /// The group's `facade_test_dirs` entry, when it has one.
    facade_test_dir: Option<String>,
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
        format!("{}/{}.rs", self.group_dir, self.stem)
    }

    /// `usecases/<context>/<stem>.rs`: the scenarios.
    pub fn scenario_file(&self) -> String {
        format!(
            "{}/{USECASES}/{}/{}.rs",
            self.core_dir, self.context, self.stem
        )
    }

    /// The test files that belong to the stem, in the service dir and in the group.
    /// A group with a facade test dir adds `<dir>/<stem>.rs` and `<dir>/<stem>_tests.rs`.
    pub fn test_files(&self) -> Vec<String> {
        let mut files = vec![
            format!("{}/{}_tests.rs", self.service_dir, self.source),
            format!("{}/{}_tests.rs", self.group_dir, self.stem),
        ];
        if let Some(dir) = &self.facade_test_dir {
            files.push(format!("{dir}/{}.rs", self.stem));
            files.push(format!("{dir}/{}_tests.rs", self.stem));
        }
        files
    }

    /// Every file of the stem; the ones that do not exist are skipped by the readers.
    pub fn files(&self) -> Vec<String> {
        let mut files = vec![self.shim(), self.pure_file(), self.scenario_file()];
        files.extend(self.test_files());
        files
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
        format!("{}/{}/{child}.rs", self.group_dir, self.stem)
    }

    /// The directories of the stem whose every file belongs to it: the children of the old file
    /// and of the pure and scenario modules.
    fn directories(&self) -> [String; 3] {
        [
            format!("{}/{}/", self.service_dir, self.source),
            format!("{}/{}/", self.group_dir, self.stem),
            format!(
                "{}/{USECASES}/{}/{}/",
                self.core_dir, self.context, self.stem
            ),
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
        format!("crate::{USECASES}::{}::{}", self.context, self.stem)
    }
}

/// The directory that holds `service/`: the core crate's `src`.
pub(super) fn core_dir(map: &Map) -> String {
    Path::new(&map.service_dir)
        .parent()
        .map_or_else(String::new, |dir| dir.to_string_lossy().into_owned())
}

/// The directory of a group's modules: the `group_dirs` entry of its top segment (`sync/repair`
/// resolves under `sync`), else `<core src>/<group>`.
pub(super) fn group_dir(map: &Map, group: &str) -> String {
    let (top, rest) = group
        .split_once('/')
        .map_or((group, None), |(top, rest)| (top, Some(rest)));
    match (map.group_dirs.get(top), rest) {
        (Some(dir), Some(rest)) => format!("{dir}/{rest}"),
        (Some(dir), None) => dir.clone(),
        (None, _) => format!("{}/{group}", core_dir(map)),
    }
}

/// The facade test directory of a group: the `facade_test_dirs` entry of its top segment.
fn facade_test_dir(map: &Map, group: &str) -> Option<String> {
    let top = group.split_once('/').map_or(group, |(top, _)| top);
    map.facade_test_dirs.get(top).cloned()
}

pub fn homes(map: &Map, area: &Area) -> Vec<Home> {
    let core = core_dir(map);
    stems(area)
        .into_iter()
        .map(|stem| {
            let group = area.homes.get(&stem).unwrap_or(&area.group).clone();
            let group_dir = group_dir(map, &group);
            let facade_test_dir = facade_test_dir(map, &group);
            Home {
                source: source_of(area, &stem),
                group,
                context: area.context.clone(),
                service_dir: map.service_dir.clone(),
                core_dir: core.clone(),
                group_dir,
                facade_test_dir,
                stem,
            }
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
            |(entry, _)| entry_name(entry).to_string(),
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
    use crate::strangle::fixture::FIX_MAP;

    #[test]
    fn the_leaves_stems_resolve_to_their_target_structure_groups() {
        let map = Map::parse(include_str!("../../strangle.toml")).expect(FIX_MAP);
        let leaves = &map.areas[map.area_index("leaves").expect(FIX_MAP)];
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
            assert_eq!(groups[stem], group, "{FIX_MAP}: {stem}");
        }
    }

    #[test]
    fn a_group_dir_moves_the_group_files_but_not_the_scenarios() {
        let mut map = Map::parse(include_str!("../../strangle.toml")).expect(FIX_MAP);
        map.group_dirs
            .insert("sync".to_string(), "crates/sync/src".to_string());
        let area = &map.areas[map.area_index("sync_ids_journal").expect(FIX_MAP)];
        let home = homes(&map, area).remove(0);
        assert_eq!(
            home.pure_file(),
            format!("crates/sync/src/{}.rs", home.stem)
        );
        assert_eq!(
            home.test_files()[1],
            format!("crates/sync/src/{}_tests.rs", home.stem)
        );
        assert!(
            home.scenario_file()
                .starts_with("crates/core/src/usecases/")
        );
    }

    #[test]
    fn a_money_stem_owns_its_facade_test_file() {
        let map = Map::parse(include_str!("../../strangle.toml")).expect(FIX_MAP);
        let leaves = &map.areas[map.area_index("leaves").expect(FIX_MAP)];
        let home = homes(&map, leaves)
            .into_iter()
            .find(|home| home.stem == "ledger")
            .expect(FIX_MAP);
        let tests = home.test_files();
        assert!(tests.contains(&"crates/core/src/money_facade_tests/ledger.rs".to_string()));
        assert!(tests.contains(&"crates/core/src/money_facade_tests/ledger_tests.rs".to_string()));
    }

    #[test]
    fn a_nested_group_resolves_under_its_top_segment_dir() {
        let map = Map::parse(include_str!("../../strangle.toml")).expect(FIX_MAP);
        assert_eq!(group_dir(&map, "sync"), "crates/sync/src");
        assert_eq!(group_dir(&map, "sync/repair"), "crates/sync/src/repair");
        let area = &map.areas[map.area_index("sync_repair").expect(FIX_MAP)];
        let home = homes(&map, area).remove(0);
        assert_eq!(
            home.pure_file(),
            format!("crates/sync/src/repair/{}.rs", home.stem)
        );
    }
}
