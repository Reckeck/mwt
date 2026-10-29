//! Output of a run: the terminal plus the `run.events` file other xtask
//! processes follow when they attach to this run.

use std::fs::File;
use std::io::Write;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub struct Sink {
    start: Instant,
    events: Mutex<Option<File>>,
}

impl Sink {
    pub fn new(events: Option<File>) -> Self {
        Self {
            start: Instant::now(),
            events: Mutex::new(events),
        }
    }

    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }

    /// One event line with the time since the start of the run.
    pub fn line(&self, text: &str) {
        let stamp = format!("[{:>6.1}s] ", self.elapsed().as_secs_f64());
        self.raw(&format!("{stamp}{text}"));
    }

    /// Text as is (tables, log tails); every line goes to the terminal and the events file.
    pub fn raw(&self, text: &str) {
        let mut out = String::new();
        for line in text.lines() {
            out.push_str(line);
            out.push('\n');
        }
        print!("{out}");
        let _ = std::io::stdout().flush();
        self.write_events(&out);
    }

    /// The last record of a run: attached processes exit with this code.
    pub fn exit_marker(&self, code: i32) {
        self.write_events(&format!("EXIT {code}\n"));
    }

    fn write_events(&self, text: &str) {
        if let Ok(mut guard) = self.events.lock()
            && let Some(file) = guard.as_mut()
        {
            let _ = file.write_all(text.as_bytes());
            let _ = file.flush();
        }
    }
}

pub fn fmt_secs(duration: Duration) -> String {
    format!("{:.1}s", duration.as_secs_f64())
}
