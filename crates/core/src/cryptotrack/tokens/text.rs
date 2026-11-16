//! Token names and symbols come from the network, so they are untrusted text:
//! anyone can deploy a contract named anything. Everything stored or shown
//! goes through [`sanitize`] first.
//!
//! What it does, in order:
//! 1. **Control and invisible characters** (C0/C1 controls, bidi overrides,
//!    zero-width and joiner characters, the BOM, line and paragraph
//!    separators) are dropped, so a name cannot reorder or hide text.
//! 2. **Links** (`scheme://…`, `www.…`, `domain.tld/…`, `t.me/…`) are replaced
//!    by [`LINK_PLACEHOLDER`]: the text that reaches the UI contains no URL
//!    at all, so nothing can be rendered as a link.
//! 3. **Whitespace** is collapsed to single spaces.
//! 4. **Length** is capped in characters, with an ellipsis.
//!
//! Look-alike substitution (Cyrillic `С` in `USDС`) cannot be fixed by
//! rewriting the text without changing what a legitimate name means, so it is
//! *detected*: [`skeleton`] folds the common confusables to ASCII and
//! [`has_look_alikes`] flags mixed-script text. A token whose skeleton equals
//! a known symbol while its contract is unknown is shown with a warning (it
//! is in quarantine anyway: identity never depends on the symbol).

/// Longest name kept, in characters.
pub const MAX_NAME_CHARS: usize = 64;
/// Longest symbol kept, in characters.
pub const MAX_SYMBOL_CHARS: usize = 16;
/// What a removed link becomes.
pub const LINK_PLACEHOLDER: &str = "[link removed]";

fn is_invisible(ch: char) -> bool {
    ch.is_control()
        || matches!(
            ch,
            '\u{00AD}'
                | '\u{061C}'
                | '\u{180E}'
                | '\u{200B}'..='\u{200F}'
                | '\u{2028}'..='\u{202E}'
                | '\u{2060}'..='\u{206F}'
                | '\u{FEFF}'
                | '\u{FFF9}'..='\u{FFFB}'
                | '\u{E0000}'..='\u{E007F}'
        )
}

/// A whitespace-separated word that would render as a link.
fn looks_like_link(word: &str) -> bool {
    let lower = word.to_ascii_lowercase();
    if lower.contains("://") || lower.starts_with("www.") || lower.contains("t.me/") {
        return true;
    }
    // `label.tld` with an alphabetic tld of 2+ letters, optionally followed by a path.
    let host = lower.split(['/', '?', '#']).next().unwrap_or("");
    let mut labels = host.split('.');
    let first = labels.next().unwrap_or("");
    let rest: Vec<&str> = labels.collect();
    let label_ok = |label: &str| {
        !label.is_empty()
            && label
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
    };
    !rest.is_empty()
        && label_ok(first)
        && rest.iter().all(|label| label_ok(label))
        && rest
            .last()
            .is_some_and(|tld| tld.len() >= 2 && tld.chars().all(|ch| ch.is_ascii_alphabetic()))
}

fn cap(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max_chars.saturating_sub(1)).collect();
    out.push('\u{2026}');
    out
}

/// Make a network-supplied name or symbol safe to store and show.
pub fn sanitize(raw: &str, max_chars: usize) -> String {
    let visible: String = raw
        .chars()
        .map(|ch| if ch.is_whitespace() { ' ' } else { ch })
        .filter(|ch| !is_invisible(*ch))
        .collect();
    let words: Vec<&str> = visible
        .split_whitespace()
        .map(|word| {
            if looks_like_link(word) {
                LINK_PLACEHOLDER
            } else {
                word
            }
        })
        .collect();
    cap(&words.join(" "), max_chars)
}

pub fn sanitize_name(raw: &str) -> String {
    sanitize(raw, MAX_NAME_CHARS)
}

pub fn sanitize_symbol(raw: &str) -> String {
    sanitize(raw, MAX_SYMBOL_CHARS)
}

/// Fold the common Latin look-alikes (Cyrillic, Greek, fullwidth) to ASCII
/// and uppercase, to compare with a known symbol.
pub fn skeleton(text: &str) -> String {
    text.chars()
        .map(|ch| match ch {
            '\u{FF01}'..='\u{FF5E}' => char::from_u32(ch as u32 - 0xFEE0).unwrap_or(ch),
            'А' | 'а' | 'Α' | 'α' => 'A',
            'В' | 'Β' => 'B',
            'С' | 'с' | 'Ϲ' | 'ϲ' => 'C',
            'Е' | 'е' | 'Ε' => 'E',
            'Н' | 'Η' => 'H',
            'І' | 'і' | 'Ι' | 'ι' => 'I',
            'Ј' | 'ј' => 'J',
            'К' | 'Κ' | 'к' => 'K',
            'М' | 'Μ' => 'M',
            'О' | 'о' | 'Ο' | 'ο' => 'O',
            'Р' | 'р' | 'Ρ' | 'ρ' => 'P',
            'Ѕ' | 'ѕ' => 'S',
            'Т' | 'Τ' => 'T',
            'Х' | 'х' | 'Χ' => 'X',
            'У' | 'у' | 'Υ' => 'Y',
            'Ζ' => 'Z',
            other => other.to_ascii_uppercase(),
        })
        .collect()
}

/// Letters of a script that has Latin look-alikes next to ASCII letters, or
/// fullwidth forms: the text is not what it seems.
pub fn has_look_alikes(text: &str) -> bool {
    let ascii_letters = text.chars().any(|ch| ch.is_ascii_alphabetic());
    let foreign = text.chars().any(|ch| {
        matches!(ch, '\u{0370}'..='\u{03FF}' | '\u{0400}'..='\u{04FF}' | '\u{0530}'..='\u{058F}')
    });
    let fullwidth = text.chars().any(|ch| matches!(ch, '\u{FF01}'..='\u{FF5E}'));
    fullwidth || (ascii_letters && foreign)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_and_bidi_characters_are_dropped() {
        assert_eq!(sanitize_name("Us\u{0000}d\u{202E}c\u{200B}\n"), "Usdc");
    }

    #[test]
    fn a_long_name_is_capped_with_an_ellipsis() {
        let long = "A".repeat(500);
        let clean = sanitize_name(&long);
        assert_eq!(clean.chars().count(), MAX_NAME_CHARS);
        assert!(clean.ends_with('\u{2026}'));
    }

    #[test]
    fn links_never_survive() {
        for raw in [
            "Claim at https://evil.example/x",
            "visit www.evil.io now",
            "airdrop evil.com/claim",
            "join t.me/scam",
        ] {
            let clean = sanitize_name(raw);
            assert!(clean.contains(LINK_PLACEHOLDER), "{raw}");
            assert!(!clean.contains("evil") && !clean.contains("t.me"), "{raw}");
        }
        assert_eq!(sanitize_name("Wrapped Ether"), "Wrapped Ether");
        assert_eq!(sanitize_name("v2.0 token"), "v2.0 token");
    }

    #[test]
    fn a_cyrillic_c_in_usdc_is_detected_and_folded() {
        let fake = "USD\u{0421}";
        assert!(has_look_alikes(fake));
        assert_eq!(skeleton(fake), "USDC");
        assert!(!has_look_alikes("USDC"));
    }
}
