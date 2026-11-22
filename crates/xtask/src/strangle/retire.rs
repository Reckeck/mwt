//! `strangle retire <area>`: marks an area `retired = true` in `strangle.toml`. Its delegates are
//! deleted, so rule 1 of `check` stops wanting the baseline methods and fails if a `forward!`
//! entry of the area reappears. The edit is textual, so the comments of the map stay.

use std::path::Path;

use anyhow::{Context, Result, bail};
use clap::Args;

use super::map::{MAP_FILE, Map};

const RETIRED_LINE: &str = "retired = true";

#[derive(Args)]
pub struct RetireArgs {
    /// One area of `strangle.toml`.
    pub area: String,
}

pub fn run(root: &Path, args: &RetireArgs) -> Result<()> {
    let path = root.join(MAP_FILE);
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    match mark_retired(&text, &args.area)? {
        Some(edited) => {
            std::fs::write(&path, edited).with_context(|| format!("write {}", path.display()))?;
            println!("retire: {} is retired", args.area);
        }
        None => println!("retire: {} was already retired", args.area),
    }
    Ok(())
}

/// The map text with `retired = true` under the area's `name` line; `None` when it is already so.
fn mark_retired(text: &str, area: &str) -> Result<Option<String>> {
    let map = Map::parse(text)?;
    if map.areas[map.area_index(area)?].retired {
        return Ok(None);
    }
    let name_line = format!("name = \"{area}\"");
    let mut edited = String::with_capacity(text.len() + RETIRED_LINE.len() + 1);
    let mut done = false;
    for line in text.split_inclusive('\n') {
        edited.push_str(line);
        if !done && line.trim_end() == name_line {
            if !line.ends_with('\n') {
                edited.push('\n');
            }
            edited.push_str(RETIRED_LINE);
            edited.push('\n');
            done = true;
        }
    }
    if !done {
        bail!("strangle.toml: no line `{name_line}` to mark (is the area written another way?)");
    }
    let check = Map::parse(&edited)?;
    if !check.areas[check.area_index(area)?].retired {
        bail!("strangle.toml: `{area}` is not retired after the edit");
    }
    Ok(Some(edited))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strangle::Workspace;
    use crate::strangle::area_fixture::{SERVICE, moved_tree, original_tree};
    use crate::strangle::baseline;
    use crate::strangle::check::findings;
    use crate::strangle::fixture::TempTree;

    const SHIM_WITHOUT_ENTRIES: &str = "//! Shim.\n\npub use crate::household::loyalty::Card;\n\n// forward:begin\n// forward:end\n";

    /// The moved fixture area, retired, with the shim replaced by `shim`.
    fn retired_tree(shim: Option<&str>) -> TempTree {
        moved_tree(|path, text| {
            if path == crate::strangle::map::MAP_FILE {
                mark_retired(&text, "loyalty").unwrap().unwrap()
            } else if path == SERVICE {
                shim.map_or(text, str::to_string)
            } else {
                text
            }
        })
    }

    fn parity_findings(after: &TempTree) -> Vec<String> {
        let before = original_tree();
        let baseline =
            baseline::capture(before.root(), &Workspace::load(before.root()).unwrap()).unwrap();
        let workspace = Workspace::load(after.root()).unwrap();
        findings(
            after.root(),
            &workspace,
            &baseline,
            Some("loyalty"),
            None,
            None,
        )
        .unwrap()
        .into_iter()
        .filter(|finding| finding.contains(": parity: "))
        .collect()
    }

    #[test]
    fn retire_sets_the_flag_once_and_keeps_the_rest() {
        let text = "# note\nservice_dir = \"s\"\ncommand_dir = \"c\"\nscan_dirs = []\n\n[[area]]\nname = \"alpha\"\ntask = \"T1\"\ngroup = \"g\"\ncontext = \"c\"\n";
        let edited = mark_retired(text, "alpha").unwrap().unwrap();
        assert!(edited.contains("name = \"alpha\"\nretired = true\ntask"));
        assert!(edited.starts_with("# note\n"));
        assert!(Map::parse(&edited).unwrap().areas[0].retired);
        assert_eq!(mark_retired(&edited, "alpha").unwrap(), None);
        assert!(mark_retired(text, "missing").is_err());
    }

    #[test]
    fn a_retired_area_passes_rule_1_without_forward_entries() {
        let tree = retired_tree(Some(SHIM_WITHOUT_ENTRIES));
        assert_eq!(parity_findings(&tree), Vec::<String>::new());
    }

    #[test]
    fn a_retired_area_fails_rule_1_when_a_forward_entry_reappears() {
        let tree = retired_tree(None);
        assert_eq!(
            parity_findings(&tree),
            [
                "loyalty: parity: forwarded but not in the baseline: clash",
                "loyalty: parity: forwarded but not in the baseline: set_info",
            ]
        );
    }
}
