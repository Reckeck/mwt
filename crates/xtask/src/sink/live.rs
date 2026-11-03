//! The live block at the bottom of a terminal: one line per running task.

use std::path::PathBuf;
use std::time::Instant;

use crate::graph::last_line;
use crate::sink::fmt_secs;

pub struct Running {
    pub name: String,
    pub began: Instant,
    pub log: PathBuf,
}

/// Stdout state behind one mutex: what is running and how many lines of it are on the screen.
#[derive(Default)]
pub struct Term {
    pub running: Vec<Running>,
    drawn: usize,
    pub live: bool,
}

impl Term {
    /// The escape sequence that removes the block (cursor up, clear to the end of the screen).
    pub fn erase(&mut self) -> String {
        let drawn = std::mem::take(&mut self.drawn);
        if drawn == 0 {
            String::new()
        } else {
            format!("\u{1b}[{drawn}A\u{1b}[0J")
        }
    }

    /// The block for the current state; empty when not live or nothing runs.
    pub fn draw(&mut self) -> String {
        if !self.live || self.running.is_empty() {
            return String::new();
        }
        let lines = render(&self.running, terminal_width());
        self.drawn = lines.len();
        lines.iter().map(|line| format!("{line}\n")).collect()
    }
}

/// `  <name>  <elapsed>  <last log line>`, each cut to `width - 1` characters (no wrapping).
pub fn render(running: &[Running], width: usize) -> Vec<String> {
    let name_width = running
        .iter()
        .map(|task| task.name.chars().count())
        .max()
        .unwrap_or(0);
    running
        .iter()
        .map(|task| {
            let last = last_line(&task.log, width);
            let line = format!(
                "  {:<name_width$}  {:>7}  {last}",
                task.name,
                fmt_secs(task.began.elapsed())
            );
            line.chars().take(width.saturating_sub(1)).collect()
        })
        .collect()
}

pub fn terminal_width() -> usize {
    #[cfg(unix)]
    {
        // SAFETY: TIOCGWINSZ only fills the zeroed struct we pass.
        let mut size: libc::winsize = unsafe { std::mem::zeroed() };
        if unsafe { libc::ioctl(1, libc::TIOCGWINSZ, &mut size) } == 0 && size.ws_col > 0 {
            return usize::from(size.ws_col);
        }
    }
    std::env::var("COLUMNS")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|width| *width > 0)
        .unwrap_or(100)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_are_cut_to_the_width() {
        let dir = std::env::temp_dir().join(format!("xtask-live-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let log = dir.join("a.log");
        std::fs::write(&log, "first\nBuilding [=>] 5/9: aa, bb\r").unwrap();
        let running = [Running {
            name: "clippy".into(),
            began: Instant::now(),
            log,
        }];
        let wide = render(&running, 100);
        assert!(wide[0].starts_with("  clippy  "));
        assert!(wide[0].ends_with("cargo 5/9: aa, bb"));
        let narrow = render(&running, 20);
        assert_eq!(narrow[0].chars().count(), 19);
    }

    #[test]
    fn erase_matches_what_was_drawn() {
        let mut term = Term {
            live: true,
            ..Term::default()
        };
        assert_eq!(term.erase(), "");
        term.running.push(Running {
            name: "a".into(),
            began: Instant::now(),
            log: PathBuf::from("/nonexistent"),
        });
        assert_eq!(term.draw().lines().count(), 1);
        assert_eq!(term.erase(), "\u{1b}[1A\u{1b}[0J");
        assert_eq!(term.erase(), "");
    }
}
