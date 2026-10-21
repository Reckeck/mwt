//! Inflation by month, as the statistics offices publish it.
//!
//! Each reading is year-over-year consumer price growth for one currency's
//! economy in one month. The app fetches them in the background (Bank of
//! Russia for RUB, BLS CPI-U for USD, Eurostat HICP for EUR); a currency
//! may instead keep the user's own flat rate.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::Amount;

/// Who published a reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum InflationSource {
    /// Bank of Russia, «Инфляция и ключевая ставка» table.
    Cbr,
    /// US Bureau of Labor Statistics, CPI-U all items (`CUUR0000SA0`).
    Bls,
    /// Eurostat HICP, euro area annual rate (`prc_hicp_minr`).
    Eurostat,
}

impl InflationSource {
    /// The built-in source of a currency (lowercase code), if there is one.
    pub fn for_currency(currency: &str) -> Option<Self> {
        match currency {
            "rub" => Some(Self::Cbr),
            "usd" => Some(Self::Bls),
            "eur" => Some(Self::Eurostat),
            _ => None,
        }
    }

    /// The currency this source measures.
    pub fn currency(self) -> &'static str {
        match self {
            Self::Cbr => "rub",
            Self::Bls => "usd",
            Self::Eurostat => "eur",
        }
    }

    pub const ALL: [Self; 3] = [Self::Cbr, Self::Bls, Self::Eurostat];
}

/// One month of one currency's inflation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct InflationReading {
    /// Lowercase currency code (`rub`).
    pub currency: String,
    /// `YYYY-MM`.
    pub month: String,
    /// Prices against the same month a year before, percent.
    pub yoy_pct: Amount,
    pub source: InflationSource,
}

/// Where a currency's inflation comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum InflationMode {
    /// The fetched monthly series (falls back to the manual rate before the
    /// first reading arrives).
    Auto,
    /// The user's flat rate from the app settings.
    Manual,
}
