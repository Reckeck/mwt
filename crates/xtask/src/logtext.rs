//! Task logs for display: progress-bar redraws collapsed, ANSI escapes removed, the cargo
//! progress bar turned into a short status.

/// Removes ANSI escape sequences (CSI, OSC and two-byte escapes).
pub fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\u{1b}' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('[') => {
                for next in chars.by_ref() {
                    if ('\u{40}'..='\u{7e}').contains(&next) {
                        break;
                    }
                }
            }
            Some(']') => {
                while let Some(next) = chars.next() {
                    if next == '\u{7}' {
                        break;
                    }
                    if next == '\u{1b}' {
                        chars.next();
                        break;
                    }
                }
            }
            // Charset selection (`ESC ( B`, sent by `tput sgr0`) has one more byte.
            Some('(' | ')' | '*' | '+' | '#' | '%') => {
                chars.next();
            }
            _ => {}
        }
    }
    out
}

/// Every line keeps only what follows its last `\r` (a progress bar redraws itself in place);
/// lines that were only erased disappear; escapes are stripped. The file on disk is not touched.
pub fn clean_log(text: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    for line in text.split('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line);
        let shown = line.rsplit('\r').next().unwrap_or("");
        if line.contains('\r') && shown.trim().is_empty() {
            continue;
        }
        lines.push(strip_ansi(shown));
    }
    lines.join("\n")
}

/// `   Building [=====>   ] 120/340: crate_a, crate_b...` as `cargo 120/340: crate_a, crate_b`;
/// the last crate in flight of the final steps is the link/LTO step.
pub fn cargo_progress(line: &str) -> Option<String> {
    let rest = line.trim().strip_prefix("Building")?.trim_start();
    let rest = rest.strip_prefix('[')?;
    let (_, rest) = rest.split_once(']')?;
    let (counts, names) = rest.trim_start().split_once(':').unwrap_or((rest, ""));
    let (done, total) = counts.trim().split_once('/')?;
    let done: u32 = done.trim().parse().ok()?;
    let total: u32 = total.trim().parse().ok()?;
    let names = names.trim().trim_end_matches("...");
    let crates: Vec<&str> = names
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .collect();
    Some(match crates.as_slice() {
        [] => format!("cargo {done}/{total}"),
        [only] if done + 1 == total || done == total => {
            format!("cargo {done}/{total}: linking/LTO {only}")
        }
        _ => format!("cargo {done}/{total}: {}", crates.join(", ")),
    })
}

/// A raw log line as shown in progress lines and the live block.
pub fn display_line(raw: &str) -> String {
    let line = strip_ansi(raw);
    let line = line.trim();
    cargo_progress(line).unwrap_or_else(|| line.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_escapes() {
        assert_eq!(strip_ansi("\u{1b}[31merror\u{1b}[0m: x"), "error: x");
        assert_eq!(strip_ansi("\u{1b}]0;title\u{7}ok"), "ok");
        assert_eq!(strip_ansi("a\u{1b}(B\u{1b}[mb"), "ab");
    }

    #[test]
    fn progress_redraws_collapse() {
        let raw = "a\nBuilding 1/3\rBuilding 2/3\rBuilding 3/3\nb\n   \rbar\r      \r\nc\n";
        assert_eq!(clean_log(raw), "a\nBuilding 3/3\nb\nc\n");
        assert_eq!(clean_log("x\r\ny"), "x\ny");
        assert_eq!(clean_log("\u{1b}[1mbold\u{1b}[0m"), "bold");
    }

    #[test]
    fn cargo_progress_lines() {
        assert_eq!(
            cargo_progress("   Building [=====>        ] 120/340: crate_a, crate_b..."),
            Some("cargo 120/340: crate_a, crate_b".into())
        );
        assert_eq!(
            cargo_progress("    Building [=========>] 339/340: trade-app"),
            Some("cargo 339/340: linking/LTO trade-app".into())
        );
        assert_eq!(
            cargo_progress("Building [===>] 340/340: trade-app"),
            Some("cargo 340/340: linking/LTO trade-app".into())
        );
        assert_eq!(
            cargo_progress("Building [=>] 10/340: only-one"),
            Some("cargo 10/340: only-one".into())
        );
        assert_eq!(cargo_progress("Compiling foo v1.0.0"), None);
        assert_eq!(cargo_progress("Building the thing"), None);
    }

    #[test]
    fn display_line_maps_cargo_only() {
        assert_eq!(display_line("  \u{1b}[1mhello\u{1b}[0m "), "hello");
        assert_eq!(display_line("Building [>] 1/2: a, b"), "cargo 1/2: a, b");
    }
}
