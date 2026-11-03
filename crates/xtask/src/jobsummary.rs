//! The GitHub Job Summary: a markdown section appended to `$GITHUB_STEP_SUMMARY`.

use std::io::Write;
use std::time::Duration;

use crate::graph::{Report, Status};
use crate::sccache::Stats;
use crate::sink::fmt_secs;

fn emoji(status: Status) -> &'static str {
    match status {
        Status::Ok => "✅",
        Status::Fail => "❌",
        Status::Skip => "⏭️",
        Status::Cancelled => "⏹️",
    }
}

fn label(status: Status) -> &'static str {
    match status {
        Status::Ok => "OK",
        Status::Fail => "FAIL",
        Status::Skip => "SKIP",
        Status::Cancelled => "CANCELLED",
    }
}

/// A table cell: no pipes or line breaks, not unbounded.
fn cell(text: &str) -> String {
    let flat: String = text
        .chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .take(300)
        .collect();
    flat.replace('|', "\\|")
}

pub fn markdown(
    cmdline: &str,
    reports: &[Report],
    total: Duration,
    stats: Option<&Stats>,
    bundles: &[(String, u64)],
) -> String {
    let mut out = format!("### xtask {}\n\n", cell(cmdline));
    out.push_str("| task | status | time | note |\n|---|---|---|---|\n");
    for report in reports {
        out.push_str(&format!(
            "| {} | {} {} | {} | {} |\n",
            cell(&report.name),
            emoji(report.status),
            label(report.status),
            fmt_secs(report.took),
            cell(&report.note)
        ));
    }
    out.push_str(&format!("| **total** | | {} | |\n", fmt_secs(total)));
    if let Some(stats) = stats {
        let rate = stats
            .hit_rate()
            .map_or_else(String::new, |rate| format!(", hit rate {rate}%"));
        out.push_str(&format!(
            "\nsccache: {} requests, {} hits, {} misses{rate}\n",
            stats.requests, stats.hits, stats.misses
        ));
    }
    if !bundles.is_empty() {
        out.push_str("\n| bundle | size |\n|---|---|\n");
        for (name, size) in bundles {
            out.push_str(&format!("| {} | {} MB |\n", cell(name), size / 1_000_000));
        }
    }
    out.push('\n');
    out
}

/// Appends to the file named by `$GITHUB_STEP_SUMMARY`, when it is set.
pub fn append(text: &str) {
    let Some(path) = std::env::var_os("GITHUB_STEP_SUMMARY").filter(|path| !path.is_empty()) else {
        return;
    };
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = file.write_all(text.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(name: &str, status: Status, note: &str) -> Report {
        Report {
            name: name.into(),
            status,
            took: Duration::from_millis(1500),
            note: note.into(),
        }
    }

    #[test]
    fn table_with_totals_stats_and_bundles() {
        let reports = [
            report("fmt", Status::Ok, ""),
            report("clippy", Status::Fail, "exited | 1\nline"),
            report("platform:macos", Status::Skip, "no host"),
        ];
        let stats = Stats {
            requests: 10,
            hits: 3,
            misses: 1,
        };
        let text = markdown(
            "check fmt",
            &reports,
            Duration::from_secs(4),
            Some(&stats),
            &[("app_1.0.deb".into(), 12_500_000)],
        );
        assert!(text.starts_with("### xtask check fmt\n"));
        assert!(text.contains("| fmt | ✅ OK | 1.5s |  |"));
        assert!(text.contains("| clippy | ❌ FAIL | 1.5s | exited \\| 1 line |"));
        assert!(text.contains("⏭️ SKIP"));
        assert!(text.contains("| **total** | | 4.0s | |"));
        assert!(text.contains("hit rate 75%"));
        assert!(text.contains("| app_1.0.deb | 12 MB |"));
    }

    #[test]
    fn without_optional_parts() {
        let text = markdown("check", &[], Duration::ZERO, None, &[]);
        assert!(!text.contains("sccache"));
        assert!(!text.contains("bundle"));
    }
}
