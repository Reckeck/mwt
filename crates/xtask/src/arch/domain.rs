//! The domain modules must not reach back into the shim: no `UnlockedVault`, no top-level
//! `crate::service::` path and no `Books::vault` in their non-test code (core-strangler Z6/D5).
//! `cryptotrack::service` is the module's own and is not the top-level `service`.
//! A few uses are exempt until the task named in the exemption moves them; entries may only be removed.

use std::collections::BTreeSet;

use super::Source;
use super::scan;

/// The domain modules the rule holds.
const DOMAIN_MODULES: &[&str] = &[
    "money",
    "household",
    "planning",
    "reports",
    "workflow",
    "cryptotrack",
];

const VAULT_TYPE: &str = "UnlockedVault";
const BOOKS_VAULT: &str = "Books::vault";
const VAULT_CALL: &str = ".vault()";
const SERVICE: &str = "service";

/// One use that stays until `until` moves it: the file, and the last segment of the `crate::service` path.
pub struct Exemption {
    pub file: &'static str,
    pub item: &'static str,
    pub until: &'static str,
    pub reason: &'static str,
}

/// Named exemptions, no blanket ones. Never add an entry: remove it when its task lands.
pub const EXEMPTIONS: &[Exemption] = &[
    Exemption {
        file: "reports/overview.rs",
        item: "Flow",
        until: "T227",
        reason: "the overview names the budget Flow of service::budget until Flow moves to planning",
    },
    Exemption {
        file: "reports/overview.rs",
        item: "LossReport",
        until: "T229",
        reason: "the overview carries service::losses::LossReport until the loss report moves to its area",
    },
];

/// One forbidden name found in a domain module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub file: String,
    pub line: usize,
    pub what: String,
}

/// What the rows found: forbidden names no exemption covers, and exemptions that cover nothing.
#[derive(Debug, Default)]
pub struct Findings {
    pub violations: Vec<Finding>,
    /// One description per exemption that covers nothing.
    pub stale: Vec<String>,
}

impl Findings {
    pub fn failures(&self) -> usize {
        self.violations.len() + self.stale.len()
    }

    pub fn lines(&self, core_src: &str) -> Vec<String> {
        let mut lines: Vec<String> = self
            .violations
            .iter()
            .map(|finding| {
                format!(
                    "error: {core_src}/{}:{}: {} is forbidden in the domain modules",
                    finding.file, finding.line, finding.what
                )
            })
            .collect();
        lines.extend(self.stale.iter().map(|entry| {
            format!("error: arch exemption {entry}: no such use any more, delete it")
        }));
        lines
    }
}

fn line_of(text: &str, offset: usize) -> usize {
    text[..offset].matches('\n').count() + 1
}

/// The whole-word occurrences of `needle` in `text`, as offsets.
fn word_offsets(text: &str, needle: &str) -> Vec<usize> {
    text.match_indices(needle)
        .map(|(offset, _)| offset)
        .filter(|&offset| {
            let before = text[..offset].chars().next_back();
            let after = text[offset + needle.len()..].chars().next();
            !before.is_some_and(scan::is_ident_char) && !after.is_some_and(scan::is_ident_char)
        })
        .collect()
}

/// The path after the module name at `offset`, as `a::b::Item`; a `{` group is `{..}`.
fn path_from(text: &str, offset: usize) -> String {
    let mut path = String::new();
    let mut rest = &text[offset..];
    loop {
        let name = scan::leading_ident(rest);
        path.push_str(name);
        rest = &rest[name.len()..];
        match rest.strip_prefix("::") {
            Some(after) if after.starts_with('{') => {
                path.push_str("::{..}");
                break;
            }
            Some(after) => {
                path.push_str("::");
                rest = after;
            }
            None => break,
        }
    }
    path
}

fn last_segment(path: &str) -> &str {
    path.rsplit("::").next().unwrap_or(path)
}

/// The forbidden names of one file: (line, description, `service` item if it is a service path).
fn forbidden_in(text: &str) -> Vec<(usize, String, Option<String>)> {
    let cleaned = scan::production_text(text);
    let mut found = Vec::new();
    for offset in word_offsets(&cleaned, VAULT_TYPE) {
        found.push((line_of(&cleaned, offset), format!("`{VAULT_TYPE}`"), None));
    }
    for needle in [BOOKS_VAULT, VAULT_CALL] {
        for (offset, _) in cleaned.match_indices(needle) {
            found.push((line_of(&cleaned, offset), format!("`{needle}`"), None));
        }
    }
    for offset in scan::name_offsets(&cleaned) {
        if scan::leading_ident(&cleaned[offset..]) == SERVICE {
            let path = path_from(&cleaned, offset);
            let item = last_segment(&path).to_string();
            found.push((
                line_of(&cleaned, offset),
                format!("`crate::{path}`"),
                Some(item),
            ));
        }
    }
    found.sort();
    found
}

