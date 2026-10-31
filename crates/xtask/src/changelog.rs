//! Text of CHANGELOG.md: the commit block of `## [Unreleased]` and its promotion to a release.
//! Pure functions on strings; `release.rs` does the reading, writing and git-cliff.

use anyhow::{Result, bail};

pub const START: &str = "<!-- commits:start -->";
pub const END: &str = "<!-- commits:end -->";
const HEADING: &str = "## [Unreleased]";
pub const EMPTY_LIST: &str = "_Коммитов с прошлого релиза нет._";

/// Three or more line breaks in a row become two.
fn collapse_blank_lines(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut run = 0;
    for ch in text.chars() {
        if ch == '\n' {
            run += 1;
            if run > 2 {
                continue;
            }
        } else {
            run = 0;
        }
        out.push(ch);
    }
    out
}

/// The `## [Unreleased]` section: `[start, end)` offsets into the changelog.
fn unreleased(text: &str) -> Result<(usize, usize)> {
    let Some(start) = text.find(HEADING) else {
        bail!("CHANGELOG.md has no «## [Unreleased]» section");
    };
    let end = match text[start + 1..].find("\n## [") {
        Some(offset) => start + 1 + offset + 1,
        None => text.len(),
    };
    Ok((start, end))
}

/// The changelog with the commit block of `[Unreleased]` replaced by `list`; text written by
/// hand above the markers stays.
pub fn with_commits(text: &str, list: &str) -> Result<String> {
    let (start, end) = unreleased(text)?;
    let mut section = text[start..end].trim_end().to_string();
    let block = format!("{START}\n\n{list}\n\n{END}");
    match (section.find(START), section.find(END)) {
        (Some(from), Some(to)) if from <= to => {
            section = format!("{}{block}{}", &section[..from], &section[to + END.len()..]);
        }
        (Some(_), Some(_)) => bail!("CHANGELOG.md: the end marker comes before the start marker"),
        _ => section = format!("{section}\n\n### Коммиты\n\n{block}"),
    }
    Ok(collapse_blank_lines(&format!(
        "{}{section}\n\n{}",
        &text[..start],
        &text[end..]
    )))
}

/// `[Unreleased]` (commit list already refreshed) becomes `## [next] — date`, a new empty
/// `[Unreleased]` opens above it.
pub fn promote(text: &str, next: &str, date: &str) -> Result<String> {
    if text.contains(&format!("## [{next}]")) {
        bail!("CHANGELOG.md already has a section for {next}");
    }
    let (start, end) = unreleased(text)?;
    let body = text[start..end]
        .replacen(HEADING, "", 1)
        .replacen(&format!("{START}\n\n"), "", 1)
        .replacen(&format!("\n\n{END}"), "", 1);
    let body = body.trim();
    let fresh = format!("{HEADING}\n\n### Коммиты\n\n{START}\n\n{EMPTY_LIST}\n\n{END}");
    Ok(collapse_blank_lines(&format!(
        "{}{fresh}\n\n## [{next}] — {date}\n\n{body}\n\n{}",
        &text[..start],
        &text[end..]
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str =
        "# Changelog\n\n## [Unreleased]\n\nBy hand.\n\n## [0.1.0] — 2026-10-13\n\nOld.\n";

    #[test]
    fn without_markers_the_block_is_appended_to_the_section() {
        let out = with_commits(BASE, "- a").unwrap();
        assert_eq!(
            out,
            format!(
                "# Changelog\n\n## [Unreleased]\n\nBy hand.\n\n### Коммиты\n\n{START}\n\n- a\n\n{END}\n\n## [0.1.0] — 2026-10-13\n\nOld.\n"
            )
        );
    }

    #[test]
    fn with_markers_only_the_list_changes() {
        let once = with_commits(BASE, "- a").unwrap();
        let twice = with_commits(&once, "- b").unwrap();
        assert_eq!(twice, once.replace("- a", "- b"));
        assert_eq!(with_commits(&twice, "- b").unwrap(), twice);
    }

    #[test]
    fn missing_unreleased_is_an_error() {
        assert!(with_commits("# Changelog\n", "- a").is_err());
        assert!(promote("# Changelog\n", "0.2.0", "2026-01-01").is_err());
    }

    #[test]
    fn markers_in_wrong_order_are_an_error() {
        let text = format!("## [Unreleased]\n\n{END}\n{START}\n");
        assert!(with_commits(&text, "- a").is_err());
    }

    #[test]
    fn promote_opens_a_fresh_unreleased() {
        let listed = with_commits(BASE, "- a").unwrap();
        let out = promote(&listed, "0.2.0", "2026-11-01").unwrap();
        let fresh =
            format!("## [Unreleased]\n\n### Коммиты\n\n{START}\n\n{EMPTY_LIST}\n\n{END}\n\n");
        assert!(out.contains(&fresh));
        assert!(
            out.contains("## [0.2.0] — 2026-11-01\n\nBy hand.\n\n### Коммиты\n\n- a\n\n## [0.1.0]")
        );
        assert!(!out.contains("commits:start -->\n\n- a"));
    }

    #[test]
    fn promote_refuses_an_existing_version() {
        assert!(promote(BASE, "0.1.0", "2026-11-01").is_err());
    }

    #[test]
    fn blank_lines_collapse() {
        assert_eq!(collapse_blank_lines("a\n\n\n\nb\n"), "a\n\nb\n");
    }
}
