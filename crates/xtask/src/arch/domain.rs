//! The domain crates must not reach back into the facade: no `Session` from the `service` module, no top-level
//! `crate::service::` path, no `trade_core::` path and no `Books::vault` in their non-test code
//! (core-strangler Z6/D5, core-dismantle D14). `cryptotrack::service` is the crate's own module.
//! A few uses are exempt until the task named in the exemption moves them; entries may only be removed.

use std::collections::BTreeMap;

use super::Source;
use super::scan;

/// The domain crates the rule holds; each is scanned under `crates/<name>/src`.
pub const DOMAIN_CRATES: &[&str] = &[
    "money",
    "household",
    "planning",
    "reports",
    "workflow",
    "cryptotrack",
];

/// Crates with a `service` module of their own: there `crate::service` is theirs, not the facade's.
const OWN_SERVICE: &[&str] = &["cryptotrack", "workflow"];

const SESSION_TYPE: &str = "Session";
const BOOKS_VAULT: &str = "Books::vault";
const VAULT_CALL: &str = ".vault()";
const SERVICE: &str = "service";
const SERVICE_SESSION_PREFIX: &str = "service::";
const FACADE_PATH: &str = "trade_core::";

/// One use that stays until `until` moves it: the file, and the last segment of the `crate::service` path.
pub struct Exemption {
    pub file: &'static str,
    pub item: &'static str,
    /// How many times the file uses the item; one more or fewer fails the check.
    pub uses: usize,
    pub until: &'static str,
    pub reason: &'static str,
}

/// Named exemptions, no blanket ones. Never add an entry: remove it when its task lands.
pub const EXEMPTIONS: &[Exemption] = &[];

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
    /// How many files were scanned, so an empty scan shows.
    pub scanned: usize,
}

impl Findings {
    pub fn failures(&self) -> usize {
        self.violations.len() + self.stale.len()
    }

