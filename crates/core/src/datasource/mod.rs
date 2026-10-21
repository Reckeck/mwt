//! Pure extraction for the user's data sources (plan I2): what the address
//! returned goes in, the number, series or text the method points at comes
//! out. Fetching lives in `pricing`, applying in `service::datasources`.

pub mod file;
pub mod html;
pub mod json;
pub mod number;

use chrono::NaiveDate;
use scraper::Html;

use crate::Amount;
use crate::model::datasource::{NumberFormat, SeriesPoint, SourceMethod, SourceOutput, TableRow};

pub use number::parse_number;

/// What a method found, before it gets a timestamp.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, ts_rs::TS)]
pub struct Extracted {
    pub number: Option<Amount>,
    pub text: Option<String>,
    pub series: Vec<SeriesPoint>,
    /// Set when the value came from a fallback (the selector stopped
    /// matching and `text_hint` was used): the source works but needs fixing.
    pub warning: Option<String>,
}

/// What the address returned, as the picker shows it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum PreviewKind {
    Html,
    Json,
    Csv,
    Xlsx,
}

/// The page for the picker: a safe HTML copy, the JSON text or file rows.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, ts_rs::TS)]
pub struct SourcePreview {
    pub kind: PreviewKind,
    /// Sanitized HTML for the sandboxed frame (`kind = html`).
    pub html: Option<String>,
    /// Pretty-printed JSON (`kind = json`); the picker parses it itself.
    pub json: Option<String>,
    /// The first rows of a CSV/XLSX file (header first).
    pub rows: Vec<Vec<String>>,
}

/// Rows of a file shown in the picker.
const PREVIEW_ROWS: usize = 200;

/// Guess what `body` is: XLSX by its zip signature or extension, JSON by its
/// first character, CSV by the extension, otherwise HTML.
pub fn preview(url: &str, body: &[u8]) -> Result<SourcePreview, String> {
    let path = url.split(['?', '#']).next().unwrap_or("").to_lowercase();
    let first = body
        .iter()
        .find(|byte| !byte.is_ascii_whitespace())
        .copied();
    let empty = SourcePreview {
        kind: PreviewKind::Html,
        html: None,
        json: None,
        rows: Vec::new(),
    };
    if body.starts_with(b"PK") || path.ends_with(".xlsx") {
        let mut rows = file::rows(crate::model::datasource::FileFormat::Xlsx, "", body)?;
        rows.truncate(PREVIEW_ROWS);
        return Ok(SourcePreview {
            kind: PreviewKind::Xlsx,
            rows,
            ..empty
        });
    }
    if matches!(first, Some(b'{' | b'['))
        && let Ok(value) = serde_json::from_slice::<serde_json::Value>(body)
    {
        return Ok(SourcePreview {
            kind: PreviewKind::Json,
            json: serde_json::to_string_pretty(&value).ok(),
            ..empty
        });
    }
    if path.ends_with(".csv") {
        let mut rows = file::rows(crate::model::datasource::FileFormat::Csv, "", body)?;
        rows.truncate(PREVIEW_ROWS);
        return Ok(SourcePreview {
            kind: PreviewKind::Csv,
            rows,
            ..empty
        });
    }
    Ok(SourcePreview {
        html: Some(html::sanitize(&String::from_utf8_lossy(body), url)),
        ..empty
    })
}

/// The value `method` points at in `body`. An error is a reason to keep the
/// previous value and count a failure.
pub fn extract(
    method: &SourceMethod,
    output: SourceOutput,
    format: &NumberFormat,
    body: &[u8],
) -> Result<Extracted, String> {
    let extracted = match method {
        SourceMethod::Css {
            selector,
            text_hint,
        } => from_css(&page(body), selector, text_hint, output, format)?,
        SourceMethod::Table {
            table_selector,
            column_header,
            row,
        } => {
            let rows = html::table_rows(&page(body), table_selector)?;
            from_rows(&rows, column_header, *row, output, format)?
        }
        SourceMethod::Label { label } => from_label(&page(body), label, output, format)?,
        SourceMethod::Json { path } => json::extract(body, path, output, format)?,
        SourceMethod::File {
            format: file_format,
            sheet,
            column,
            row,
        } => {
            let rows = file::rows(*file_format, sheet, body)?;
            from_rows(&rows, column, *row, output, format)?
        }
    };
    check_output(&extracted, output)?;
    Ok(extracted)
}

