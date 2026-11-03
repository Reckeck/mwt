//! Output of a run: the terminal (per output mode) plus the `run.events` file other xtask
//! processes follow when they attach to this run. The events file always gets plain lines.

mod live;

use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::annotate::{MAX_ANNOTATIONS, annotations, parse_locations};
use crate::graph::{Status, tail};
use crate::logtext::clean_log;
use crate::output::{Mode, Output, Verbosity};
use live::{Running, Term};

const TAIL_LINES: usize = 40;
const LIVE_EVERY: Duration = Duration::from_millis(500);

/// A task that has just ended.
pub struct TaskEnd<'a> {
    pub name: &'a str,
    pub status: Status,
    pub took: Duration,
    /// The note of SKIP and FAIL.
    pub reason: &'a str,
    pub root: &'a Path,
    pub log: &'a Path,
    /// `name (log path)`, shown on FAIL lines.
    pub log_hint: &'a str,
}

pub struct Sink {
    start: Instant,
    events: Mutex<Option<File>>,
    output: Output,
    term: Mutex<Term>,
}

impl Sink {
    #[cfg(test)]
    pub fn new(events: Option<File>) -> Self {
        Self::with_output(events, Output::plain())
    }

    pub fn with_output(events: Option<File>, output: Output) -> Self {
        Self {
            start: Instant::now(),
            events: Mutex::new(events),
            output,
            term: Mutex::new(Term::default()),
        }
    }

    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }

    fn stamp(&self) -> String {
        format!("[{:>6.1}s] ", self.elapsed().as_secs_f64())
    }

    /// One event line with the time since the start of the run.
    pub fn line(&self, text: &str) {
        self.raw(&format!("{}{text}", self.stamp()));
    }

    /// Text as is (tables, log tails); every line goes to the terminal and the events file.
    pub fn raw(&self, text: &str) {
        let out = lines_of(text);
        self.emit(&out, &out);
    }

    /// The last record of a run: attached processes exit with this code.
    pub fn exit_marker(&self, code: i32) {
        self.write_events(&format!("EXIT {code}\n"));
    }

    /// Writes to stdout (under the live block, if any) and the events file.
    fn emit(&self, stdout: &str, events: &str) {
        if let Ok(mut term) = self.term.lock() {
            let mut buf = term.erase();
            buf.push_str(stdout);
            buf.push_str(&term.draw());
            print!("{buf}");
            let _ = std::io::stdout().flush();
        }
        self.write_events(events);
    }

    fn write_events(&self, text: &str) {
        if text.is_empty() {
            return;
        }
        if let Ok(mut guard) = self.events.lock()
            && let Some(file) = guard.as_mut()
        {
            let _ = file.write_all(text.as_bytes());
            let _ = file.flush();
        }
    }

    pub fn task_start(&self, name: &str, log: &Path) {
        if let Ok(mut term) = self.term.lock() {
            term.running.push(Running {
                name: name.to_string(),
                began: Instant::now(),
                log: log.to_path_buf(),
            });
        }
        let line = format!("{}start  {name}\n", self.stamp());
        let shown = if self.output.verbosity == Verbosity::Quiet {
            ""
        } else {
            &line
        };
        self.emit(shown, &line);
    }

    pub fn task_end(&self, end: &TaskEnd) {
        if let Ok(mut term) = self.term.lock() {
            term.running.retain(|task| task.name != end.name);
        }
        let stamp = self.stamp();
        let events = plain_text(end, &stamp);
        let shown = match self.output.mode {
            Mode::Github => github_text(end, self.output.verbosity),
            Mode::Plain | Mode::Tty => match self.output.verbosity {
                Verbosity::Quiet if end.status != Status::Fail => String::new(),
                Verbosity::Verbose => verbose_text(end, &stamp),
                _ => events.clone(),
            },
        };
        self.emit(&shown, &events);
    }

    /// Whether a silent task should report itself every minute.
    pub fn wants_progress(&self) -> bool {
        match self.output.mode {
            _ if self.output.verbosity == Verbosity::Quiet => false,
            Mode::Tty => false,
            Mode::Github => true,
            Mode::Plain => std::env::var_os("CI").is_some(),
        }
    }

    pub fn progress(&self, name: &str, took: Duration, last: &str) {
        self.line(&format!("...    {name} {}: {last}", fmt_secs(took)));
    }

    pub fn is_tty(&self) -> bool {
        self.output.mode == Mode::Tty
    }

    /// Starts redrawing the live block (a terminal only).
    pub fn start_live(self: &Arc<Self>) {
        if !self.is_tty() {
            return;
        }
        if let Ok(mut term) = self.term.lock() {
            term.live = true;
        }
        let sink = self.clone();
        tokio::spawn(async move {
            let mut ticks = tokio::time::interval(LIVE_EVERY);
            loop {
                ticks.tick().await;
                if !sink.redraw() {
                    break;
                }
            }
        });
    }

    fn redraw(&self) -> bool {
        let Ok(mut term) = self.term.lock() else {
            return false;
        };
        if !term.live {
            return false;
        }
        let mut buf = term.erase();
        buf.push_str(&term.draw());
        print!("{buf}");
        let _ = std::io::stdout().flush();
        true
    }

    /// Removes the live block for good.
    pub fn stop_live(&self) {
        if let Ok(mut term) = self.term.lock()
            && term.live
        {
            term.live = false;
            print!("{}", term.erase());
            let _ = std::io::stdout().flush();
        }
    }
}

