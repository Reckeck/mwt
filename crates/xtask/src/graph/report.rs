//! Task selection by name and the end-of-run output (log tail, summary table).

use std::collections::{BTreeSet, HashMap};
use std::path::Path;
use std::time::Duration;

use super::{Report, Status, Task};
use crate::sink::{Sink, fmt_secs};

/// Keeps `names` and everything they depend on, in the original order.
pub fn select(tasks: Vec<Task>, names: &[String]) -> anyhow::Result<Vec<Task>> {
    if names.is_empty() {
        return Ok(tasks);
    }
    let known: HashMap<&str, &Task> = tasks
        .iter()
        .map(|task| (task.name.as_str(), task))
        .collect();
    let mut wanted = BTreeSet::new();
    let mut stack: Vec<&str> = Vec::new();
    for name in names {
        let Some((key, _)) = known.get_key_value(name.as_str()) else {
            let all: Vec<&str> = tasks.iter().map(|task| task.name.as_str()).collect();
            anyhow::bail!("unknown task `{name}` (one of: {})", all.join(", "));
        };
        stack.push(key);
    }
    while let Some(name) = stack.pop() {
        if wanted.insert(name.to_string()) {
            stack.extend(known[name].deps.iter().map(String::as_str));
        }
    }
    Ok(tasks
        .into_iter()
        .filter(|task| wanted.contains(&task.name))
        .collect())
}

pub fn rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

/// The last non-empty line of the log (progress bars split on `\r`), cut to `width` chars.
pub fn last_line(log: &Path, width: usize) -> String {
    let text = std::fs::read_to_string(log).unwrap_or_default();
    let line = text
        .rsplit(['\n', '\r'])
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("");
    line.chars().take(width).collect()
}

pub fn tail(log: &Path, lines: usize) -> String {
    let text = std::fs::read_to_string(log).unwrap_or_default();
    let all: Vec<&str> = text.lines().collect();
    let from = all.len().saturating_sub(lines);
    let mut out = String::from("    ---- tail of the log ----\n");
    for line in &all[from..] {
        out.push_str("    | ");
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// The final table; the exit code is decided by the caller.
pub fn summary(reports: &[Report], total: Duration, sink: &Sink) {
    let mut out = String::from("\n");
    for report in reports {
        let label = match report.status {
            Status::Ok => "OK",
            Status::Skip => "SKIP",
            Status::Fail => "FAIL",
            Status::Cancelled => "CANCELLED",
        };
        let note = if report.note.is_empty() {
            String::new()
        } else {
            format!("  {}", report.note)
        };
        out.push_str(&format!(
            "  {:<20} {:<10} {:>8}{note}\n",
            report.name,
            label,
            fmt_secs(report.took)
        ));
    }
    out.push_str(&format!(
        "  {:<20} {:<10} {:>8}\n",
        "total",
        "",
        fmt_secs(total)
    ));
    sink.raw(&out);
}
