//! The planned moments of a schedule: the parameters an "on schedule" trigger
//! reads, the dates they plan in a window, moved off days off.

use chrono::{Datelike, Duration, NaiveDate};
use serde_json::{Map, Value};

use mwt_base::calendar::workdays::{WorkdayRules, WorkdayShift};
use mwt_base::calendar::{Calendar, clamp_day};

use crate::json_value::{json_field, json_number, json_text};

/// How far back a schedule looks for missed moments.
const LOOKBACK_DAYS: i64 = 400;
/// How far a moved date may be from its planned one: the New Year holidays
/// and the weekends around them stay well inside.
const SHIFT_MARGIN_DAYS: i64 = 20;

fn hour_minute(raw: &str) -> (u32, u32) {
    let mut parts = raw.trim().splitn(2, ':');
    let hour = parts.next().and_then(|part| part.parse().ok()).unwrap_or(9);
    let minute = parts.next().and_then(|part| part.parse().ok()).unwrap_or(0);
    (hour.min(23), minute.min(59))
}

fn whole(value: Option<&Value>, default: i64) -> i64 {
    value
        .and_then(json_number)
        .and_then(|amount| amount.into_inner().trunc().to_string().parse().ok())
        .unwrap_or(default)
}

/// Every moment a schedule plans in `(after, until]`, oldest first, at most
/// `limit` of the newest, and how many older ones were left out.
///
/// With `workday` set to `back` or `forward` a planned date that falls on a
/// weekend or a public holiday moves to a working day. The dates are looked
/// for a little beyond `(after, until]` on both sides, so a date of next week
/// moved back to today is not missed, and only the moved moments inside the
/// window are kept.
pub fn schedule_moments_limited(
    params: &Map<String, Value>,
    cal: &Calendar,
    after: i64,
    until: i64,
    limit: usize,
) -> (Vec<i64>, usize) {
    let every = json_field(params, "every").map_or_else(|| "month".to_string(), json_text);
    let interval = whole(json_field(params, "interval"), 1).max(1);
    let day = whole(json_field(params, "day"), 1).clamp(1, 31) as u8;
    let weekday = whole(json_field(params, "weekday"), 4).clamp(0, 6);
    let (hour, minute) =
        hour_minute(&json_field(params, "time").map_or_else(|| "09:00".to_string(), json_text));

    let shift = WorkdayShift::from_param(
        &json_field(params, "workday").map_or_else(String::new, json_text),
    );
    let rules = WorkdayRules::ru_default();
    let margin = if shift == WorkdayShift::None {
        0
    } else {
        SHIFT_MARGIN_DAYS
    };
    let last = cal.date_of(until) + Duration::days(margin);
    let first = (cal.date_of(after) - Duration::days(margin))
        .max(cal.date_of(until) - Duration::days(LOOKBACK_DAYS));
    // «Every N days» counts from `anchor` when set (an automation's first
    // date), else from a fixed Monday.
    let epoch = json_field(params, "anchor")
        .map(json_text)
        .and_then(|raw| NaiveDate::parse_from_str(raw.get(..10).unwrap_or(&raw), "%Y-%m-%d").ok())
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(1970, 1, 5).unwrap_or_default());
    let planned = |date: NaiveDate| -> bool {
        match every.as_str() {
            "day" => (date - epoch).num_days().rem_euclid(interval) == 0,
            "week" => {
                i64::from(date.weekday().num_days_from_monday()) == weekday
                    && ((date - epoch).num_days().div_euclid(7)).rem_euclid(interval) == 0
            }
            _ => {
                let month_index = i64::from(date.year()) * 12 + i64::from(date.month0());
                date == clamp_day(date, day) && month_index.rem_euclid(interval) == 0
            }
        }
    };
    let mut out = Vec::new();
    let mut date = first;
    while date <= last {
        if planned(date) {
            let moment = cal.at_local_hm(shift.apply(date, &rules), hour, minute);
            if moment > after && moment <= until {
                out.push(moment);
            }
        }
        date += Duration::days(1);
    }
    // Two planned dates moved onto one working day fire once.
    out.sort_unstable();
    out.dedup();
    let skipped = out.len().saturating_sub(limit);
    out.drain(..skipped);
    (out, skipped)
}
