//! GitHub error annotations: source locations found in a failed task's log.

use std::path::Path;

use crate::logtext::strip_ansi;

/// At most this many annotations per task (GitHub shows ten per step anyway).
pub const MAX_ANNOTATIONS: usize = 10;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Location {
    /// Relative to the repository root.
    pub file: String,
    pub line: u32,
    pub col: Option<u32>,
    pub message: String,
}

/// Locations in a log: rustc/clippy (`error[..]: msg` + `--> path:l:c`), `path:l:c` and
/// `path(l,c)` forms (biome, vue-tsc, vitest) and the repo's static checks (`error: file:line: msg`).
/// Only files that exist in the repository count (`is_file` gets the path relative to `root`).
pub fn parse_locations(text: &str, root: &Path, is_file: &dyn Fn(&str) -> bool) -> Vec<Location> {
    let text = strip_ansi(text);
    let mut found: Vec<Location> = Vec::new();
    let mut pending: Option<String> = None;
    // The last `Error: ...` line: a stack frame below it points at the place, the line says what failed.
    let mut thrown: Option<String> = None;
    let mut push = |file: &str, line: u32, col: Option<u32>, message: &str| {
        let Some(file) = relative(file, root).filter(|file| is_file(file)) else {
            return;
        };
        let known = found
            .iter()
            .any(|seen| seen.file == file && seen.line == line && seen.col == col);
        if !known {
            found.push(Location {
                file,
                line,
                col,
                message: message.chars().take(300).collect(),
            });
        }
    };
    for line in text.lines() {
        if let Some((file, number, message)) = static_check(line) {
            pending = None;
            push(file, number, None, message);
        } else if let Some(arrow) = line.trim_start().strip_prefix("--> ") {
            if let Some(message) = pending.take()
                && let Some((file, number, col)) = split_position(arrow.trim())
            {
                push(file, number, col, &message);
            }
        } else if let Some(message) = rustc_header(line) {
            pending = Some(message.to_string());
        } else {
            pending = None;
            if let Some(error) = thrown_error(line) {
                thrown = Some(error.to_string());
            }
            if let Some((file, number, col)) = generic(line) {
                let message = match &thrown {
                    Some(error) if is_stack_frame(line) => error.as_str(),
                    _ => line.trim(),
                };
                push(file, number, col, message);
            }
        }
    }
    found
}

/// `Error: msg`, `AssertionError: msg`, `TypeError: msg` (JS, vitest), trimmed.
fn thrown_error(line: &str) -> Option<&str> {
    let line = line.trim();
    let (name, _) = line.split_once(": ")?;
    (name.ends_with("Error") && name.chars().all(char::is_alphanumeric)).then_some(line)
}

/// A JS stack frame as vitest (`❯ fn path:l:c`) or node (`at fn (path:l:c)`) prints it.
fn is_stack_frame(line: &str) -> bool {
    let line = line.trim_start();
    line.starts_with('❯') || line.starts_with("at ")
}

fn static_check(line: &str) -> Option<(&str, u32, &str)> {
    let rest = line.trim_start().strip_prefix("error: ")?;
    let (file, rest) = rest.split_once(':')?;
    let (number, message) = rest.split_once(": ")?;
    if file.contains(char::is_whitespace) {
        return None;
    }
    Some((file, number.parse().ok()?, message))
}

fn rustc_header(line: &str) -> Option<&str> {
    let rest = line
        .strip_prefix("error")
        .or_else(|| line.strip_prefix("warning"))?;
    let rest = match rest.strip_prefix('[') {
        Some(code) => code.split_once(']')?.1,
        None => rest,
    };
    rest.strip_prefix(": ")
}

/// `path:line:col` or `path:line`.
fn split_position(text: &str) -> Option<(&str, u32, Option<u32>)> {
    let (head, last) = text.rsplit_once(':')?;
    let last: u32 = last.parse().ok()?;
    if let Some((file, middle)) = head.rsplit_once(':')
        && let Ok(number) = middle.parse::<u32>()
    {
        return Some((file, number, Some(last)));
    }
    Some((head, last, None))
}

fn generic(line: &str) -> Option<(&str, u32, Option<u32>)> {
    line.split_whitespace().find_map(|token| {
        let token =
            token.trim_start_matches(|ch: char| !ch.is_alphanumeric() && ch != '.' && ch != '/');
        let token = token.trim_end_matches([':', ',', ')', '\'', '"']);
        if let Some((file, inside)) = token.split_once('(') {
            // vue-tsc: `path(line,col)` with the `)` trimmed above.
            let (number, col) = inside.split_once(',')?;
            return Some((file, number.parse().ok()?, col.parse().ok()));
        }
        split_position(token)
    })
}

