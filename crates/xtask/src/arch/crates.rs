//! The crate level of `xtask arch`: the workspace crate edges of `cargo metadata` against the crate
//! table of the crate split (spec crate-split, D8 and "Interfaces", amended 2026-11-23), plus the rules
//! about where external crates may be used. Pure functions over JSON and a table; the one process
//! (`cargo metadata`) runs in `metadata`.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde_json::Value;

/// One row of the crate table: a crate and the workspace crates it may depend on.
pub struct Row {
    pub name: &'static str,
    pub uses: &'static [&'static str],
}

/// An exception of the external-crate rule: `package` may depend on `dependency` until `until`.
pub struct ExternalAllow {
    pub package: &'static str,
    pub dependency: &'static str,
    pub until: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Normal,
    Dev,
    Build,
}

impl Kind {
    fn label(self) -> &'static str {
        match self {
            Self::Normal => "dependency",
            Self::Dev => "dev-dependency",
            Self::Build => "build dependency",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependency {
    pub name: String,
    pub kind: Kind,
    pub optional: bool,
}

/// A workspace crate and what it lists under `[dependencies]`, `[dev-dependencies]`, `[build-dependencies]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    pub name: String,
    pub dependencies: Vec<Dependency>,
}

const BASE: &str = "mwt-base";
const MACROS: &str = "mwt-macros";
const MODEL: &str = "mwt-model";
const CRYPTO: &str = "mwt-crypto";
const NET: &str = "mwt-net";
const VAULT: &str = "mwt-vault";
const SYNC: &str = "mwt-sync";
const MONEY: &str = "mwt-money";
const HOUSEHOLD: &str = "mwt-household";
const WORKFLOW: &str = "mwt-workflow";
const PLANNING: &str = "mwt-planning";
const REPORTS: &str = "mwt-reports";
const CRYPTOTRACK: &str = "mwt-cryptotrack";
const CONTRACT: &str = "mwt-contract";
const TESTKIT: &str = "mwt-testkit";
const FACADE: &str = "trade-core";
const USECASES: &str = "mwt-usecases";
const APP: &str = "trade-app";
const PLUGIN_PREFIX: &str = "tauri-plugin-";
/// Dev-dependencies of every crate may name this crate besides its own row.
const DEV_ALLOWED: &str = TESTKIT;

/// The crate table: the spec's "Interfaces" table as data.
pub const TABLE: &[Row] = &[
    Row {
        name: BASE,
        uses: &[MACROS],
    },
    Row {
        name: MACROS,
        uses: &[],
    },
    Row {
        name: CRYPTO,
        uses: &[BASE],
    },
    Row {
        name: MODEL,
        uses: &[BASE, MACROS, CRYPTO],
    },
    Row {
        name: NET,
        uses: &[BASE, MODEL],
    },
    Row {
        name: VAULT,
        uses: &[MODEL, CRYPTO, BASE],
    },
    Row {
        name: SYNC,
        uses: &[MODEL, CRYPTO, BASE],
    },
    Row {
        name: TESTKIT,
        uses: &[MODEL, BASE, VAULT],
    },
    Row {
        name: MONEY,
        uses: &[MODEL, BASE],
    },
    Row {
        name: HOUSEHOLD,
        uses: &[MONEY, MODEL, BASE],
    },
    Row {
        name: WORKFLOW,
        uses: &[MODEL, BASE],
    },
    Row {
        name: PLANNING,
        uses: &[HOUSEHOLD, MONEY, MODEL, BASE],
    },
    Row {
        name: REPORTS,
        uses: &[PLANNING, HOUSEHOLD, MONEY, MODEL, BASE],
    },
    Row {
        name: CRYPTOTRACK,
        uses: &[MONEY, MODEL, BASE],
    },
    Row {
        name: CONTRACT,
        uses: &[
            MONEY,
            HOUSEHOLD,
            WORKFLOW,
            PLANNING,
            REPORTS,
            CRYPTOTRACK,
            MODEL,
            BASE,
            SYNC,
            CRYPTO,
            VAULT,
            NET,
            MACROS,
        ],
    },
    Row {
        name: USECASES,
        uses: &[
            BASE,
            CRYPTO,
            MODEL,
            NET,
            VAULT,
            SYNC,
            TESTKIT,
            MONEY,
            HOUSEHOLD,
            WORKFLOW,
            PLANNING,
            REPORTS,
            CRYPTOTRACK,
            CONTRACT,
            MACROS,
        ],
    },
    // The facade: re-exports `mwt-usecases`; the type generator names the view types of three crates.
    Row {
        name: FACADE,
        uses: &[USECASES, MONEY, REPORTS, CRYPTOTRACK],
    },
    Row {
        name: APP,
        uses: &[
            FACADE,
            "mwt-app-macros",
            "tauri-plugin-biometric",
            "tauri-plugin-insets",
            "tauri-plugin-net",
            "tauri-plugin-saf",
        ],
    },
    Row {
        name: "mwt-app-macros",
        uses: &[],
    },
    Row {
        name: "xtask",
        uses: &[],
    },
    Row {
        name: "tauri-plugin-biometric",
        uses: &[],
    },
    Row {
        name: "tauri-plugin-insets",
        uses: &[],
    },
    Row {
        name: "tauri-plugin-net",
        uses: &[],
    },
    Row {
        name: "tauri-plugin-saf",
        uses: &[],
    },
];

/// `reqwest` and the parsers live in `mwt-net` only, except these entries; a stale entry fails.
pub const EXTERNAL_ALLOW: &[ExternalAllow] = &[
    // The shell's updater and pricing adapter.
    ExternalAllow {
        package: APP,
        dependency: "reqwest",
        until: "permanent",
    },
];

const NET_ONLY: &[&str] = &["reqwest", "scraper", "calamine", "csv"];
const OPTIONAL_ONLY: &[&str] = &["ts-rs", "schemars"];
const SYNC_ONLY: &[&str] = &["snow", "spake2"];

/// The workspace crates of a `cargo metadata --no-deps` document with their dependency lists.
pub fn parse(json: &Value) -> Vec<Package> {
    let members: Option<BTreeSet<&str>> = json
        .get("workspace_members")
        .and_then(Value::as_array)
        .map(|ids| ids.iter().filter_map(Value::as_str).collect());
    let packages = json.get("packages").and_then(Value::as_array);
    packages
        .into_iter()
        .flatten()
        .filter(|package| {
            let id = package.get("id").and_then(Value::as_str).unwrap_or("");
            members.as_ref().is_none_or(|set| set.contains(id))
        })
        .filter_map(|package| {
            let name = package.get("name")?.as_str()?.to_string();
            let dependencies = package
                .get("dependencies")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(parse_dependency)
                .collect();
            Some(Package { name, dependencies })
        })
        .collect()
}

fn parse_dependency(dependency: &Value) -> Option<Dependency> {
    let kind = match dependency.get("kind").and_then(Value::as_str) {
        None => Kind::Normal,
        Some("dev") => Kind::Dev,
        Some("build") => Kind::Build,
        Some(_) => return None,
    };
    Some(Dependency {
        name: dependency.get("name")?.as_str()?.to_string(),
        kind,
        optional: dependency
            .get("optional")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

/// The row a crate may use, as an edge test for the dependency kind.
fn edge_allowed(row: &Row, dependency: &Dependency) -> bool {
    row.uses.contains(&dependency.name.as_str())
        || (dependency.kind == Kind::Dev && dependency.name == DEV_ALLOWED)
}

/// One message per rule the packages break: edges against `table`, external crates, stale `external_allow`.
pub fn violations(
    packages: &[Package],
    table: &[Row],
    external_allow: &[ExternalAllow],
) -> Vec<String> {
    let members: BTreeSet<&str> = packages
        .iter()
        .map(|package| package.name.as_str())
        .collect();
    let mut found = Vec::new();
    for package in packages {
        let row = table.iter().find(|row| row.name == package.name);
        if row.is_none() {
            found.push(format!(
                "error: crate `{}` is not in the crate table (crates.rs, spec \"Interfaces\")",
                package.name
            ));
        }
        for dependency in &package.dependencies {
            if dependency.name == package.name {
                continue;
            }
            found.extend(dependency_problem(
                package,
                dependency,
                row,
                &members,
                external_allow,
            ));
        }
    }
    found.extend(stale(packages, external_allow));
    found
}

fn dependency_problem(
    package: &Package,
    dependency: &Dependency,
    row: Option<&Row>,
    members: &BTreeSet<&str>,
    external_allow: &[ExternalAllow],
) -> Option<String> {
    let name = dependency.name.as_str();
    let edge = format!("`{}` -> `{name}`", package.name);
    let kind = dependency.kind.label();
    if package.name != APP && (name == APP || name.starts_with(PLUGIN_PREFIX)) {
        return Some(format!(
            "error: {edge} ({kind}): nothing but `{APP}` depends on `{APP}` or a `{PLUGIN_PREFIX}*` crate"
        ));
    }
    if OPTIONAL_ONLY.contains(&name) && !dependency.optional {
        return Some(format!(
            "error: {edge} ({kind}): `{name}` must be an optional dependency (feature `ts`/`schema`)"
        ));
    }
    if NET_ONLY.contains(&name)
        && package.name != NET
        && !is_external_allowed(package, name, external_allow)
    {
        return Some(format!(
            "error: {edge} ({kind}): `{name}` belongs to `{NET}` only"
        ));
    }
    if SYNC_ONLY.contains(&name) && package.name != SYNC {
        return Some(format!(
            "error: {edge} ({kind}): `{name}` belongs to `{SYNC}` only"
        ));
    }
    let row = row?;
    if members.contains(name) && !edge_allowed(row, dependency) {
        return Some(format!(
            "error: {edge} ({kind}): edge is not in the crate table row of `{}`",
            package.name
        ));
    }
    None
}

fn is_external_allowed(package: &Package, dependency: &str, allow: &[ExternalAllow]) -> bool {
    allow
        .iter()
        .any(|entry| entry.package == package.name && entry.dependency == dependency)
}

fn stale(packages: &[Package], external_allow: &[ExternalAllow]) -> Vec<String> {
    external_allow
        .iter()
        .filter(|entry| {
            !packages.iter().any(|package| {
                package.name == entry.package
                    && package
                        .dependencies
                        .iter()
                        .any(|dependency| dependency.name == entry.dependency)
            })
        })
        .map(|entry| {
            format!(
                "error: EXTERNAL_ALLOW (crates.rs): stale entry `{}` -> `{}` (until {}): no such dependency any more, delete it",
                entry.package, entry.dependency, entry.until
            )
        })
        .collect()
}

/// The crate-table findings for the repository at `root`, from `cargo metadata --format-version 1 --no-deps`.
pub fn check(root: &Path) -> Result<Vec<String>> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(root)
        .output()
        .context("run cargo metadata")?;
    if !output.status.success() {
        bail!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let json: Value =
        serde_json::from_slice(&output.stdout).context("parse the output of cargo metadata")?;
    Ok(violations(&parse(&json), TABLE, EXTERNAL_ALLOW))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dependency(name: &str, kind: Kind, optional: bool) -> Dependency {
        Dependency {
            name: name.to_string(),
            kind,
            optional,
        }
    }

    fn normal(name: &str) -> Dependency {
        dependency(name, Kind::Normal, false)
    }

    fn package(name: &str, dependencies: Vec<Dependency>) -> Package {
        Package {
            name: name.to_string(),
            dependencies,
        }
    }

    fn run(packages: &[Package]) -> Vec<String> {
        violations(packages, TABLE, &[])
    }

    fn only(messages: &[String], parts: &[&str]) {
        assert_eq!(messages.len(), 1, "{messages:?}");
        for part in parts {
            assert!(messages[0].contains(part), "{part} not in {messages:?}");
        }
    }

    #[test]
    fn an_edge_not_in_the_table_fails() {
        let found = run(&[
            package(MONEY, vec![normal(WORKFLOW)]),
            package(WORKFLOW, vec![]),
        ]);
        only(&found, &["`mwt-money` -> `mwt-workflow`"]);
    }

    #[test]
    fn a_crate_not_in_the_table_fails() {
        let found = run(&[package("mwt-new", vec![])]);
        only(&found, &["`mwt-new`", "not in the crate table"]);
    }

    #[test]
    fn a_dev_dependency_outside_the_row_fails() {
        let found = run(&[
            package(MONEY, vec![dependency(VAULT, Kind::Dev, false)]),
            package(VAULT, vec![]),
        ]);
        only(&found, &["`mwt-money` -> `mwt-vault`", "dev-dependency"]);
    }

    #[test]
    fn reqwest_and_the_parsers_outside_net_fail() {
        let found = run(&[package(
            MONEY,
            vec![
                normal("reqwest"),
                normal("scraper"),
                normal("calamine"),
                normal("csv"),
            ],
        )]);
        assert_eq!(found.len(), 4, "{found:?}");
        assert!(found[0].contains("`mwt-money` -> `reqwest`"));
        assert!(found[1].contains("`scraper`"));
    }

    #[test]
    fn ts_rs_and_schemars_must_be_optional_in_every_kind() {
        let found = run(&[package(
            MONEY,
            vec![
                normal("ts-rs"),
                dependency("schemars", Kind::Dev, false),
                dependency("ts-rs", Kind::Build, false),
            ],
        )]);
        assert_eq!(found.len(), 3, "{found:?}");
        assert!(found[1].contains("`schemars`") && found[1].contains("dev-dependency"));
    }

    #[test]
    fn snow_and_spake2_outside_sync_fail() {
        let found = run(&[package(CRYPTO, vec![normal("snow"), normal("spake2")])]);
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(found[0].contains("`mwt-crypto` -> `snow`"));
    }

    #[test]
    fn a_build_dependency_outside_the_row_fails() {
        let found = run(&[
            package(MONEY, vec![dependency(CRYPTO, Kind::Build, false)]),
            package(CRYPTO, vec![]),
        ]);
        only(&found, &["`mwt-money` -> `mwt-crypto`", "build dependency"]);
    }

    #[test]
    fn a_stale_external_allow_entry_fails() {
        let allow = [ExternalAllow {
            package: MONEY,
            dependency: "reqwest",
            until: "F7",
        }];
        let found = violations(&[package(MONEY, vec![])], TABLE, &allow);
        only(&found, &["stale entry", "`mwt-money` -> `reqwest`"]);
    }

    #[test]
    fn trade_app_and_plugins_are_named_by_trade_app_only() {
        let found = run(&[
            package(FACADE, vec![normal(APP), normal("tauri-plugin-saf")]),
            package(APP, vec![normal(FACADE), normal("tauri-plugin-saf")]),
            package("tauri-plugin-saf", vec![]),
        ]);
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(found[0].contains("`trade-core` -> `trade-app`"));
        assert!(found[1].contains("`trade-core` -> `tauri-plugin-saf`"));
        let found = run(&[package(MONEY, vec![normal("tauri-plugin-fs")])]);
        only(&found, &["`mwt-money` -> `tauri-plugin-fs`"]);
    }

    #[test]
    fn a_tree_that_obeys_every_rule_is_green() {
        let allow = [ExternalAllow {
            package: APP,
            dependency: "reqwest",
            until: "permanent",
        }];
        let packages = [
            package(
                MONEY,
                vec![
                    normal(MODEL),
                    dependency(MONEY, Kind::Dev, false),
                    dependency(TESTKIT, Kind::Dev, false),
                    dependency(BASE, Kind::Dev, false),
                    dependency("ts-rs", Kind::Normal, true),
                ],
            ),
            package(USECASES, vec![normal(MONEY)]),
            package(APP, vec![normal("reqwest"), normal(FACADE)]),
            package(NET, vec![normal("reqwest"), normal("csv")]),
            package(SYNC, vec![normal("snow")]),
        ];
        assert!(violations(&packages, TABLE, &allow).is_empty());
    }

    #[test]
    fn parse_reads_workspace_packages_and_dependency_kinds() {
        let json = serde_json::json!({
            "workspace_members": ["a 0.1.0 (path+file:///a)"],
            "packages": [
                {"id": "a 0.1.0 (path+file:///a)", "name": "mwt-money", "dependencies": [
                    {"name": "mwt-model", "kind": null, "optional": false},
                    {"name": "mwt-testkit", "kind": "dev", "optional": false},
                    {"name": "cc", "kind": "build", "optional": true},
                ]},
                {"id": "b 0.1.0 (registry+x)", "name": "serde", "dependencies": []},
            ]
        });
        let packages = parse(&json);
        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].name, "mwt-money");
        let kinds: Vec<Kind> = packages[0]
            .dependencies
            .iter()
            .map(|item| item.kind)
            .collect();
        assert_eq!(kinds, [Kind::Normal, Kind::Dev, Kind::Build]);
        assert!(packages[0].dependencies[2].optional);
    }

    #[test]
    fn a_row_of_the_table_names_only_crates_of_the_table() {
        for row in TABLE {
            for used in row.uses {
                assert!(TABLE.iter().any(|other| other.name == *used), "{used}");
            }
        }
    }
}
