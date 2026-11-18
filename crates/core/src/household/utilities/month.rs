//! What the meters say: readings of an item by month and zone, the month a
//! meter is billed for and the bill of a metered month.

use std::collections::BTreeMap;

use crate::Amount;
use crate::calendar::{self, first_of_month};
use crate::model::VaultDoc;
use crate::model::planning::{MeterReading, UtilityItem, UtilityKind};
use crate::utilities_formula::{self, FormulaInput, FormulaIssue, ZoneValues};

/// The meter at the start and at the end of one month.
#[derive(Debug, Clone, PartialEq)]
pub struct MonthReadings {
    /// `None` for the first and only reading of a meter: nothing was used yet.
    pub start: Option<Amount>,
    pub end: Amount,
    pub end_id: String,
    /// When the start and the end were read (`sort_key`), for "days".
    pub start_at: Option<(String, u8)>,
    pub end_at: (String, u8),
    /// A multi-rate meter: the same per zone; the totals above sum them.
    pub zones: Vec<(String, MonthReadings)>,
}

impl MonthReadings {
    pub(super) fn consumption(&self) -> Option<Amount> {
        self.start.map(|start| self.end - start)
    }

    /// Days between the start and the end reading; a month when unknown.
    fn days(&self) -> Amount {
        let date = |(period, day): &(String, u8)| {
            let month = calendar::parse_month(period)?;
            let day = u32::from(*day).clamp(1, calendar::days_in_month(month));
            Some(first_of_month(month) + chrono::Duration::days(i64::from(day) - 1))
        };
        let days = self
            .start_at
            .as_ref()
            .and_then(date)
            .zip(date(&self.end_at))
            .map_or(30, |(start, end)| (end - start).num_days().max(0));
        Amount::new(rust_decimal::Decimal::from(days))
    }

    pub(super) fn formula_input(&self, tariff: Amount) -> FormulaInput {
        let values = |month: &MonthReadings| ZoneValues {
            current: month.end,
            previous: month.start.unwrap_or(month.end),
        };
        FormulaInput {
            total: values(self),
            zones: self
                .zones
                .iter()
                .map(|(zone, month)| (zone.clone(), values(month)))
                .collect(),
            tariff,
            days: self.days(),
        }
    }
}

/// The bill of a metered item for a month: consumption × tariff, or its
/// own formula. `None` while there is no consumption yet.
pub(super) fn metered_amount(
    item: &UtilityItem,
    month: &MonthReadings,
) -> std::result::Result<Option<Amount>, FormulaIssue> {
    let UtilityKind::Metered {
        tariff, formula, ..
    } = &item.kind
    else {
        return Ok(None);
    };
    let Some(used) = month.consumption() else {
        return Ok(None);
    };
    match formula
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
    {
        None => Ok(Some((used * *tariff).round(2))),
        Some(text) => utilities_formula::evaluate(text, &month.formula_input(*tariff)).map(Some),
    }
}

/// The zone names of an item: trimmed, without blanks.
pub(super) fn zones_of(item: &UtilityItem) -> Vec<String> {
    match &item.kind {
        UtilityKind::Metered { zones, .. } => zones
            .iter()
            .map(|zone| zone.trim().to_string())
            .filter(|zone| !zone.is_empty())
            .collect(),
        _ => Vec::new(),
    }
}

/// Ids of readings lower than the previous reading of the same zone, by date.
/// `readings` is oldest first (`readings_of`); a merge of two devices can
/// leave a meter going backwards, the values are never rewritten.
pub fn out_of_order_ids(readings: &[MeterReading]) -> Vec<String> {
    let mut last: BTreeMap<Option<&str>, Amount> = BTreeMap::new();
    let mut out = Vec::new();
    for reading in readings {
        let zone = reading.zone.as_deref();
        if last
            .get(&zone)
            .is_some_and(|before| reading.value < *before)
        {
            out.push(reading.id.clone());
        }
        last.insert(zone, reading.value);
    }
    out
}