fn relative(file: &str, root: &Path) -> Option<String> {
    let file = file.strip_prefix("file://").unwrap_or(file);
    let file = Path::new(file)
        .strip_prefix(root)
        .unwrap_or(Path::new(file));
    let file = file.to_str()?.trim_start_matches("./");
    (!file.is_empty() && !file.starts_with('/') && !file.contains("..")).then(|| file.to_string())
}

/// The message part of a workflow command.
pub fn escape_data(text: &str) -> String {
    text.replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

/// A property value of a workflow command.
pub fn escape_property(text: &str) -> String {
    escape_data(text).replace(':', "%3A").replace(',', "%2C")
}

/// The `::error` lines of a failed task: one per location, or one with the reason.
pub fn annotations(task: &str, reason: &str, locations: &[Location]) -> Vec<String> {
    let title = escape_property(task);
    if locations.is_empty() {
        return vec![format!("::error title={title}::{}", escape_data(reason))];
    }
    locations
        .iter()
        .take(MAX_ANNOTATIONS)
        .map(|location| {
            let col = location
                .col
                .map(|col| format!(",col={col}"))
                .unwrap_or_default();
            format!(
                "::error file={},line={}{col},title={title}::{}",
                escape_property(&location.file),
                location.line,
                escape_data(&location.message)
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Vec<Location> {
        let known = [
            "src/a.ts",
            "src/b.vue",
            "crates/core/src/lib.rs",
            "tests/x.test.ts",
        ];
        parse_locations(text, Path::new("/repo"), &|file| known.contains(&file))
    }

    #[test]
    fn a_stack_frame_takes_the_message_of_the_error_above_it() {
        let text =
            "Error: page.eval hung for 60000 ms\n ❯ Timeout.<anonymous> tests/x.test.ts:114:16\n";
        let found = parse(text);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].file, "tests/x.test.ts");
        assert_eq!(found[0].line, 114);
        assert_eq!(found[0].message, "Error: page.eval hung for 60000 ms");
    }

    #[test]
    fn rustc_and_clippy() {
        let text = "error[E0425]: cannot find value `x`\n  --> crates/core/src/lib.rs:12:5\n   |\n";
        assert_eq!(
            parse(text),
            vec![Location {
                file: "crates/core/src/lib.rs".into(),
                line: 12,
                col: Some(5),
                message: "cannot find value `x`".into()
            }]
        );
        let clippy = "\u{1b}[1merror\u{1b}[0m: this is bad\n --> /repo/crates/core/src/lib.rs:3:1\nerror: could not compile `x`\n";
        let found = parse(clippy);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].file, "crates/core/src/lib.rs");
        assert_eq!(found[0].message, "this is bad");
    }

    #[test]
    fn biome_vue_tsc_and_vitest() {
        let found = parse("src/a.ts:7:3 lint/style/x  FIXABLE  ━━━\n");
        assert_eq!((found[0].line, found[0].col), (7, Some(3)));
        let found = parse("src/b.vue(4,9): error TS2322: Type 'a' is not assignable\n");
        assert_eq!(found[0].file, "src/b.vue");
        assert_eq!((found[0].line, found[0].col), (4, Some(9)));
        let found = parse(" ❯ tests/x.test.ts:21:11\n");
        assert_eq!(found[0].file, "tests/x.test.ts");
    }

    #[test]
    fn static_check_format() {
        let found = parse("error: src/a.ts:30: raw colour in a class\n");
        assert_eq!(
            found,
            vec![Location {
                file: "src/a.ts".into(),
                line: 30,
                col: None,
                message: "raw colour in a class".into()
            }]
        );
    }

    #[test]
    fn no_match() {
        assert!(parse("all fine\nerror: aborting due to 1 previous error\n").is_empty());
        // The file is not in the repository.
        assert!(parse("node_modules/x/y.js:1:2 boom\n").is_empty());
    }

    #[test]
    fn annotation_lines() {
        let location = Location {
            file: "src/a.ts".into(),
            line: 3,
            col: Some(1),
            message: "bad 100%\nthing".into(),
        };
        assert_eq!(
            annotations("biome", "r", &[location]),
            vec!["::error file=src/a.ts,line=3,col=1,title=biome::bad 100%25%0Athing"]
        );
        assert_eq!(
            annotations("platform:linux", "exited, 1", &[]),
            vec!["::error title=platform%3Alinux::exited, 1"]
        );
    }
}
