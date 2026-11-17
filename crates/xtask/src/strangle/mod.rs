//! `cargo xtask strangle`: the helper of the core-strangler phase. The mechanical part of moving an
//! area out of `impl UnlockedVault` (facts today; more subcommands later). One `syn` scanner
//! (`scan`), one area map (`map`, `strangle.toml`), one set of numbers per subcommand.

mod map;
mod scan;
mod stats;

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
}

pub fn run(root: &Path, args: &StrangleArgs) -> Result<()> {
    let workspace = Workspace::load(root)?;
    match &args.command {
        StrangleCommand::Stats(stats_args) => stats::run(&workspace, stats_args),
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

    pub fn workspace() -> Workspace {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!("xtask-strangle-{}-{id}", std::process::id()));
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
