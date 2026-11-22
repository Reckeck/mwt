//! The bill a meter's formula (or tariff) gives for typed readings: the
//! form's preview and the bill dialog's re-pricing. Needs no vault.

use mwt_base::Amount;
use mwt_base::utilities_formula::{self, FormulaInput, FormulaIssue, ZoneValues};

/// One zone's readings for a formula preview.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FormulaZoneReading {
    /// `None` for a one-rate meter.
    #[serde(default)]
    pub zone: Option<String>,
    pub previous: Amount,
    pub current: Amount,
}

/// What a meter's bill would be: the form's preview and the bill dialog's
/// re-pricing as a reading is typed. Needs no vault.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct UtilityFormulaInput {
    pub tariff: Amount,
    /// Empty = consumption × tariff.
    #[serde(default)]
    pub formula: Option<String>,
    #[serde(default)]
    pub zones: Vec<String>,
    pub readings: Vec<FormulaZoneReading>,
    /// Days between the readings; a month when not given.
    #[serde(default)]
    pub days: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct UtilityFormulaPreview {
    pub amount: Option<Amount>,
    pub consumption: Option<Amount>,
    pub issue: Option<FormulaIssue>,
}

/// The bill by a meter's formula (or tariff) for the readings given.
pub fn preview_utility_formula(input: &UtilityFormulaInput) -> UtilityFormulaPreview {
    let zoned = |zone: Option<&str>| {
        input
            .readings
            .iter()
            .find(|reading| {
                reading.zone.as_deref().map(utilities_formula::zone_key)
                    == zone.map(utilities_formula::zone_key)
            })
            .map_or(ZoneValues::default(), |reading| ZoneValues {
                current: reading.current,
                previous: reading.previous,
            })
    };
    let zones: Vec<(String, ZoneValues)> = input
        .zones
        .iter()
        .map(|zone| (zone.clone(), zoned(Some(zone))))
        .collect();
    let total = if zones.is_empty() {
        zoned(None)
    } else {
        zones
            .iter()
            .fold(ZoneValues::default(), |sum, (_, values)| ZoneValues {
                current: sum.current + values.current,
                previous: sum.previous + values.previous,
            })
    };
    let consumption = Some(total.current - total.previous);
    let formula = input
        .formula
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty());
    let Some(formula) = formula else {
        return UtilityFormulaPreview {
            amount: consumption.map(|used| (used * input.tariff).round(2)),
            consumption,
            issue: None,
        };
    };
    let days = Amount::new(rust_decimal::Decimal::from(input.days.unwrap_or(30)));
    let formula_input = FormulaInput {
        total,
        zones,
        tariff: input.tariff,
        days,
    };
    match utilities_formula::evaluate(formula, &formula_input) {
        Ok(amount) => UtilityFormulaPreview {
            amount: Some(amount),
            consumption,
            issue: None,
        },
        Err(issue) => UtilityFormulaPreview {
            amount: None,
            consumption,
            issue: Some(issue),
        },
    }
}