/// Holds the non-test files of the domain modules against the rows and the exemptions.
pub fn check(sources: &[Source], exemptions: &[Exemption]) -> Findings {
    let mut used: BTreeSet<usize> = BTreeSet::new();
    let mut violations = Vec::new();
    for source in sources
        .iter()
        .filter(|source| DOMAIN_MODULES.contains(&source.module.as_str()))
    {
        for (line, what, item) in forbidden_in(&source.text) {
            let exempt = item.as_deref().and_then(|name| {
                exemptions
                    .iter()
                    .position(|entry| entry.file == source.file && entry.item == name)
            });
            match exempt {
                Some(index) => {
                    used.insert(index);
                }
                None => violations.push(Finding {
                    file: source.file.clone(),
                    line,
                    what,
                }),
            }
        }
    }
    let stale = exemptions
        .iter()
        .enumerate()
        .filter(|(index, _)| !used.contains(index))
        .map(|(_, entry)| {
            format!(
                "{} `{}` (until {}: {})",
                entry.file, entry.item, entry.until, entry.reason
            )
        })
        .collect();
    Findings { violations, stale }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(file: &str, text: &str) -> Source {
        Source {
            file: file.to_string(),
            module: file.split('/').next().unwrap().to_string(),
            text: text.to_string(),
        }
    }

    const NONE: &[Exemption] = &[];

    fn messages(sources: &[Source], exemptions: &[Exemption]) -> Vec<String> {
        check(sources, exemptions).lines("core")
    }

    #[test]
    fn the_vault_type_through_service_fails_with_file_and_line() {
        let found = messages(
            &[source(
                "money/rates.rs",
                "use std::fmt;\nuse crate::service::UnlockedVault;\n",
            )],
            NONE,
        );
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(
            found
                .iter()
                .all(|line| line.contains("core/money/rates.rs:2"))
        );
    }

    #[test]
    fn books_vault_fails() {
        let found = messages(
            &[source(
                "planning/a.rs",
                "fn f(b: Books) {\n    b.vault();\n}\n",
            )],
            NONE,
        );
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains("planning/a.rs:2"));
        let found = messages(&[source("planning/a.rs", "let _ = Books::vault;\n")], NONE);
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn the_cryptotrack_own_service_is_not_flagged() {
        let text = "use super::service::x;\nuse crate::cryptotrack::service::y;\n";
        assert!(messages(&[source("cryptotrack/a.rs", text)], NONE).is_empty());
    }

    #[test]
    fn a_group_import_of_service_fails() {
        let found = messages(
            &[source(
                "reports/a.rs",
                "use crate::{\n    service::X,\n};\n",
            )],
            NONE,
        );
        assert_eq!(found.len(), 1);
        assert!(found[0].contains("reports/a.rs:2"));
    }

    #[test]
    fn test_code_comments_and_other_modules_pass() {
        let text = "// UnlockedVault\n#[cfg(test)]\nmod tests {\n    use crate::service::UnlockedVault;\n}\n";
        assert!(messages(&[source("money/a.rs", text)], NONE).is_empty());
        let outside = source("service/a.rs", "use crate::service::UnlockedVault;\n");
        assert!(messages(&[outside], NONE).is_empty());
    }

    #[test]
    fn a_named_exemption_passes_and_covers_only_its_file_and_item() {
        let exemptions = [Exemption {
            file: "reports/a.rs",
            item: "notice",
            until: "T234",
            reason: "test",
        }];
        let text = "let _ = crate::service::notice(1);\n";
        assert!(messages(&[source("reports/a.rs", text)], &exemptions).is_empty());
        assert_eq!(
            messages(&[source("reports/b.rs", text)], &exemptions).len(),
            2
        );
        let other = "let _ = crate::service::other(1);\n";
        assert_eq!(
            messages(
                &[source("reports/a.rs", &format!("{text}{other}"))],
                &exemptions
            )
            .len(),
            1
        );
    }

    #[test]
    fn an_exemption_that_covers_nothing_is_stale() {
        let exemptions = [Exemption {
            file: "reports/a.rs",
            item: "notice",
            until: "T234",
            reason: "test",
        }];
        let found = messages(&[source("reports/a.rs", "fn f() {}\n")], &exemptions);
        assert_eq!(found.len(), 1);
        assert!(found[0].contains("T234") && found[0].contains("delete it"));
    }

    #[test]
    fn there_are_exactly_the_two_named_exemptions() {
        let tasks: Vec<&str> = EXEMPTIONS.iter().map(|entry| entry.until).collect();
        assert_eq!(tasks, ["T227", "T229"]);
        assert!(EXEMPTIONS.iter().all(|entry| !entry.reason.is_empty()));
    }
}