/// Every line ends with a newline.
fn lines_of(text: &str) -> String {
    text.lines().map(|line| format!("{line}\n")).collect()
}

fn result_line(end: &TaskEnd) -> String {
    let took = fmt_secs(end.took);
    match end.status {
        Status::Ok => format!("OK     {} {took}", end.name),
        Status::Skip => format!("SKIP   {}: {}", end.name, end.reason),
        Status::Fail => format!("FAIL   {} {took}: {}", end.log_hint, end.reason),
        Status::Cancelled => format!("CANCELLED {} {took}", end.name),
    }
}

/// The lines the events file gets (and plain mode prints).
fn plain_text(end: &TaskEnd, stamp: &str) -> String {
    let mut out = format!("{stamp}{}\n", result_line(end));
    if end.status == Status::Fail {
        out.push_str(&tail(end.log, TAIL_LINES));
    }
    out
}

fn read_clean(log: &Path) -> String {
    clean_log(&std::fs::read_to_string(log).unwrap_or_default())
}

fn verbose_text(end: &TaskEnd, stamp: &str) -> String {
    let mut out = format!("{stamp}{}\n", result_line(end));
    for line in read_clean(end.log).lines() {
        out.push_str("    ");
        out.push_str(line);
        out.push('\n');
    }
    out
}

fn github_text(end: &TaskEnd, verbosity: Verbosity) -> String {
    let failed = end.status == Status::Fail;
    if verbosity == Verbosity::Quiet && !failed {
        return String::new();
    }
    let took = fmt_secs(end.took);
    let header = match end.status {
        Status::Ok => format!("OK {} {took}", end.name),
        Status::Skip => format!("SKIP {}: {}", end.name, end.reason),
        Status::Fail => format!("FAIL {} {took}: {}", end.name, end.reason),
        Status::Cancelled => format!("CANCELLED {} {took}", end.name),
    };
    let log = read_clean(end.log);
    let mut out = format!(
        "::group::{header}\n{}\n::endgroup::\n",
        log.trim_end_matches('\n')
    );
    if failed {
        out.push_str(&tail(end.log, TAIL_LINES));
        let is_file = |file: &str| end.root.join(file).is_file();
        let mut found = parse_locations(&log, end.root, &is_file);
        found.truncate(MAX_ANNOTATIONS);
        for line in annotations(end.name, end.reason, &found) {
            out.push_str(&line);
            out.push('\n');
        }
    }
    out
}

pub fn fmt_secs(duration: Duration) -> String {
    format!("{:.1}s", duration.as_secs_f64())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn end<'a>(status: Status, log: &'a Path, root: &'a Path) -> TaskEnd<'a> {
        TaskEnd {
            name: "biome",
            status,
            took: Duration::from_millis(2500),
            reason: "exited with 1",
            root,
            log,
            log_hint: "biome (.cache/xtask/biome.log)",
        }
    }

    #[test]
    fn plain_lines_keep_their_shape() {
        let log = Path::new("/nonexistent.log");
        let root = Path::new("/");
        assert_eq!(
            plain_text(&end(Status::Ok, log, root), "[ 1.0s] "),
            "[ 1.0s] OK     biome 2.5s\n"
        );
        let fail = plain_text(&end(Status::Fail, log, root), "[ 1.0s] ");
        assert!(
            fail.starts_with("[ 1.0s] FAIL   biome (.cache/xtask/biome.log) 2.5s: exited with 1\n")
        );
        assert!(fail.contains("---- tail of the log ----"));
    }

    #[test]
    fn github_failure_has_group_tail_and_annotation() {
        let dir = std::env::temp_dir().join(format!("xtask-sink-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src/a.ts"), "").unwrap();
        let log = dir.join("t.log");
        std::fs::write(&log, "\u{1b}[1mwork\u{1b}[0m\rsrc/a.ts:3:4 lint/x boom\n").unwrap();
        let text = github_text(&end(Status::Fail, &log, &dir), Verbosity::Normal);
        assert!(text.starts_with("::group::FAIL biome 2.5s: exited with 1\n"));
        assert!(text.contains("\n::endgroup::\n"));
        assert!(text.contains("---- tail of the log ----"));
        assert!(text.contains(
            "::error file=src/a.ts,line=3,col=4,title=biome::src/a.ts:3:4 lint/x boom\n"
        ));
        let ok = github_text(&end(Status::Ok, &log, &dir), Verbosity::Normal);
        assert!(ok.starts_with("::group::OK biome 2.5s\n") && !ok.contains("::error"));
        assert_eq!(
            github_text(&end(Status::Ok, &log, &dir), Verbosity::Quiet),
            ""
        );
    }

    #[test]
    fn verbose_indents_the_log() {
        let dir = std::env::temp_dir().join(format!("xtask-sink-v-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let log = dir.join("t.log");
        std::fs::write(&log, "one\ntwo\n").unwrap();
        let text = verbose_text(&end(Status::Ok, &log, &dir), "[ 1.0s] ");
        assert_eq!(text, "[ 1.0s] OK     biome 2.5s\n    one\n    two\n");
    }
}