fn page(body: &[u8]) -> Html {
    Html::parse_document(&String::from_utf8_lossy(body))
}

fn check_output(extracted: &Extracted, output: SourceOutput) -> Result<(), String> {
    let ok = match output {
        SourceOutput::Number => extracted.number.is_some(),
        SourceOutput::Series => !extracted.series.is_empty(),
        SourceOutput::Text => extracted
            .text
            .as_deref()
            .is_some_and(|text| !text.is_empty()),
    };
    if ok {
        Ok(())
    } else {
        Err(match output {
            SourceOutput::Number => "no number found".into(),
            SourceOutput::Series => "no series found".into(),
            SourceOutput::Text => "no text found".into(),
        })
    }
}

fn from_css(
    doc: &Html,
    selector: &str,
    text_hint: &str,
    output: SourceOutput,
    format: &NumberFormat,
) -> Result<Extracted, String> {
    if output == SourceOutput::Series {
        let parsed = scraper::Selector::parse(selector)
            .map_err(|err| format!("bad selector «{selector}»: {err}"))?;
        let series = doc
            .select(&parsed)
            .enumerate()
            .filter_map(|(index, element)| {
                let text = element.text().collect::<String>();
                Some(SeriesPoint {
                    label: (index + 1).to_string(),
                    value: parse_number(&text, format)?,
                })
            })
            .collect();
        return Ok(Extracted {
            series,
            ..Extracted::default()
        });
    }
    let found = html::select_text(doc, selector)?;
    let number = found.as_deref().and_then(|text| parse_number(text, format));
    let usable = match output {
        SourceOutput::Number => number.is_some(),
        _ => found.is_some(),
    };
    if usable {
        return Ok(Extracted {
            number,
            text: found,
            ..Extracted::default()
        });
    }
    // The page changed: look for the number after the text that stood next
    // to the element when it was picked.
    let after = html::text_after(&html::page_text(doc), text_hint);
    let number = after.as_deref().and_then(|text| parse_number(text, format));
    if after.is_none() || (number.is_none() && output == SourceOutput::Number) {
        return Err(format!("selector «{selector}» matches nothing"));
    }
    Ok(Extracted {
        number,
        text: after.map(|text| text.trim().to_string()),
        warning: Some(format!(
            "selector «{selector}» matches nothing, used the text near it"
        )),
        ..Extracted::default()
    })
}

fn from_label(
    doc: &Html,
    label: &str,
    output: SourceOutput,
    format: &NumberFormat,
) -> Result<Extracted, String> {
    if output == SourceOutput::Series {
        return Err("a label gives one value, not a series".into());
    }
    let after = html::text_after(&html::page_text(doc), label)
        .ok_or_else(|| format!("no «{label}» on the page"))?;
    Ok(Extracted {
        number: parse_number(&after, format),
        text: Some(after.trim().to_string()),
        ..Extracted::default()
    })
}

