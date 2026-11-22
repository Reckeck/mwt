//! `cargo xtask strangle`: the helper of the core-strangler phase. The mechanical part of moving an
//! area out of `impl UnlockedVault`. One `syn` scanner (`scan`), one area map (`map`,
//! `strangle.toml`), one set of numbers per subcommand. Subcommands:
//!
//! - `stats`: facts of one area or all (files, lines, methods, `persist()`, commands, calls, untested).
//! - `scaffold`: creates every module-group, area and scenario stub of the layout.
//! - `widen`: makes non-`pub` `impl UnlockedVault` methods in `service/` `pub(crate)` (methods only, never fields).
//! - `carve`: moves items named in a TOML file out of a service file into a new one.
//! - `relocate`: moves an area's files into their stubs and leaves the shim with `pub use`.
//! - `forward`: regenerates the `forward!` block of an area's shims.
//! - `check`: holds an area against `baseline.json` (parity, logic, `persist()`, tests, files).
//! - `callers`: lists the remaining calls of an area's delegates and of `Books::vault()`.
//! - `rewire`: best effort replacement of delegate calls by the new function.
//! - `remaining`: the per-file counts of `UnlockedVault`, `forward!` entries and `service::` paths, held by `check --all`.
//! - `retire`: marks an area whose delegates are deleted.

mod baseline;
mod callers;
mod carve;
mod check;
mod fields;
mod fileset;
mod forward;
mod home;
mod map;
mod relocate;
mod remaining;
mod retire;
mod rewire;
mod scaffold;
mod scan;
mod scope;
mod signature;
mod source;
mod stats;
mod widen;

use std::path::Path;

use anyhow::Result;
use clap::{Args, Subcommand};

use map::{Layout, Map};
use scan::{Corpus, Scanned};

#[derive(Args)]
pub struct StrangleArgs {
    #[command(subcommand)]
    pub command: StrangleCommand,
}

#[derive(Subcommand)]
pub enum StrangleCommand {
    /// Facts of one area or all (files, lines, methods, persist(), commands, calls, untested).
    Stats(stats::StatsArgs),
    /// Creates every module-group, area and scenario stub of the layout and declares it once.
    Scaffold(scaffold::ScaffoldArgs),
    /// Makes every non-`pub` method of `impl UnlockedVault` in `service/` `pub(crate)`.
    Widen,
    /// Moves the items named in a TOML file out of a service file into a new one.
    Carve(carve::CarveArgs),
    /// Moves an area's files into their stubs and leaves the shim with `pub use`.
    Relocate(relocate::RelocateArgs),
    /// Regenerates the `forward!` block of an area's shims from its new functions.
    Forward(forward::ForwardArgs),
    /// Holds an area against `baseline.json`: parity, logic, `persist()`, tests, allowed files.
    Check(check::CheckArgs),
    /// Lists every remaining call of an area's delegates and of `Books::vault()`.
    Callers(callers::CallersArgs),
    /// Best effort: replaces delegate calls on a proven `UnlockedVault` receiver by the new function.
    Rewire(rewire::RewireArgs),
    /// The per-file counts of `UnlockedVault`, `forward!` entries and `service::` paths.
    Remaining(remaining::RemainingArgs),
    /// Marks an area `retired = true`: its delegates are deleted, rule 1 wants none.
    Retire(retire::RetireArgs),
}

pub fn run(root: &Path, args: &StrangleArgs) -> Result<()> {
    match &args.command {
        StrangleCommand::Stats(stats_args) => stats::run(&Workspace::load(root)?, stats_args),
        StrangleCommand::Scaffold(scaffold_args) => scaffold::run(root, scaffold_args),
        StrangleCommand::Widen => widen::run(root, &Workspace::load(root)?),
        StrangleCommand::Carve(carve_args) => carve::run(root, carve_args),
        StrangleCommand::Relocate(relocate_args) => relocate::run(root, relocate_args),
        StrangleCommand::Forward(forward_args) => forward::run(root, forward_args),
        StrangleCommand::Check(check_args) => check::run(root, check_args),
        StrangleCommand::Callers(callers_args) => callers::run(root, callers_args),
        StrangleCommand::Rewire(rewire_args) => rewire::run(root, rewire_args),
        StrangleCommand::Remaining(remaining_args) => remaining::run(root, remaining_args),
        StrangleCommand::Retire(retire_args) => retire::run(root, retire_args),
    }
}

