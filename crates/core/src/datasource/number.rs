//! Numbers as people write them: «6,35 %», «1 234,5», «−0.25», «$1,234.50».

use std::str::FromStr;

use crate::Amount;
use crate::model::datasource::NumberFormat;

/// A space that groups thousands: plain, no-break or thin.
fn is_group_space(ch: char) -> bool {
    matches!(ch, ' ' | '\u{a0}' | '\u{202f}' | '\u{2009}')
}

fn is_minus(ch: char) -> bool {
    matches!(ch, '-' | '−' | '–')
}

/// The first number in `text`, by `format`; `None` when there is none.
pub fn parse_number(text: &str, format: &NumberFormat) -> Option<Amount> {
    let cleaned = if format.strip.is_empty() {
        text.to_string()
    } else {
        text.replace(&format.strip, "")
    };
    let raw = first_number_token(&cleaned)?;
    let normalized = normalize(&raw, format.decimal_comma)?;
    let value = rust_decimal::Decimal::from_str(&normalized).ok()?;
    Some(Amount::new(value) * format.multiplier)
}

/// The digits, separators and sign of the first number in `text`.
fn first_number_token(text: &str) -> Option<String> {
    let chars: Vec<char> = text.chars().collect();
    let start = chars.iter().position(char::is_ascii_digit)?;
    let mut token = String::new();
    if start > 0 && is_minus(chars[start - 1]) {
        token.push('-');
    }
    let mut index = start;
    while index < chars.len() {
        let ch = chars[index];
        if ch.is_ascii_digit() || ch == ',' || ch == '.' {
            token.push(ch);
        } else if is_group_space(ch) && groups_three(&chars, index + 1) {
            // «1 234» is one number; «14.00 6.35» is two.
        } else {
            break;
        }
        index += 1;
    }
    let token = token.trim_end_matches([',', '.']).to_string();
    Some(token)
}

/// Exactly three digits start at `from` and no digit follows them.
fn groups_three(chars: &[char], from: usize) -> bool {
    let digits = chars
        .iter()
        .skip(from)
        .take_while(|ch| ch.is_ascii_digit())
        .count();
    digits == 3
}

fn normalize(raw: &str, decimal_comma: bool) -> Option<String> {
    let mut out = if decimal_comma && raw.contains(',') {
        raw.replace('.', "").replace(',', ".")
    } else if decimal_comma {
        // «6.35» on a Russian page: a single dot is still a fraction.
        if raw.matches('.').count() > 1 {
            raw.replace('.', "")
        } else {
            raw.to_string()
        }
    } else {
        raw.replace(',', "")
    };
    if out.matches('.').count() > 1 {
        return None;
    }
    if out.starts_with('.') {
        out.insert(0, '0');
    }
    (!out.is_empty() && out != "-").then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn russian() -> NumberFormat {
        NumberFormat::default()
    }

    fn english() -> NumberFormat {
        NumberFormat {
            decimal_comma: false,
            ..NumberFormat::default()
        }
    }

    fn amount(text: &str) -> Amount {
        Amount::new(rust_decimal::Decimal::from_str(text).unwrap())
    }

    #[test]
    fn reads_numbers_written_for_people() {
        assert_eq!(parse_number("6,35 %", &russian()), Some(amount("6.35")));
        assert_eq!(
            parse_number("1 234,5 ₽", &russian()),
            Some(amount("1234.5"))
        );
        assert_eq!(
            parse_number("1\u{a0}234\u{a0}567", &russian()),
            Some(amount("1234567"))
        );
        assert_eq!(
            parse_number("ставка 6.35", &russian()),
            Some(amount("6.35"))
        );
        assert_eq!(
            parse_number("−0,25 п.п.", &russian()),
            Some(amount("-0.25"))
        );
        assert_eq!(
            parse_number("$1,234.50", &english()),
            Some(amount("1234.50"))
        );
        assert_eq!(parse_number("нет данных", &russian()), None);
    }

    #[test]
    fn two_numbers_in_a_row_are_not_glued() {
        assert_eq!(
            parse_number("14.00 6.35", &english()),
            Some(amount("14.00"))
        );
        assert_eq!(parse_number("2025 6,35", &russian()), Some(amount("2025")));
    }

    #[test]
    fn strip_and_multiplier_apply() {
        let format = NumberFormat {
            decimal_comma: false,
            multiplier: amount("0.01"),
            strip: "bp".into(),
        };
        assert_eq!(parse_number("635bp", &format), Some(amount("6.35")));
    }
}
