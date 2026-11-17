//! Where one area file lives before and after the move: the shim in `service/`, the pure module
//! `<group>/<stem>.rs`, and the scenario module `usecases/<context>/<stem>.rs` (spec D1). One home
//! for those paths, shared by `relocate`, `forward`, `check` and the baseline.

use std::collections::BTreeSet;
use std::path::Path;

use super::map::{Area, Map};
use super::scaffold::stems;

/// One file stem of an area, with every path it owns.
pub struct Home {
    pub stem: String,
    pub group: String,
    pub context: String,
    service_dir: String,
    core_dir: String,
}

impl Home {
    /// `service/<stem>.rs`: the old file, then the shim.
    pub fn shim(&self) -> String {
        format!("{}/{}.rs", self.service_dir, self.stem)
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
            format!("{}/{}_tests.rs", self.service_dir, self.stem),
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

    /// `crate::<group>::<stem>`.
    pub fn pure_path(&self) -> String {
        format!("crate::{}::{}", self.group, self.stem)
    }

    /// `crate::usecases::<context>::<stem>`.
    pub fn scenario_path(&self) -> String {
        format!("crate::usecases::{}::{}", self.context, self.stem)
    }
}

/// The directory that holds `service/`: the core crate's `src`.
fn core_dir(map: &Map) -> String {
    Path::new(&map.service_dir)
        .parent()
        .map_or_else(String::new, |dir| dir.to_string_lossy().into_owned())
}

pub fn homes(map: &Map, area: &Area) -> Vec<Home> {
    let core = core_dir(map);
    stems(area)
        .into_iter()
        .map(|stem| Home {
            group: area.homes.get(&stem).unwrap_or(&area.group).clone(),
            context: area.context.clone(),
            service_dir: map.service_dir.clone(),
            core_dir: core.clone(),
            stem,
        })
        .collect()
}

/// Every file of every stem of an area, sorted.
pub fn area_files(map: &Map, area: &Area) -> Vec<String> {
    let mut files: Vec<String> = homes(map, area).iter().flat_map(Home::files).collect();
    files.sort();
    files.dedup();
    files
}

/// The files a task of the area may change (spec D5): the area files, its `ipc_contract` stub, and
/// the shared files its regions cut from.
pub fn allowed_files(map: &Map, area: &Area) -> BTreeSet<String> {
    let mut allowed: BTreeSet<String> = area_files(map, area).into_iter().collect();
    if area.ipc_contract {
        allowed.insert(format!("{}/ipc_contract/{}.rs", core_dir(map), area.name));
    }
    allowed.extend(
        area.regions
            .iter()
            .map(|region| format!("{}/{}", map.service_dir, region.file)),
    );
    allowed
}