/// A column of `rows` (header row, then data rows): one cell or the series.
fn from_rows(
    rows: &[Vec<String>],
    column: &str,
    row: TableRow,
    output: SourceOutput,
    format: &NumberFormat,
) -> Result<Extracted, String> {
    let (header_at, index) =
        find_column(rows, column).ok_or_else(|| format!("no column «{column}»"))?;
    let body: Vec<Vec<String>> = rows
        .iter()
        .skip(header_at.map_or(0, |at| at + 1))
        .filter(|cells| cells.get(index).is_some_and(|cell| !cell.trim().is_empty()))
        .cloned()
        .collect();
    if output == SourceOutput::Series {
        let series = body
            .iter()
            .filter_map(|cells| {
                Some(SeriesPoint {
                    label: cells.first()?.trim().to_string(),
                    value: parse_number(cells.get(index)?, format)?,
                })
            })
            .collect();
        return Ok(Extracted {
            series,
            ..Extracted::default()
        });
    }
    let picked = html::pick_row(&body, row).ok_or("the table has no rows")?;
    let cell = picked.get(index).cloned().unwrap_or_default();
    Ok(Extracted {
        number: parse_number(&cell, format),
        text: Some(cell),
        ..Extracted::default()
    })
}

/// The header row and the column index: a header cell equal to (or else
/// containing) `column`, or a spreadsheet letter `A`…`ZZ` without a header.
fn find_column(rows: &[Vec<String>], column: &str) -> Option<(Option<usize>, usize)> {
    let wanted = column.trim().to_lowercase();
    if wanted.is_empty() {
        return None;
    }
    let lookup = |exact: bool| {
        rows.iter().enumerate().find_map(|(row_at, cells)| {
            cells
                .iter()
                .position(|cell| {
                    let cell = cell.trim().to_lowercase();
                    if exact {
                        cell == wanted
                    } else {
                        cell.contains(&wanted)
                    }
                })
                .map(|index| (Some(row_at), index))
        })
    };
    lookup(true)
        .or_else(|| column_letter(column.trim()).map(|index| (None, index)))
        .or_else(|| lookup(false))
}

fn column_letter(text: &str) -> Option<usize> {
    if text.is_empty() || text.len() > 2 || !text.chars().all(|ch| ch.is_ascii_uppercase()) {
        return None;
    }
    Some(
        text.bytes()
            .fold(0usize, |acc, byte| acc * 26 + usize::from(byte - b'A' + 1))
            - 1,
    )
}

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

#[cfg(test)]
mod tests {
    #[test]
    fn preview_tells_json_from_html() {
        let json = super::preview("https://api.example.com/x", b" {\"rate\": 6.35}").unwrap();
        assert_eq!(json.kind, super::PreviewKind::Json);
        assert!(json.json.unwrap().contains("6.35"));
        let page = super::preview("https://cbr.ru/", b"<p onclick=x()>6,35</p>").unwrap();
        assert_eq!(page.kind, super::PreviewKind::Html);
        assert!(!page.html.unwrap().contains("onclick"));
        let csv = super::preview("https://x.org/a.csv", b"a,b\n1,2\n").unwrap();
        assert_eq!(csv.rows.len(), 2);
    }

    use super::*;
    use crate::model::datasource::FileFormat;
    use std::str::FromStr;

    fn amount(text: &str) -> Amount {
        Amount::new(rust_decimal::Decimal::from_str(text).unwrap())
    }

    const PAGE: &str = r#"<html><body>
        <div class="rate"><span id="key">Ключевая ставка</span><b class="value">16,50 %</b></div>
        <table class="data"><tr><th>Дата</th><th>Ключевая ставка, %</th><th>Инфляция, % г/г</th></tr>
        <tr><td>09.2026</td><td>14,00</td><td>6,35</td></tr>
        <tr><td>08.2026</td><td>14,00</td><td>6,40</td></tr></table></body></html>"#;

    #[test]
    fn css_reads_the_picked_element() {
        let method = SourceMethod::Css {
            selector: "div.rate b.value".into(),
            text_hint: "Ключевая ставка".into(),
        };
        let got = extract(
            &method,
            SourceOutput::Number,
            &NumberFormat::default(),
            PAGE.as_bytes(),
        )
        .unwrap();
        assert_eq!(got.number, Some(amount("16.50")));
        assert_eq!(got.warning, None);
    }