    pub fn lines(&self) -> Vec<String> {
        let mut lines: Vec<String> = self
            .violations
            .iter()
            .map(|finding| {
                format!(
                    "error: crates/{}:{}: {} is forbidden in the domain crates",
                    finding.file, finding.line, finding.what
                )
            })
            .collect();
        lines.extend(self.stale.iter().map(|entry| {
            format!("error: arch exemption {entry}: no such use any more, delete it")
        }));
        lines.push(format!("arch: {} domain crate files scanned", self.scanned));
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

/// The occurrences of `needle` not preceded by an identifier character.
fn word_start_offsets(text: &str, needle: &str) -> Vec<usize> {
    text.match_indices(needle)
        .map(|(offset, _)| offset)
        .filter(|&offset| {
            !text[..offset]
                .chars()
                .next_back()
                .is_some_and(scan::is_ident_char)
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
fn forbidden_in(text: &str, own_service: bool) -> Vec<(usize, String, Option<String>)> {
    let cleaned = scan::production_text(text);
    let mut found = Vec::new();
    for offset in word_offsets(&cleaned, SESSION_TYPE) {
        if cleaned[..offset].ends_with(SERVICE_SESSION_PREFIX) {
            let what = format!("`{SERVICE_SESSION_PREFIX}{SESSION_TYPE}`");
            found.push((line_of(&cleaned, offset), what, None));
        }
    }
    for offset in word_start_offsets(&cleaned, FACADE_PATH) {
        found.push((line_of(&cleaned, offset), format!("`{FACADE_PATH}`"), None));
    }
    for needle in [BOOKS_VAULT, VAULT_CALL] {
        for (offset, _) in cleaned.match_indices(needle) {
            found.push((line_of(&cleaned, offset), format!("`{needle}`"), None));
        }
    }
    for offset in scan::name_offsets(&cleaned) {
        if !own_service && scan::leading_ident(&cleaned[offset..]) == SERVICE {
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
    let mut used: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    let mut violations = Vec::new();
    for source in sources {
        for (line, what, item) in
            forbidden_in(&source.text, OWN_SERVICE.contains(&source.module.as_str()))
        {
            let exempt = item.as_deref().and_then(|name| {
                exemptions
                    .iter()
                    .position(|entry| entry.file == source.file && entry.item == name)
            });
            match exempt {
                Some(index) => used.entry(index).or_default().push(line),
                None => violations.push(Finding {
                    file: source.file.clone(),
                    line,
                    what,
                }),
            }
        }
    }
    for (index, lines) in &used {
        let entry = &exemptions[*index];
        if lines.len() != entry.uses {
            violations.push(Finding {
                file: entry.file.to_string(),
                line: lines[0],
                what: format!(
                    "`{}` used {} times, the exemption expects {}",
                    entry.item,
                    lines.len(),
                    entry.uses
                ),
            });
        }
    }
    let stale = exemptions
        .iter()
        .enumerate()
        .filter(|(index, _)| !used.contains_key(index))
        .map(|(_, entry)| {
            format!(
                "{} `{}` (until {}: {})",
                entry.file, entry.item, entry.until, entry.reason
            )
        })
        .collect();
    Findings {
        violations,
        stale,
        scanned: sources.len(),
    }
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
        let mut lines = check(sources, exemptions).lines();
        lines.retain(|line| line.starts_with("error:"));
        lines
    }

    #[test]
    fn the_vault_type_through_service_fails_with_file_and_line() {
        let found = messages(
            &[source(
                "money/rates.rs",
                "use std::fmt;\nuse crate::service::Session;\n",
            )],
            NONE,
        );
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(
            found
                .iter()
                .all(|line| line.contains("crates/money/rates.rs:2"))
        );
    }

    #[test]
    fn session_from_the_service_module_fails_but_another_session_does_not() {
        let found = messages(
            &[source("money/a.rs", "use crate::service::Session;\n")],
            NONE,
        );
        assert!(
            found.iter().any(|line| line.contains("`service::Session`")),
            "{found:?}"
        );
        let other = "use crate::clock::Session;\nlet session = Session::new();\n";
        assert!(messages(&[source("money/a.rs", other)], NONE).is_empty());
    }

    #[test]
    fn the_facade_crate_path_fails_and_a_longer_name_does_not() {
        let found = messages(&[source("money/a.rs", "use trade_core::Amount;\n")], NONE);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains("crates/money/a.rs:1"));
        let text = "use my_trade_core::X;\n";
        assert!(messages(&[source("money/a.rs", text)], NONE).is_empty());
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
    fn test_code_comments_and_a_crates_own_service_pass() {
        let text =
            "// service::Session\n#[cfg(test)]\nmod tests {\n    use crate::service::Session;\n}\n";
        assert!(messages(&[source("money/a.rs", text)], NONE).is_empty());
        let own = source("workflow/a.rs", "use crate::service::graph::G;\n");
        assert!(messages(&[own], NONE).is_empty());
    }

    #[test]
    fn a_named_exemption_passes_and_covers_only_its_file_and_item() {
        let exemptions = [Exemption {
            file: "reports/a.rs",
            item: "notice",
            uses: 1,
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
    fn a_second_use_of_an_exempted_item_fails_with_file_item_and_count() {
        let exemptions = [Exemption {
            file: "reports/a.rs",
            item: "notice",
            uses: 1,
            until: "T234",
            reason: "test",
        }];
        let text = "let _ = crate::service::notice(1);\nlet _ = crate::service::notice(2);\n";
        let found = messages(&[source("reports/a.rs", text)], &exemptions);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(
            found[0].contains("reports/a.rs")
                && found[0].contains("`notice`")
                && found[0].contains("used 2 times")
                && found[0].contains("expects 1"),
            "{found:?}"
        );
    }

    #[test]
    fn an_exemption_that_covers_nothing_is_stale() {
        let exemptions = [Exemption {
            file: "reports/a.rs",
            item: "notice",
            uses: 1,
            until: "T234",
            reason: "test",
        }];
        let found = messages(&[source("reports/a.rs", "fn f() {}\n")], &exemptions);
        assert_eq!(found.len(), 1);
        assert!(found[0].contains("T234") && found[0].contains("delete it"));
    }

    #[test]
    fn there_is_no_named_exemption_left() {
        let tasks: Vec<&str> = EXEMPTIONS.iter().map(|entry| entry.until).collect();
        assert!(tasks.is_empty());
        assert!(EXEMPTIONS.iter().all(|entry| !entry.reason.is_empty()));
    }
}
