//! `cargo xtask audit`: dependency advisories of Rust (cargo-audit) and npm (pnpm audit),
//! minus the exceptions of `audit-ignore.toml`, each with a reason.

use std::collections::HashSet;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::Value;

const IGNORE_FILE: &str = "audit-ignore.toml";
const CARGO_AUDIT_INSTALL: &str = "cargo install cargo-audit --locked --version 0.22.2";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Cargo,
    Pnpm,
}

impl Tool {
    fn name(self) -> &'static str {
        match self {
            Self::Cargo => "cargo",
            Self::Pnpm => "pnpm",
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEntry {
    id: String,
    tool: String,
    reason: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFile {
    #[serde(default)]
    ignore: Vec<RawEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ignore {
    pub id: String,
    pub tool: Tool,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Vulnerability,
    Unsound,
    Yanked,
    Unmaintained,
    /// Any other informational warning of cargo-audit.
    Notice,
    /// pnpm: high or critical.
    Severe,
    /// pnpm: below high.
    Minor,
}

impl Kind {
    fn fails(self) -> bool {
        matches!(
            self,
            Self::Vulnerability | Self::Unsound | Self::Yanked | Self::Severe
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub id: String,
    pub aliases: Vec<String>,
    pub package: String,
    pub kind: Kind,
    pub title: String,
}

impl Finding {
    fn line(&self) -> String {
        let title = if self.title.is_empty() {
            String::new()
        } else {
            format!(": {}", self.title)
        };
        format!("{} {} ({:?}){title}", self.id, self.package, self.kind)
    }
}

#[derive(Debug, Default)]
pub struct Verdict {
    pub failing: Vec<Finding>,
    pub ignored: Vec<(Finding, String)>,
    pub info: Vec<Finding>,
    /// Ignore ids of this tool that suppressed nothing.
    pub stale: Vec<String>,
}

pub fn parse_ignores(text: &str) -> Result<Vec<Ignore>> {
    let raw: RawFile = toml::from_str(text).map_err(|source| anyhow::anyhow!("{source}"))?;
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for entry in raw.ignore {
        let id = entry.id.trim().to_string();
        if id.is_empty() {
            bail!("an [[ignore]] entry has an empty id");
        }
        if entry.reason.trim().is_empty() {
            bail!("[[ignore]] {id}: reason is empty (say why the advisory is accepted)");
        }
        let tool = match entry.tool.as_str() {
            "cargo" => Tool::Cargo,
            "pnpm" => Tool::Pnpm,
            other => bail!("[[ignore]] {id}: unknown tool `{other}` (cargo or pnpm)"),
        };
        if !seen.insert(id.clone()) {
            bail!("[[ignore]] {id}: listed twice");
        }
        out.push(Ignore {
            id,
            tool,
            reason: entry.reason.trim().to_string(),
        });
    }
    Ok(out)
}

fn text(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// Findings of `cargo audit --json`: vulnerabilities and every kind of warning.
pub fn parse_cargo(json: &str) -> Result<Vec<Finding>> {
    let root: Value = serde_json::from_str(json).context("cargo audit printed no JSON")?;
    let mut out = Vec::new();
    let list = root
        .pointer("/vulnerabilities/list")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut entries: Vec<(Option<Kind>, Value)> = list
        .into_iter()
        .map(|item| (Some(Kind::Vulnerability), item))
        .collect();
    if let Some(groups) = root.get("warnings").and_then(Value::as_object) {
        for (name, items) in groups {
            let kind = match name.as_str() {
                "unsound" => Kind::Unsound,
                "yanked" => Kind::Yanked,
                "unmaintained" => Kind::Unmaintained,
                _ => Kind::Notice,
            };
            for item in items.as_array().into_iter().flatten() {
                entries.push((Some(kind), item.clone()));
            }
        }
    }
    for (kind, item) in entries {
        let Some(kind) = kind else { continue };
        let package = item.get("package").cloned().unwrap_or(Value::Null);
        let name = text(&package, "name");
        let version = text(&package, "version");
        let advisory = item.get("advisory").filter(|value| !value.is_null());
        let (id, title, aliases) = match advisory {
            Some(advisory) => (
                text(advisory, "id"),
                text(advisory, "title"),
                advisory
                    .get("aliases")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|alias| alias.as_str().map(String::from))
                    .collect(),
            ),
            None => (
                format!("{name}@{version}"),
                "yanked release".to_string(),
                Vec::new(),
            ),
        };
        out.push(Finding {
            id,
            aliases,
            package: format!("{name} {version}"),
            kind,
            title,
        });
    }
    Ok(out)
}

/// Findings of `pnpm audit --json`.
pub fn parse_pnpm(json: &str) -> Result<Vec<Finding>> {
    let root: Value = serde_json::from_str(json).context("pnpm audit printed no JSON")?;
    if let Some(error) = root.get("error") {
        bail!("pnpm audit failed: {error}");
    }
    let mut out = Vec::new();
    if let Some(advisories) = root.get("advisories").and_then(Value::as_object) {
        for (key, item) in advisories {
            let mut id = text(item, "github_advisory_id");
            if id.is_empty() {
                id = key.clone();
            }
            let kind = match text(item, "severity").as_str() {
                "high" | "critical" => Kind::Severe,
                _ => Kind::Minor,
            };
            out.push(Finding {
                id,
                aliases: Vec::new(),
                package: text(item, "module_name"),
                kind,
                title: text(item, "title"),
            });
        }
    }
    Ok(out)
}

/// Splits findings into failing, ignored (with the reason), information; reports stale ignores.
pub fn classify(findings: Vec<Finding>, tool: Tool, ignores: &[Ignore]) -> Verdict {
    let mine: Vec<&Ignore> = ignores.iter().filter(|entry| entry.tool == tool).collect();
    let mut used = HashSet::new();
    let mut verdict = Verdict::default();
    for finding in findings {
        let hit = mine
            .iter()
            .find(|entry| entry.id == finding.id || finding.aliases.contains(&entry.id));
        if let Some(entry) = hit {
            used.insert(entry.id.clone());
            verdict.ignored.push((finding, entry.reason.clone()));
        } else if finding.kind.fails() {
            verdict.failing.push(finding);
        } else {
            verdict.info.push(finding);
        }
    }
    verdict.stale = mine
        .iter()
        .filter(|entry| !used.contains(&entry.id))
        .map(|entry| entry.id.clone())
        .collect();
    verdict
}

fn run_tool(root: &Path, program: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(program)
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|source| anyhow::anyhow!("cannot start `{program}`: {source}"))?;
    // Both tools exit non-zero when they find something; the JSON on stdout is what counts.
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn cargo_audit_installed() -> bool {
    Command::new("cargo")
        .args(["audit", "--version"])
        .output()
        .is_ok_and(|output| output.status.success())
}

fn report(tool: Tool, verdict: &Verdict) {
    let name = tool.name();
    for finding in &verdict.failing {
        println!("FAIL    [{name}] {}", finding.line());
    }
    for (finding, reason) in &verdict.ignored {
        println!(
            "ignored [{name}] {}\n          reason: {reason}",
            finding.line()
        );
    }
    for finding in &verdict.info {
        println!("info    [{name}] {}", finding.line());
    }
    for id in &verdict.stale {
        println!("warning [{name}] {IGNORE_FILE}: {id} suppresses nothing any more, remove it");
    }
}

pub fn run(root: &Path) -> Result<i32> {
    let path = root.join(IGNORE_FILE);
    let ignores = match std::fs::read_to_string(&path) {
        Ok(content) => parse_ignores(&content).with_context(|| IGNORE_FILE.to_string())?,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(source) => return Err(source).with_context(|| format!("read {IGNORE_FILE}")),
    };
    if !cargo_audit_installed() {
        bail!("cargo-audit is not installed; install: {CARGO_AUDIT_INSTALL}");
    }
    // The ignores are applied here, not with `--ignore`, so the report can say what was accepted.
    let cargo_json = run_tool(
        root,
        "cargo",
        &["audit", "--json", "--deny", "unsound", "--deny", "yanked"],
    )?;
    let cargo = classify(parse_cargo(&cargo_json)?, Tool::Cargo, &ignores);
    report(Tool::Cargo, &cargo);

    let pnpm_json = run_tool(root, "pnpm", &["audit", "--json", "--audit-level", "high"])?;
    let pnpm = classify(parse_pnpm(&pnpm_json)?, Tool::Pnpm, &ignores);
    report(Tool::Pnpm, &pnpm);

    let failing = cargo.failing.len() + pnpm.failing.len();
    let ignored = cargo.ignored.len() + pnpm.ignored.len();
    let info = cargo.info.len() + pnpm.info.len();
    println!("audit: {failing} failing, {ignored} ignored, {info} informational");
    Ok(i32::from(failing > 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    const GLIB: &str = r#"{"vulnerabilities":{"found":false,"count":0,"list":[]},
      "warnings":{
        "unsound":[{"kind":"unsound","package":{"name":"glib","version":"0.18.5"},
          "advisory":{"id":"RUSTSEC-2024-0429","title":"Unsoundness","aliases":["GHSA-wrw7-89jp-8q8g"]}}],
        "unmaintained":[{"kind":"unmaintained","package":{"name":"bincode","version":"2.0.1"},
          "advisory":{"id":"RUSTSEC-2025-0141","title":"unmaintained","aliases":[]}}],
        "yanked":[{"kind":"yanked","package":{"name":"foo","version":"1.0.0"},"advisory":null}]}}"#;

    fn ignore(id: &str, tool: Tool) -> Ignore {
        Ignore {
            id: id.into(),
            tool,
            reason: "because".into(),
        }
    }

    #[test]
    fn ignore_file_parses() {
        let text = "[[ignore]]\nid = \"A\"\ntool = \"cargo\"\nreason = \"why\"\n";
        let list = parse_ignores(text).unwrap();
        assert_eq!(
            list,
            vec![Ignore {
                id: "A".into(),
                tool: Tool::Cargo,
                reason: "why".into()
            }]
        );
        assert!(parse_ignores("").unwrap().is_empty());
    }

    #[test]
    fn ignore_file_rejects_bad_entries() {
        let entry = |extra: &str| format!("[[ignore]]\nid = \"A\"\n{extra}");
        let empty = parse_ignores(&entry("tool = \"cargo\"\nreason = \"  \"\n"));
        assert!(empty.unwrap_err().to_string().contains("reason is empty"));
        let missing = parse_ignores(&entry("tool = \"cargo\"\n"));
        assert!(missing.is_err());
        let tool = parse_ignores(&entry("tool = \"npm\"\nreason = \"x\"\n"));
        assert!(tool.unwrap_err().to_string().contains("unknown tool"));
        let twice = format!(
            "{}\n{}",
            entry("tool = \"cargo\"\nreason = \"x\"\n"),
            entry("tool = \"pnpm\"\nreason = \"x\"\n")
        );
        assert!(
            parse_ignores(&twice)
                .unwrap_err()
                .to_string()
                .contains("twice")
        );
    }

    #[test]
    fn cargo_findings_are_classified() {
        let findings = parse_cargo(GLIB).unwrap();
        assert_eq!(findings.len(), 3);
        let bare = classify(findings.clone(), Tool::Cargo, &[]);
        let failing: Vec<_> = bare.failing.iter().map(|item| item.id.as_str()).collect();
        assert_eq!(failing, vec!["RUSTSEC-2024-0429", "foo@1.0.0"]);
        assert_eq!(bare.info.len(), 1);

        let accepted = classify(
            findings,
            Tool::Cargo,
            &[ignore("RUSTSEC-2024-0429", Tool::Cargo)],
        );
        assert_eq!(accepted.ignored.len(), 1);
        assert_eq!(accepted.failing.len(), 1);
        assert!(accepted.stale.is_empty());
    }

    #[test]
    fn ignore_by_alias_and_stale_entries() {
        let findings = parse_cargo(GLIB).unwrap();
        let ignores = [
            ignore("GHSA-wrw7-89jp-8q8g", Tool::Cargo),
            ignore("RUSTSEC-0000-0000", Tool::Cargo),
            ignore("GHSA-other", Tool::Pnpm),
        ];
        let verdict = classify(findings, Tool::Cargo, &ignores);
        assert_eq!(verdict.ignored.len(), 1);
        assert_eq!(verdict.stale, vec!["RUSTSEC-0000-0000"]);
    }

    #[test]
    fn vulnerabilities_fail() {
        let json = r#"{"vulnerabilities":{"found":true,"count":1,"list":[{"package":{"name":"x","version":"1"},
          "advisory":{"id":"RUSTSEC-1","title":"bad","aliases":[]}}]},"warnings":{}}"#;
        let verdict = classify(parse_cargo(json).unwrap(), Tool::Cargo, &[]);
        assert_eq!(verdict.failing.len(), 1);
    }

    #[test]
    fn pnpm_severity_decides() {
        let json = r#"{"advisories":{"1":{"github_advisory_id":"GHSA-a","module_name":"left","severity":"high","title":"t"},
          "2":{"github_advisory_id":"GHSA-b","module_name":"pad","severity":"moderate","title":"t"}}}"#;
        let found = parse_pnpm(json).unwrap();
        let verdict = classify(found.clone(), Tool::Pnpm, &[]);
        assert_eq!(verdict.failing.len(), 1);
        assert_eq!(verdict.info.len(), 1);
        let accepted = classify(found, Tool::Pnpm, &[ignore("GHSA-a", Tool::Pnpm)]);
        assert!(accepted.failing.is_empty());
        assert!(parse_pnpm(r#"{"error":{"code":"x"}}"#).is_err());
        assert!(parse_pnpm(r#"{"advisories":{}}"#).unwrap().is_empty());
    }
}
