//! CSV and XLSX files as rows of text cells.

use std::io::Cursor;

use calamine::{Data, Reader};
use chrono::{Duration, NaiveDate};

use crate::model::datasource::FileFormat;

/// The rows of the file; `sheet` picks an XLSX sheet by name (empty: the first).
pub fn rows(format: FileFormat, sheet: &str, body: &[u8]) -> Result<Vec<Vec<String>>, String> {
    match format {
        FileFormat::Csv => csv_rows(body),
        FileFormat::Xlsx => xlsx_rows(sheet, body),
    }
}

fn csv_rows(body: &[u8]) -> Result<Vec<Vec<String>>, String> {
    let text = String::from_utf8_lossy(body);
    let text = text.trim_start_matches('\u{feff}');
    let first_line = text.lines().next().unwrap_or_default();
    // Russian exports separate by `;` because `,` is the decimal mark.
    let delimiter = b";\t,"
        .iter()
        .copied()
        .max_by_key(|delimiter| first_line.bytes().filter(|byte| byte == delimiter).count());
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .delimiter(delimiter.unwrap_or(b','))
        .from_reader(text.as_bytes());
    reader
        .records()
        .map(|record| {
            record
                .map(|cells| cells.iter().map(str::to_string).collect())
                .map_err(|err| format!("bad CSV: {err}"))
        })
        .collect()
}

fn xlsx_rows(sheet: &str, body: &[u8]) -> Result<Vec<Vec<String>>, String> {
    let mut book = calamine::open_workbook_auto_from_rs(Cursor::new(body.to_vec()))
        .map_err(|err| format!("bad XLSX: {err}"))?;
    let names = book.sheet_names();
    let name = if sheet.trim().is_empty() {
        names.first().cloned()
    } else {
        names
            .iter()
            .find(|name| name.trim().eq_ignore_ascii_case(sheet.trim()))
            .cloned()
    }
    .ok_or_else(|| format!("no sheet «{sheet}»"))?;
    let range = book
        .worksheet_range(&name)
        .map_err(|err| format!("bad XLSX sheet: {err}"))?;
    Ok(range
        .rows()
        .map(|cells| cells.iter().map(cell_text).collect())
        .collect())
}

/// A cell as text; a date cell becomes `YYYY-MM-DD` so «by date» works.
fn cell_text(cell: &Data) -> String {
    match cell {
        Data::DateTime(date_time) => serial_date(date_time.as_f64())
            .map(|date| date.format("%Y-%m-%d").to_string())
            .unwrap_or_default(),
        Data::Empty | Data::Error(_) => String::new(),
        other => other.to_string(),
    }
}

/// An Excel day number (1900 system) as a date.
fn serial_date(serial: f64) -> Option<NaiveDate> {
    if !serial.is_finite() || serial < 1.0 {
        return None;
    }
    let days = format!("{:.0}", serial.floor()).parse::<i64>().ok()?;
    NaiveDate::from_ymd_opt(1899, 12, 30)?.checked_add_signed(Duration::days(days))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_guesses_the_delimiter() {
        let semicolon = rows(FileFormat::Csv, "", "\u{feff}a;b\n1;2,5\n".as_bytes()).unwrap();
        assert_eq!(semicolon, vec![vec!["a", "b"], vec!["1", "2,5"]]);
        let comma = rows(FileFormat::Csv, "", b"a,b\n1,\"2,5\"\n").unwrap();
        assert_eq!(comma[1], vec!["1", "2,5"]);
    }

    #[test]
    fn excel_serial_dates() {
        assert_eq!(serial_date(46_295.0), NaiveDate::from_ymd_opt(2026, 9, 30));
        assert_eq!(serial_date(0.0), None);
    }

    #[test]
    fn xlsx_reads_a_named_sheet_with_every_cell_kind() {
        let rows = xlsx_rows(" budget ", include_bytes!("fixtures/sample.xlsx")).unwrap();
        assert_eq!(
            rows,
            [
                ["date", "name", "amount"],
                ["2023-03-15", "Кафе", "1234.5"],
                ["2023-03-16", "", "7"],
            ]
        );
    }

    #[test]
    fn broken_xlsx_is_an_error() {
        assert!(rows(FileFormat::Xlsx, "", b"not a zip").is_err());
    }
}
