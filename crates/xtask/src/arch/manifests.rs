//! The dev-only rule: no crate lists `mwt-testkit` under `[dependencies]`. It belongs in
//! `[dev-dependencies]`; the one exception is the use-case crate, which reaches it through its `testkit`
//! feature as an `optional = true` entry.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

const DEV_ONLY: &str = "mwt-testkit";
/// The crate whose `testkit` feature may enable the dev-only crate.
const USECASES: &str = "mwt-usecases";

/// The message for each crate under `crates_dir` that lists the dev-only crate as a normal dependency.
pub fn check(crates_dir: &Path) -> Result<Vec<String>> {
    let mut found = Vec::new();
    let mut entries = fs::read_dir(crates_dir)
        .with_context(|| format!("read {}", crates_dir.display()))?
        .collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let manifest = entry.path().join("Cargo.toml");
        let Ok(text) = fs::read_to_string(&manifest) else {
            continue;
        };
        let label = format!("crates/{}/Cargo.toml", entry.file_name().to_string_lossy());
        found.extend(violations(&label, &text)?);
    }
    Ok(found)
}

fn violations(label: &str, text: &str) -> Result<Option<String>> {
    let manifest: toml::Table = toml::from_str(text).with_context(|| format!("parse {label}"))?;
    let Some(dependency) = manifest
        .get("dependencies")
        .and_then(toml::Value::as_table)
        .and_then(|table| table.get(DEV_ONLY))
    else {
        return Ok(None);
    };
    let name = manifest
        .get("package")
        .and_then(|package| package.get("name"))
        .and_then(toml::Value::as_str);
    let optional = dependency
        .get("optional")
        .and_then(toml::Value::as_bool)
        .unwrap_or(false);
    if name == Some(USECASES) && optional {
        return Ok(None);
    }
    Ok(Some(format!(
        "error: {label}: `{DEV_ONLY}` is dev-only: list it under [dev-dependencies] (only `{USECASES}` may list it as an optional dependency of its `testkit` feature)"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(name: &str, dependencies: &str) -> String {
        format!("[package]\nname = \"{name}\"\n\n{dependencies}")
    }

    #[test]
    fn a_dev_dependency_is_fine() {
        let text = manifest(
            "mwt-vault",
            "[dev-dependencies]\nmwt-testkit = { path = \"../testkit\" }\n",
        );
        assert_eq!(violations("v", &text).unwrap(), None);
    }

    #[test]
    fn a_normal_dependency_fails() {
        let text = manifest(
            "mwt-money",
            "[dependencies]\nmwt-testkit = { path = \"../testkit\" }\n",
        );
        let message = violations("crates/money/Cargo.toml", &text)
            .unwrap()
            .unwrap();
        assert!(message.contains("crates/money/Cargo.toml"), "{message}");
    }

    #[test]
    fn only_the_facade_may_list_it_optional() {
        let optional = "[dependencies]\nmwt-testkit = { path = \"../testkit\", optional = true }\n";
        assert_eq!(
            violations("f", &manifest(USECASES, optional)).unwrap(),
            None
        );
        assert!(
            violations("o", &manifest("mwt-money", optional))
                .unwrap()
                .is_some()
        );
        let plain = "[dependencies]\nmwt-testkit = { path = \"../testkit\" }\n";
        assert!(
            violations("f", &manifest(USECASES, plain))
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn the_workspace_keeps_the_rule() {
        let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        assert_eq!(check(&crates).unwrap(), Vec::<String>::new());
    }
}