/// Readings of an item, oldest first.
pub fn readings_of(stored: &[MeterReading], item_id: &str) -> Vec<MeterReading> {
    let mut out: Vec<MeterReading> = stored
        .iter()
        .filter(|reading| reading.item_id == item_id)
        .cloned()
        .collect();
    out.sort_by_key(MeterReading::sort_key);
    out
}

/// Readings of one zone of an item (`None` = a one-rate meter), oldest first.
pub fn readings_in(
    stored: &[MeterReading],
    item_id: &str,
    zone: Option<&str>,
) -> Vec<MeterReading> {
    let mut out = readings_of(stored, item_id);
    out.retain(|reading| reading.zone.as_deref() == zone);
    out
}

/// Where the meter stood at the start and at the end of each month with a
/// reading. A multi-rate meter counts a month once every zone is read in
/// it, the totals summing the zones.
pub fn monthly_readings(doc: &VaultDoc, item_id: &str) -> BTreeMap<String, MonthReadings> {
    let zones = super::items::utility_item(doc, item_id)
        .map(zones_of)
        .unwrap_or_default();
    if zones.is_empty() {
        return zone_months(doc, item_id, None);
    }
    let per_zone: Vec<(String, BTreeMap<String, MonthReadings>)> = zones
        .iter()
        .map(|zone| (zone.clone(), zone_months(doc, item_id, Some(zone))))
        .collect();
    let mut out = BTreeMap::new();
    let Some((_, first_zone)) = per_zone.first() else {
        return out;
    };
    for (period, lead) in first_zone {
        let parts: Option<Vec<(String, MonthReadings)>> = per_zone
            .iter()
            .map(|(zone, months)| {
                months
                    .get(period)
                    .map(|month| (zone.clone(), month.clone()))
            })
            .collect();
        let Some(parts) = parts else {
            continue;
        };
        let start = parts
            .iter()
            .map(|(_, month)| month.start)
            .try_fold(Amount::ZERO, |sum, start| start.map(|value| sum + value));
        out.insert(
            period.clone(),
            MonthReadings {
                start,
                end: parts
                    .iter()
                    .fold(Amount::ZERO, |sum, (_, month)| sum + month.end),
                end_id: lead.end_id.clone(),
                start_at: lead.start_at.clone(),
                end_at: lead.end_at.clone(),
                zones: parts,
            },
        );
    }
    out
}

/// One zone's months. A month starts from the last reading before it; the
/// very first month of a meter starts from its own first reading, so a
/// flat moved into on the 15th counts from the 15th.
pub fn zone_months(
    doc: &VaultDoc,
    item_id: &str,
    zone: Option<&str>,
) -> BTreeMap<String, MonthReadings> {
    let mut out: BTreeMap<String, MonthReadings> = BTreeMap::new();
    let mut previous: Option<&MeterReading> = None;
    let readings = readings_in(&doc.meter_readings, item_id, zone);
    for group in readings.chunk_by(|left, right| left.period == right.period) {
        let (Some(first), Some(last)) = (group.first(), group.last()) else {
            continue;
        };
        let base = previous.or((group.len() > 1).then_some(first));
        out.insert(
            last.period.clone(),
            MonthReadings {
                start: base.map(|reading| reading.value),
                end: last.value,
                end_id: last.id.clone(),
                start_at: base.map(MeterReading::sort_key),
                end_at: last.sort_key(),
                zones: Vec::new(),
            },
        );
        previous = Some(last);
    }
    out
}

/// The month a meter is billed for in `period`: its own readings, else the
/// latest month read before it that has a consumption and no payment from
/// then on — a meter read in September is paid in October, once.
pub fn billable_month(
    doc: &VaultDoc,
    item_id: &str,
    period: &str,
    paid: &BTreeMap<String, Amount>,
) -> Option<MonthReadings> {
    let mut monthly = monthly_readings(doc, item_id);
    if let Some(month) = monthly.remove(period) {
        return Some(month);
    }
    monthly
        .range(..period.to_string())
        .next_back()
        .filter(|(read, month)| {
            month.start.is_some() && paid.range((*read).clone()..).next().is_none()
        })
        .map(|(_, month)| month.clone())
}