/// Everything a subcommand reads: the area map, its resolution, and the scanned sources.
pub struct Workspace {
    pub map: Map,
    pub layout: Layout,
    pub corpus: Corpus,
}

impl Workspace {
    pub fn load(root: &Path) -> Result<Self> {
        Self::build(root, Map::load(root)?)
    }

    pub fn build(root: &Path, map: Map) -> Result<Self> {
        let corpus = Corpus::scan(root, &map.scan_dirs)?;
        let prefix = service_prefix(&map);
        let service_files: Vec<String> = corpus
            .under(&prefix)
            .map(|(path, _)| path[prefix.len()..].to_string())
            .collect();
        let layout = map.layout(root, &service_files)?;
        Ok(Self {
            map,
            layout,
            corpus,
        })
    }

    /// A file of the service dir by its path inside it.
    pub fn service_file(&self, relative: &str) -> Option<&Scanned> {
        self.corpus
            .get(&format!("{}{relative}", service_prefix(&self.map)))
    }

    /// Every file of the service dir with its path inside it.
    pub fn service_files(&self) -> impl Iterator<Item = (&str, &Scanned)> {
        let skip = service_prefix(&self.map).len();
        self.corpus
            .under(&service_prefix(&self.map))
            .map(move |(path, scanned)| (&path[skip..], scanned))
    }
}

fn service_prefix(map: &Map) -> String {
    format!("{}/", map.service_dir)
}

#[cfg(test)]
pub mod fixture {
    //! A small service tree, written to a temp dir, scanned, and removed again.

    use super::*;

    /// Prefix of every live-tree assertion message: the map is the file to fix.
    pub const FIX_MAP: &str =
        "the live tree and crates/xtask/strangle.toml disagree; fix strangle.toml";

    const MAP: &str = r#"
service_dir = "crates/svc"
command_dir = "crates/app"
scan_dirs = ["crates/svc", "crates/app"]

[[area]]
name = "alpha"
task = "T1"
group = "g"
context = "c"
files = ["alpha.rs", "alpha_tests.rs"]

[[area]]
name = "beta"
task = "T2"
group = "g"
context = "c"
regions = [{ file = "mod.rs", from = "// -- beta", to = "// -- end" }]
"#;

    const ALPHA: &str = "impl UnlockedVault {
    pub fn first(&mut self) {
        self.persist();
        self.beta_pub();
    }
    pub fn quiet(&self) {}
    fn hid(&self) {}
    pub(crate) fn hid2(&self) {}
}
#[cfg(test)]
mod tests {
    #[test]
    fn pins() { v.first(); }
}
";

    const SHARED: &str = "// header
impl UnlockedVault {
    // -- beta
    pub fn beta_pub(&self) {}
    // -- end
    pub fn unowned(&self) {}
}
";

    const COMMANDS: &str = "#[tauri::command]
fn one(vault: V) { vault.first(); vault.beta_pub(); }
#[tauri::command]
fn two(vault: V) { vault.beta_pub(); }
";

    /// Files written under a temp dir, removed again when dropped.
    pub struct TempTree {
        root: std::path::PathBuf,
    }

    impl TempTree {
        pub fn new(files: &[(&str, &str)]) -> Self {
            let root = unique_dir();
            for (path, text) in files {
                let file = root.join(path);
                std::fs::create_dir_all(file.parent().unwrap()).unwrap();
                std::fs::write(file, text).unwrap();
            }
            Self { root }
        }

        pub fn root(&self) -> &std::path::Path {
            &self.root
        }
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.root).ok();
        }
    }

    fn unique_dir() -> std::path::PathBuf {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        std::env::temp_dir().join(format!("xtask-strangle-{}-{id}", std::process::id()))
    }

    pub fn workspace() -> Workspace {
        let root = unique_dir();
        for (path, text) in [
            ("crates/svc/alpha.rs", ALPHA),
            (
                "crates/svc/alpha_tests.rs",
                "// tests\nfn extra() {}\n// end\n",
            ),
            ("crates/svc/mod.rs", SHARED),
            ("crates/app/commands.rs", COMMANDS),
        ] {
            let file = root.join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, text).unwrap();
        }
        let workspace = Workspace::build(&root, Map::parse(MAP).unwrap()).unwrap();
        std::fs::remove_dir_all(&root).unwrap();
        workspace
    }
}

