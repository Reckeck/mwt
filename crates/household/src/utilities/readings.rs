//! Meter readings: the rules a new or corrected reading must pass. The
//! writes are the scenarios in `usecases::household::utilities`.

use mwt_base::Amount;
use mwt_base::calendar;
use mwt_base::error::{Error, Result};
use mwt_base::utilities_formula;
use mwt_model::VaultDoc;
use mwt_model::{MeterReading, UtilityItem, UtilityKind};

use super::month::{readings_in, zones_of};

/// A reading as typed: the meter, the value, the month, the day and the zone.
#[derive(Debug, Clone, Copy)]
pub struct ReadingInput<'input> {
    pub item_id: &'input str,
    pub value: Amount,
    pub period: &'input str,
    pub day: Option<u8>,
    pub zone: Option<&'input str>,
}

/// Names a reading by its meter, zone and day. The rule belongs to the sync
/// layer, so the scenarios hand it in.
pub type ReadingNamer<'namer> = &'namer dyn Fn(&str, Option<&str>, &(String, u8)) -> String;

/// A reading moved by a correction: the record it replaces and the new one.
#[derive(Debug, Clone, PartialEq)]
pub struct ReadingMove {
    pub old_id: String,
    pub reading: MeterReading,
}

/// Validate a reading against `stored` and build the record. A reading made
/// on the day of a stored one has its id and replaces it. `now` (ISO) is its
/// `recorded_at`. Writes nothing.
pub(super) fn plan_reading(
    items: &[UtilityItem],
    stored: &[MeterReading],
    input: ReadingInput<'_>,
    name: ReadingNamer<'_>,
    now: &str,
) -> Result<MeterReading> {
    let ReadingInput {
        item_id,
        value,
        period,
        day,
        zone,
    } = input;
    let item = items
        .iter()
        .find(|item| item.id == item_id)
        .ok_or_else(|| Error::Missing(format!("utility {item_id}")))?;
    if !matches!(item.kind, UtilityKind::Metered { .. }) {
        return Err(Error::InvalidInput(
            "only a metered bill takes readings".into(),
        ));
    }
    let zones = zones_of(item);
    // A multi-rate meter takes a reading per zone, stored under the zone's own name.
    let zone = match zone.map(utilities_formula::zone_key) {
        None if zones.is_empty() => None,
        None => {
            return Err(Error::InvalidInput(format!(
                "pick the zone: {}",
                zones.join(", ")
            )));
        }
        Some(key) => Some(
            zones
                .iter()
                .find(|zone| utilities_formula::zone_key(zone) == key)
                .cloned()
                .ok_or_else(|| Error::InvalidInput(format!("the meter has no zone «{key}»")))?,
        ),
    };
    let month = calendar::parse_month(period)
        .ok_or_else(|| Error::InvalidInput(format!("month {period}: expected YYYY-MM")))?;
    let period = calendar::month_key(month);
    if let Some(day) = day
        && (day == 0 || u32::from(day) > calendar::days_in_month(month))
    {
        return Err(Error::InvalidInput(format!("day {day} is not in {period}")));
    }
    if value.is_negative() {
        return Err(Error::InvalidInput("a reading cannot be negative".into()));
    }
    // No day given means the month's end; it is stored as that day so the
    // reading keeps its place whenever it was entered.
    let day = day.or_else(|| u8::try_from(calendar::days_in_month(month)).ok());
    let mut reading = MeterReading {
        id: String::new(),
        item_id: item_id.to_string(),
        value,
        period,
        recorded_at: now.to_string(),
        day,
        zone: zone.clone(),
    };
    // Named by meter, zone and day: the same reading made on two devices
    // is one record.
    reading.id = name(item_id, zone.as_deref(), &reading.sort_key());
    let key = reading.sort_key();
    let readings = readings_in(stored, item_id, zone.as_deref());
    if let Some(before) = readings
        .iter()
        .rev()
        .find(|reading| reading.sort_key() < key)
        && value < before.value
    {
        return Err(Error::InvalidInput(format!(
            "the reading is below an earlier one ({})",
            before.value
        )));
    }
    if let Some(after) = readings.iter().find(|reading| reading.sort_key() > key)
        && value > after.value
    {
        return Err(Error::InvalidInput(format!(
            "the reading is above a later one ({})",
            after.value
        )));
    }
    Ok(reading)
}

/// A new reading against the readings in the document.
pub fn plan_add_reading(
    doc: &VaultDoc,
    input: ReadingInput<'_>,
    name: ReadingNamer<'_>,
    now: &str,
) -> Result<MeterReading> {
    plan_reading(&doc.utility_items, &doc.meter_readings, input, name, now)
}

/// A correction: its value and, if moved, its month and day. The same rules
/// as a new one apply against every other reading of the meter.
pub fn plan_update_reading(
    doc: &VaultDoc,
    id: &str,
    value: Amount,
    period: &str,
    day: Option<u8>,
    name: ReadingNamer<'_>,
    now: &str,
) -> Result<ReadingMove> {
    let old = doc
        .meter_readings
        .iter()
        .find(|reading| reading.id == id)
        .ok_or_else(|| Error::Missing(format!("reading {id}")))?;
    let others: Vec<MeterReading> = doc
        .meter_readings
        .iter()
        .filter(|reading| reading.id != id)
        .cloned()
        .collect();
    let input = ReadingInput {
        item_id: &old.item_id,
        value,
        period,
        day,
        zone: old.zone.as_deref(),
    };
    Ok(ReadingMove {
        old_id: id.to_string(),
        reading: plan_reading(&doc.utility_items, &others, input, name, now)?,
    })
}

/// A reading to delete must exist.
pub fn require_reading(doc: &VaultDoc, id: &str) -> Result<()> {
    if doc.meter_readings.iter().any(|reading| reading.id == id) {
        Ok(())
    } else {
        Err(Error::Missing(format!("reading {id}")))
    }
}
