//! The user's own data sources (plan I2): a number, a series or a text taken
//! from a web page, a JSON answer or a CSV/XLSX file, refreshed in the
//! background and used for inflation, an asset's price or a workflow.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::Amount;

/// Values kept per source for its chart and for monthly series.
pub const MAX_SOURCE_HISTORY: usize = 400;
/// Failed refreshes in a row after which a source is marked broken.
pub const BROKEN_AFTER: u8 = 3;
/// A new number this many times larger or smaller than the last one waits
/// for the user's confirmation.
pub const JUMP_FACTOR: u32 = 10;

/// How to find the value in what the address returns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SourceMethod {
    /// One element of an HTML page by CSS selector. `text_hint` is the text
    /// near it when it was picked: if the selector stops matching, the
    /// number after that text is taken instead.
    Css {
        selector: String,
        #[serde(default)]
        text_hint: String,
    },
    /// A cell of an HTML table: the column by its header text, the row by
    /// position or by the newest date in the first column.
    Table {
        table_selector: String,
        column_header: String,
        row: TableRow,
    },
    /// A value of a JSON answer by a dotted path (`data.items.0.rate`); an
    /// array of numbers or of `[label, number]` pairs is a series.
    Json { path: String },
    /// The first number after a label anywhere in the page text.
    Label { label: String },
    /// A CSV or XLSX file: the column by its header (or a letter `A`…), the
    /// row as in a table.
    File {
        format: FileFormat,
        #[serde(default)]
        sheet: String,
        column: String,
        row: TableRow,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TableRow {
    First,
    Last,
    /// The row whose first cell is the latest date.
    ByDate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FileFormat {
    Csv,
    Xlsx,
}

/// What the source gives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SourceOutput {
    Number,
    /// A whole column or array: `(label, number)` rows.
    Series,
    Text,
}

/// How to read a number written for people: «6,35 %», «1 234,5».
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct NumberFormat {
    /// `,` separates the fraction (Russian style); otherwise it groups
    /// thousands.
    #[serde(default)]
    pub decimal_comma: bool,
    /// Applied after parsing: 0.01 turns «635 bp» into 6.35.
    #[serde(default = "one")]
    pub multiplier: Amount,
    /// Text removed before parsing (a currency sign, «руб.»).
    #[serde(default)]
    pub strip: String,
}

fn one() -> Amount {
    Amount::from(1)
}

impl Default for NumberFormat {
    fn default() -> Self {
        Self {
            decimal_comma: true,
            multiplier: one(),
            strip: String::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SourceInterval {
    Hour,
    Day,
    Week,
}

impl SourceInterval {
    pub fn seconds(self) -> i64 {
        match self {
            Self::Hour => 3600,
            Self::Day => 86_400,
            Self::Week => 7 * 86_400,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SourceStatus {
    /// Not fetched yet, or the last refresh worked.
    Ok,
    /// [`BROKEN_AFTER`] refreshes in a row failed.
    Broken,
    /// The new value jumped more than [`JUMP_FACTOR`] times and waits for
    /// confirmation.
    Pending,
}

/// One row of a series value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct SeriesPoint {
    pub label: String,
    pub value: Amount,
}

/// One fetched value.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct SourceValue {
    /// RFC3339: when it was fetched.
    pub at: String,
    #[serde(default)]
    pub number: Option<Amount>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub series: Vec<SeriesPoint>,
}

/// A source the user set up.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct DataSource {
    pub id: String,
    pub name: String,
    /// `https://…` address of the page, JSON answer or file.
    pub url: String,
    pub method: SourceMethod,
    pub output: SourceOutput,
    #[serde(default)]
    pub number: NumberFormat,
    pub interval: SourceInterval,
    #[serde(default)]
    pub last: Option<SourceValue>,
    /// Earlier values, oldest first, at most [`MAX_SOURCE_HISTORY`].
    #[serde(default)]
    pub history: Vec<SourceValue>,
    /// The value waiting for confirmation while `status` is `Pending`.
    #[serde(default)]
    pub pending: Option<SourceValue>,
    #[serde(default)]
    pub failures: u8,
    #[serde(default = "status_ok")]
    pub status: SourceStatus,
    /// The last error, for «Починить».
    #[serde(default)]
    pub error: Option<String>,
    /// RFC3339 of the last attempt, successful or not.
    #[serde(default)]
    pub checked_at: Option<String>,
    pub created_at: String,
}

fn status_ok() -> SourceStatus {
    SourceStatus::Ok
}

/// An asset priced by a source (plan I2): the source's number is the price
/// of one unit in `currency`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct AssetPriceSource {
    /// `AssetRef::key()`.
    pub asset_key: String,
    pub source_id: String,
    /// Lowercase code of the currency the number is in.
    pub currency: String,
}

/// A deposit whose rate a source follows: a change is announced, not applied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct DepositRateSource {
    pub store_id: String,
    pub source_id: String,
}