#[cfg(test)]
pub mod area_fixture {
    //! One area `loyalty` before the move (`ORIGINAL`) and after it (`moved_files`), written to a
    //! temp dir, for `relocate`, `forward`, `baseline` and `check`.

    use super::fixture::TempTree;
    use super::map::MAP_FILE;

    const MAP: &str = r#"
service_dir = "crates/core/src/service"
command_dir = "crates/app/src"
scan_dirs = ["crates/core/src"]

[[area]]
name = "loyalty"
task = "T26"
group = "household"
context = "household"
files = ["loyalty.rs"]
"#;

    pub const SERVICE: &str = "crates/core/src/service/loyalty.rs";
    pub const PURE: &str = "crates/core/src/household/loyalty.rs";
    pub const SCENARIO: &str = "crates/core/src/usecases/household/loyalty.rs";
    pub const TESTS: &str = "crates/core/src/household/loyalty_tests.rs";

    pub const ORIGINAL: &str = "//! Loyalty.
use super::*;

/// A loyalty card.
pub struct Card {
    pub id: String,
}

pub(crate) struct Hidden;

impl UnlockedVault {
    /// Sets the info.
    pub fn set_info(&mut self, id: &str, info: Info) -> Result<Store> {
        self.persist();
        Ok(Store)
    }

    pub fn clash(&self, id: &str) -> bool {
        let rates = self.rate_book();
        rates.contains(id)
    }

    pub(crate) fn helper(&self) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::golden;

    #[test]
    fn sets_info() {
        let card = crate::service::loyalty::Card { id: String::new() };
    }

    #[test]
    fn clash_works() {}
}
";

    /// The tree before `relocate`: the old file and the empty stubs.
    pub fn original_tree() -> TempTree {
        TempTree::new(&[
            (MAP_FILE, MAP),
            (SERVICE, ORIGINAL),
            (PURE, "//! placeholder, filled by T26\n"),
            (TESTS, "//! placeholder, filled by T26\n"),
            (SCENARIO, "//! placeholder, filled by T26\n"),
        ])
    }

    /// The area after the recipe: a shim with its forward block, a query, a scenario, the tests.
    pub fn moved_files() -> Vec<(&'static str, String)> {
        vec![
            (MAP_FILE, MAP.to_string()),
            (
                SERVICE,
                "//! Shim.

pub use crate::household::loyalty::Card;

// forward:begin
forward! {
    /// Sets the info.
    pub fn set_info(&mut self, id: &str, info: Info) -> Result<Store>
        => crate::usecases::household::loyalty::set_info;
    pub fn clash(&self, id: &str) -> bool
        => crate::household::loyalty::clash @books;
}
// forward:end
"
                .to_string(),
            ),
            (
                PURE,
                "//! Pure part.
pub struct Card {
    pub id: String,
}

/// Is there a clash.
pub fn clash(books: &Books<'_>, id: &str) -> bool {
    books.rate_book().contains(id)
}
"
                .to_string(),
            ),
            (
                SCENARIO,
                "//! Scenarios.
/// Sets the info.
pub fn set_info(vault: &mut UnlockedVault, id: &str, info: Info) -> Result<Store> {
    vault.transact(|tx| apply(tx, id, info))
}
"
                .to_string(),
            ),
            (
                TESTS,
                "#[test]
fn sets_info() {}

#[test]
fn clash_works() {}
"
                .to_string(),
            ),
        ]
    }

    /// The moved area on disk, with `edit` applied to the text of each file by path.
    pub fn moved_tree(edit: impl Fn(&str, String) -> String) -> TempTree {
        let files = moved_files();
        let edited: Vec<(&str, String)> = files
            .into_iter()
            .map(|(path, text)| (path, edit(path, text)))
            .collect();
        let borrowed: Vec<(&str, &str)> = edited
            .iter()
            .map(|(path, text)| (*path, text.as_str()))
            .collect();
        TempTree::new(&borrowed)
    }
}
