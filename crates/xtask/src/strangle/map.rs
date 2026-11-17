//! The area map `strangle.toml`: which files and regions of `service/` belong to which area.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

/// Where the map lives, relative to the workspace root.
pub const MAP_FILE: &str = "crates/xtask/strangle.toml";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Map {
    /// The directory of `impl UnlockedVault` code, relative to the workspace root.
    pub service_dir: String,
    /// The directory holding the `#[tauri::command]` functions.
    pub command_dir: String,
    /// Every directory the scanner reads (the service and command dirs lie inside them).
    pub scan_dirs: Vec<String>,
    #[serde(rename = "area")]
    pub areas: Vec<Area>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Area {
    pub name: String,
    /// The plan task that owns the area.
    pub task: String,
    /// Target module group: `<group>/<area>.rs`.
    pub group: String,
    /// Scenario home: `usecases/<context>/<area>.rs`.
    pub context: String,
    /// Files under `service_dir`; a trailing `/` takes the whole directory.
    #[serde(default)]
    pub files: Vec<String>,
    /// Parts of a shared file, cut between two marker lines.
    #[serde(default)]
    pub regions: Vec<Region>,
    /// File stem to group, for the files whose home differs from `group`.
    #[serde(default)]
    pub homes: BTreeMap<String, String>,
    /// Whether `scaffold` makes stubs for the area (the phase-6 areas have none).
    #[serde(default = "yes")]
    pub stubs: bool,
    /// The area needs its own `ipc_contract/<area>.rs` stub for new DTOs.
    #[serde(default)]
    pub ipc_contract: bool,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Region {
    pub file: String,
    /// The region starts at the first line that begins with this text.
    pub from: String,
    /// It ends before the first line that begins with this text.
    pub to: String,
}

/// A piece of one file that an area owns; lines are 1-based and inclusive.
#[derive(Debug, Clone)]
pub struct Span {
    pub area: usize,
    pub file: String,
    pub from: usize,
    pub to: usize,
    /// The whole file (a region is a part of a shared file).
    pub whole: bool,
}

/// Every span of every area, found by file and line.
#[derive(Debug)]
pub struct Layout {
    spans: Vec<Span>,
    by_file: BTreeMap<String, Vec<usize>>,
}

impl Map {
    pub fn load(root: &Path) -> Result<Self> {
        let path = root.join(MAP_FILE);
        let text =
            std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        Self::parse(&text)
    }

    pub fn parse(text: &str) -> Result<Self> {
        let map: Self = toml::from_str(text).context("parse strangle.toml")?;
        let mut seen = std::collections::BTreeSet::new();
        for area in &map.areas {
            if !seen.insert(&area.name) {
                bail!("strangle.toml: area `{}` is listed twice", area.name);
            }
        }
        Ok(map)
    }

    pub fn area_index(&self, name: &str) -> Result<usize> {
        self.areas
            .iter()
            .position(|area| area.name == name)
            .with_context(|| format!("no area `{name}` in strangle.toml"))
    }

    /// Resolves the files and regions against the files that exist (`service_files`, relative to
    /// `service_dir`). A listed file or directory that is gone is an error: the map is stale.
    pub fn layout(&self, root: &Path, service_files: &[String]) -> Result<Layout> {
        let mut spans = Vec::new();
        for (index, area) in self.areas.iter().enumerate() {
            for entry in &area.files {
                spans.extend(file_spans(index, entry, service_files)?);
            }
            for region in &area.regions {
                spans.push(region_span(index, region, &root.join(&self.service_dir))?);
            }
        }
        Ok(Layout::new(spans))
    }
}

fn file_spans(area: usize, entry: &str, service_files: &[String]) -> Result<Vec<Span>> {
    let found: Vec<&String> = service_files
        .iter()
        .filter(|file| {
            if entry.ends_with('/') {
                file.starts_with(entry)
            } else {
                *file == entry
            }
        })
        .collect();
    if found.is_empty() {
        bail!("strangle.toml: `{entry}` matches no file under the service dir");
    }
    Ok(found
        .into_iter()
        .map(|file| Span {
            area,
            file: file.clone(),
            from: 1,
            to: usize::MAX,
            whole: true,
        })
        .collect())
}

fn region_span(area: usize, region: &Region, service_dir: &Path) -> Result<Span> {
    let path = service_dir.join(&region.file);
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let starting = |marker: &str| {
        text.lines()
            .position(|line| line.trim_start().starts_with(marker))
    };
    let from = starting(&region.from)
        .with_context(|| format!("{}: no line starts with `{}`", region.file, region.from))?;
    let to = starting(&region.to)
        .with_context(|| format!("{}: no line starts with `{}`", region.file, region.to))?;
    if to <= from {
        bail!(
            "{}: `{}` comes before `{}`",
            region.file,
            region.to,
            region.from
        );
    }
    // `position` is 0-based: the first line is `from + 1`, the line before the end marker is `to`.
    Ok(Span {
        area,
        file: region.file.clone(),
        from: from + 1,
        to,
        whole: false,
    })
}

impl Layout {
    pub fn new(spans: Vec<Span>) -> Self {
        let mut by_file: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (index, span) in spans.iter().enumerate() {
            by_file.entry(span.file.clone()).or_default().push(index);
        }
        Self { spans, by_file }
    }

    pub fn spans(&self) -> &[Span] {
        &self.spans
    }

    /// The area that owns `line` of `file`, if any.
    pub fn area_of(&self, file: &str, line: usize) -> Option<usize> {
        self.by_file
            .get(file)?
            .iter()
            .map(|&index| &self.spans[index])
            .find_map(|span| (span.from..=span.to).contains(&line).then_some(span.area))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SMALL: &str = r#"
service_dir = "svc"
command_dir = "app"
scan_dirs = ["svc", "app"]

[[area]]
name = "one"
task = "T1"
group = "g"
context = "c"
files = ["a.rs", "dir/"]
regions = [{ file = "mod.rs", from = "// -- first", to = "// -- second" }]
"#;

    fn names(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| (*item).to_string()).collect()
    }

    #[test]
    fn the_shipped_map_parses_and_names_are_unique() {
        let map = Map::parse(include_str!("../../strangle.toml")).unwrap();
        assert!(map.areas.iter().any(|area| area.name == "loyalty"));
        assert!(
            map.areas
                .iter()
                .all(|area| !area.task.is_empty() && !area.group.is_empty())
        );
    }

    #[test]
    fn a_repeated_area_name_is_refused() {
        let twice = format!(
            "{SMALL}\n[[area]]\nname = \"one\"\ntask = \"T\"\ngroup = \"g\"\ncontext = \"c\"\n"
        );
        assert!(
            Map::parse(&twice)
                .unwrap_err()
                .to_string()
                .contains("twice")
        );
    }

    #[test]
    fn files_directories_and_regions_become_spans() {
        let root = std::env::temp_dir().join(format!("xtask-strangle-map-{}", std::process::id()));
        std::fs::create_dir_all(root.join("svc")).unwrap();
        std::fs::write(
            root.join("svc/mod.rs"),
            "x\n// -- first --\nfn a() {}\n// -- second\n",
        )
        .unwrap();
        let map = Map::parse(SMALL).unwrap();
        let files = names(&["a.rs", "dir/b.rs", "dir/c.rs", "other.rs", "mod.rs"]);
        let layout = map.layout(&root, &files).unwrap();
        assert_eq!(layout.spans().len(), 4);
        assert_eq!(layout.area_of("dir/c.rs", 99), Some(0));
        assert_eq!(layout.area_of("other.rs", 1), None);
        assert_eq!(layout.area_of("mod.rs", 1), None);
        assert_eq!(layout.area_of("mod.rs", 2), Some(0));
        assert_eq!(layout.area_of("mod.rs", 3), Some(0));
        assert_eq!(layout.area_of("mod.rs", 4), None);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_listed_file_that_is_gone_is_an_error() {
        let map = Map::parse(SMALL).unwrap();
        let error = map
            .layout(Path::new("/nonexistent"), &names(&["a.rs"]))
            .unwrap_err();
        assert!(error.to_string().contains("dir/"));
    }
}