    #[test]
    fn css_falls_back_to_the_text_hint() {
        let method = SourceMethod::Css {
            selector: "div.gone".into(),
            text_hint: "Ключевая ставка".into(),
        };
        let got = extract(
            &method,
            SourceOutput::Number,
            &NumberFormat::default(),
            PAGE.as_bytes(),
        )
        .unwrap();
        assert_eq!(got.number, Some(amount("16.50")));
        assert!(got.warning.is_some());
        let without_hint = SourceMethod::Css {
            selector: "div.gone".into(),
            text_hint: String::new(),
        };
        assert!(
            extract(
                &without_hint,
                SourceOutput::Number,
                &NumberFormat::default(),
                PAGE.as_bytes()
            )
            .is_err()
        );
    }

    #[test]
    fn table_by_header_and_newest_date() {
        let method = SourceMethod::Table {
            table_selector: "table.data".into(),
            column_header: "Инфляция".into(),
            row: TableRow::ByDate,
        };
        let format = NumberFormat::default();
        let got = extract(&method, SourceOutput::Number, &format, PAGE.as_bytes()).unwrap();
        assert_eq!(got.number, Some(amount("6.35")));
        let series = extract(&method, SourceOutput::Series, &format, PAGE.as_bytes()).unwrap();
        assert_eq!(series.series.len(), 2);
        assert_eq!(series.series[1].label, "08.2026");
        assert_eq!(series.series[1].value, amount("6.40"));
    }

    #[test]
    fn label_takes_the_number_after_it() {
        let method = SourceMethod::Label {
            label: "ключевая ставка".into(),
        };
        let got = extract(
            &method,
            SourceOutput::Number,
            &NumberFormat::default(),
            PAGE.as_bytes(),
        )
        .unwrap();
        assert_eq!(got.number, Some(amount("16.50")));
    }

    #[test]
    fn csv_column_by_header_and_letter() {
        let csv = "date;value\n2026-08;6,40\n2026-09;6,35\n";
        let by_header = SourceMethod::File {
            format: FileFormat::Csv,
            sheet: String::new(),
            column: "value".into(),
            row: TableRow::Last,
        };
        let format = NumberFormat::default();
        let got = extract(&by_header, SourceOutput::Number, &format, csv.as_bytes()).unwrap();
        assert_eq!(got.number, Some(amount("6.35")));
        let by_letter = SourceMethod::File {
            format: FileFormat::Csv,
            sheet: String::new(),
            column: "B".into(),
            row: TableRow::ByDate,
        };
        let got = extract(&by_letter, SourceOutput::Number, &format, csv.as_bytes()).unwrap();
        assert_eq!(got.number, Some(amount("6.35")));
    }

    #[test]
    fn wrong_output_is_an_error() {
        let method = SourceMethod::Label {
            label: "нет такого".into(),
        };
        assert!(
            extract(
                &method,
                SourceOutput::Number,
                &NumberFormat::default(),
                PAGE.as_bytes()
            )
            .is_err()
        );
    }

    #[test]
    fn dates_in_labels() {
        let day = |year, month, day| NaiveDate::from_ymd_opt(year, month, day);
        assert_eq!(parse_date_label("30.09.2026"), day(2026, 9, 30));
        assert_eq!(parse_date_label("09.2026"), day(2026, 9, 1));
        assert_eq!(parse_date_label("2026-09-30"), day(2026, 9, 30));
        assert_eq!(parse_date_label("2026-09"), day(2026, 9, 1));
        assert_eq!(parse_date_label("2026M09"), day(2026, 9, 1));
        assert_eq!(parse_date_label("Сентябрь 2026"), day(2026, 9, 1));
        assert_eq!(parse_date_label("Sep 2026"), day(2026, 9, 1));
        assert_eq!(parse_date_label("Май 2026"), day(2026, 5, 1));
        assert_eq!(parse_date_label("итого"), None);
        assert_eq!(column_letter("B"), Some(1));
        assert_eq!(column_letter("AA"), Some(26));
    }
}
