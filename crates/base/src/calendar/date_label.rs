//! Dates in the row labels of a custom data source.

use chrono::NaiveDate;

const MONTHS: &[(&str, u32)] = &[
    ("январ", 1),
    ("феврал", 2),
    ("март", 3),
    ("апрел", 4),
    ("ма", 5),
    ("июн", 6),
    ("июл", 7),
    ("август", 8),
    ("сентябр", 9),
    ("октябр", 10),
    ("ноябр", 11),
    ("декабр", 12),
    ("jan", 1),
    ("feb", 2),
    ("mar", 3),
    ("apr", 4),
    ("may", 5),
    ("jun", 6),
    ("jul", 7),
    ("aug", 8),
    ("sep", 9),
    ("oct", 10),
    ("nov", 11),
    ("dec", 12),
];

/// A date in a row label: `30.09.2026`, `09.2026`, `2026-09-30`, `2026-09`,
/// `2026M09`, `сентябрь 2026`, `Sep 2026`. A month without a day is its 1st.
pub fn parse_date_label(text: &str) -> Option<NaiveDate> {
    let text = text.trim().to_lowercase();
    let digits: Vec<u32> = text
        .split(|ch: char| !ch.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse().ok())
        .collect();
    let first_len = text
        .split(|ch: char| !ch.is_ascii_digit())
        .find(|part| !part.is_empty())
        .map_or(0, str::len);
    let date = |year: u32, month: u32, day: u32| {
        NaiveDate::from_ymd_opt(i32::try_from(year).ok()?, month, day)
    };
    match digits.as_slice() {
        [year, month, day, ..] if first_len == 4 => date(*year, *month, *day),
        [day, month, year, ..] if *year >= 1000 => date(*year, *month, *day),
        [year, month] if first_len == 4 => date(*year, *month, 1),
        [month, year] if *year >= 1000 && *month <= 12 => date(*year, *month, 1),
        [year] if *year >= 1000 => {
            let month = MONTHS
                .iter()
                .find(|(prefix, _)| text.contains(prefix))
                .map(|(_, month)| *month)?;
            date(*year, month, 1)
        }
        _ => None,
    }
}
