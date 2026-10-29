//! Date nodes: parts of a date, making and shifting one, the distance between
//! two, the start or end of a period, working days and writing a date out.
//! Days are cut in the user's zone; a date leaves as an RFC3339 moment.

use chrono::{Datelike, Duration, Months, NaiveDate, NaiveDateTime, NaiveTime, Timelike};

use crate::Amount;
use crate::calendar::Calendar;
use crate::workflow::exec::Executor;
use crate::workflow::graph::Node;
use crate::workflow::types::WfValue;

use super::data::{WorkdayRules, parse_date};
use super::{Outputs, single};

const MONTHS_RU: [&str; 12] = [
    "января",
    "февраля",
    "марта",
    "апреля",
    "мая",
    "июня",
    "июля",
    "августа",
    "сентября",
    "октября",
    "ноября",
    "декабря",
];
const WEEKDAYS_RU: [&str; 7] = [
    "понедельник",
    "вторник",
    "среда",
    "четверг",
    "пятница",
    "суббота",
    "воскресенье",
];

fn number(value: i64) -> WfValue {
    WfValue::Number(Amount::new(value.into()))
}

/// The last day of a month.
pub(crate) fn days_in_month(year: i32, month: u32) -> u32 {
    let next = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
    };
    next.and_then(|first| first.pred_opt())
        .map_or(28, |last| last.day())
}

/// A day of a month, a day past its end being its last day.
pub(crate) fn clamped_date(year: i32, month: u32, day: u32) -> Option<NaiveDate> {
    let month = month.clamp(1, 12);
    NaiveDate::from_ymd_opt(year, month, day.clamp(1, days_in_month(year, month)))
}

/// `HH:MM` as a time of day; 00:00 when unreadable.
fn time_of(text: &str) -> NaiveTime {
    let mut parts = text.trim().splitn(2, ':');
    let hour = parts
        .next()
        .and_then(|part| part.trim().parse().ok())
        .unwrap_or(0u32)
        .min(23);
    let minute = parts
        .next()
        .and_then(|part| part.trim().parse().ok())
        .unwrap_or(0u32)
        .min(59);
    NaiveTime::from_hms_opt(hour, minute, 0).unwrap_or_default()
}

/// Write a date by a pattern of `ГГГГ ММ МММ ДД ЧЧ мм ДН` (or Latin
/// `YYYY MM MMM DD HH mm DW`).
pub(crate) fn format_date(local: NaiveDateTime, pattern: &str) -> String {
    let pattern = if pattern.trim().is_empty() {
        "ДД.ММ.ГГГГ"
    } else {
        pattern
    };
    let month_word = MONTHS_RU[local.month0() as usize];
    let weekday = WEEKDAYS_RU[local.weekday().num_days_from_monday() as usize];
    // Longer tokens first, so "МММ" is not read as "ММ" + "М".
    let tokens: [(&str, String); 14] = [
        ("ГГГГ", format!("{:04}", local.year())),
        ("YYYY", format!("{:04}", local.year())),
        ("МММ", month_word.to_string()),
        ("MMM", month_word.to_string()),
        ("ММ", format!("{:02}", local.month())),
        ("MM", format!("{:02}", local.month())),
        ("ДД", format!("{:02}", local.day())),
        ("DD", format!("{:02}", local.day())),
        ("ЧЧ", format!("{:02}", local.hour())),
        ("HH", format!("{:02}", local.hour())),
        ("мм", format!("{:02}", local.minute())),
        ("mm", format!("{:02}", local.minute())),
        ("ДН", weekday.to_string()),
        ("DW", weekday.to_string()),
    ];
    let mut out = String::new();
    let mut rest = pattern;
    'scan: while let Some(first) = rest.chars().next() {
        for (token, value) in &tokens {
            if let Some(after) = rest.strip_prefix(token) {
                out.push_str(value);
                rest = after;
                continue 'scan;
            }
        }
        out.push(first);
        rest = &rest[first.len_utf8()..];
    }
    out
}

impl Executor<'_> {
    /// A date input as a local date-time; now when nothing is wired.
    fn local_input(&mut self, node: &Node, name: &str) -> Result<NaiveDateTime, String> {
        let cal = Calendar::new(&self.ctx.zone);
        let value = self.input(node, name)?;
        match &value {
            WfValue::Null => Ok(cal.local(self.now())),
            WfValue::Text(text) if text.trim().is_empty() => Ok(cal.local(self.now())),
            WfValue::Date(text) | WfValue::Text(text) => {
                let text = text.trim();
                if let Some(secs) = crate::pricing::history::unix_secs(text) {
                    return Ok(cal.local(secs));
                }
                parse_date(text, &cal)
                    .map(|day| day.and_time(NaiveTime::default()))
                    .ok_or_else(|| format!("«{text}» is not a date"))
            }
            other => Err(format!("«{}» is not a date", other.as_text())),
        }
    }

    /// A local date-time as the date value that travels on.
    fn date_value(&self, local: NaiveDateTime) -> WfValue {
        let cal = Calendar::new(&self.ctx.zone);
        WfValue::Date(crate::calendar::iso(cal.at_local_hm(
            local.date(),
            local.hour(),
            local.minute(),
        )))
    }

    fn whole(&mut self, node: &Node, name: &str) -> Result<Option<i64>, String> {
        Ok(self
            .number_input(node, name)?
            .and_then(|value| value.into_inner().trunc().to_string().parse::<i64>().ok()))
    }

    fn workday_rules(&self, node: &Node) -> WorkdayRules {
        let cal = Calendar::new(&self.ctx.zone);
        let dates = |text: String| -> Vec<NaiveDate> {
            text.split([',', ';', ' ', '\n'])
                .filter_map(|part| parse_date(part, &cal))
                .collect()
        };
        WorkdayRules {
            holidays: self.text_param(node, "calendar") != "weekends",
            weekend: self.text_param(node, "weekend"),
            extra_off: dates(self.text_param(node, "extra_off")),
            extra_work: dates(self.text_param(node, "extra_work")),
        }
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn eval_date(&mut self, node: &Node) -> Result<Outputs, String> {
        match node.kind.as_str() {
            "date.parts" => {
                let local = self.local_input(node, "date")?;
                Ok(Outputs::from([
                    ("year", number(local.year().into())),
                    ("month", number(local.month().into())),
                    ("day", number(local.day().into())),
                    (
                        "weekday",
                        number(local.weekday().num_days_from_monday().into()),
                    ),
                    ("hour", number(local.hour().into())),
                    ("minute", number(local.minute().into())),
                    (
                        "days_in_month",
                        number(days_in_month(local.year(), local.month()).into()),
                    ),
                    (
                        "date_text",
                        WfValue::Text(local.date().format("%Y-%m-%d").to_string()),
                    ),
                ]))
            }
            "date.make" => {
                let base = self.local_input(node, "base")?;
                let year = self.whole(node, "year")?.unwrap_or(base.year().into());
                let month = self.whole(node, "month")?.unwrap_or(base.month().into());
                let day = self.whole(node, "day")?.unwrap_or(1);
                let shift = super::json_number(&self.raw_param(node, "month_shift"))
                    .and_then(|shift| shift.into_inner().trunc().to_string().parse::<i64>().ok())
                    .unwrap_or(0);
                if !(1..=12).contains(&month) {
                    return Err(format!("{month} is not a month (1–12)"));
                }
                // Month arithmetic from zero: shift 1 from December is January.
                let months = year * 12 + (month - 1) + shift;
                let (year, month) = (months.div_euclid(12), months.rem_euclid(12) + 1);
                let date = clamped_date(
                    i32::try_from(year).map_err(|_| "the year is out of range")?,
                    month as u32,
                    day.clamp(1, 31) as u32,
                )
                .ok_or("no such date")?;
                let time = time_of(&self.text_param(node, "time"));
                Ok(single("date", self.date_value(date.and_time(time))))
            }
            "date.add" => {
                let local = self.local_input(node, "date")?;
                let shifted = if let WfValue::Duration(secs) = self.input(node, "duration")? {
                    local + Duration::seconds(secs)
                } else {
                    let amount = self.whole(node, "amount")?.unwrap_or(0);
                    let unit = self.text_param(node, "unit");
                    match unit.as_str() {
                        "minutes" => local + Duration::minutes(amount),
                        "hours" => local + Duration::hours(amount),
                        "weeks" => local + Duration::weeks(amount),
                        "months" | "years" => {
                            let months = if unit == "years" { amount * 12 } else { amount };
                            let count = Months::new(months.unsigned_abs() as u32);
                            if months >= 0 {
                                local.checked_add_months(count)
                            } else {
                                local.checked_sub_months(count)
                            }
                            .ok_or("the date is out of range")?
                        }
                        _ => local + Duration::days(amount),
                    }
                };
                Ok(single("date", self.date_value(shifted)))
            }
            "date.diff" => {
                let from = self.local_input(node, "a")?;
                let to = self.local_input(node, "b")?;
                let secs = (to - from).num_seconds();
                Ok(Outputs::from([
                    ("days", number((to.date() - from.date()).num_days())),
                    ("hours", number(secs / 3600)),
                    ("duration", WfValue::Duration(secs)),
                ]))
            }
            "date.boundary" => {
                let local = self.local_input(node, "date")?;
                let day = local.date();
                let date = match self.text_param(node, "which").as_str() {
                    "day_start" => day,
                    "week_start" => {
                        day - Duration::days(day.weekday().num_days_from_monday().into())
                    }
                    "month_end" => clamped_date(day.year(), day.month(), 31).unwrap_or(day),
                    "next_month_start" => clamped_date(day.year(), day.month(), 1)
                        .and_then(|first| first.checked_add_months(Months::new(1)))
                        .unwrap_or(day),
                    "year_start" => NaiveDate::from_ymd_opt(day.year(), 1, 1).unwrap_or(day),
                    "year_end" => NaiveDate::from_ymd_opt(day.year(), 12, 31).unwrap_or(day),
                    _ => clamped_date(day.year(), day.month(), 1).unwrap_or(day),
                };
                Ok(single(
                    "date",
                    self.date_value(date.and_time(NaiveTime::default())),
                ))
            }
            "date.shift_workday" => {
                let local = self.local_input(node, "date")?;
                let rules = self.workday_rules(node);
                let back = self.text_param(node, "direction") != "forward";
                let day = local.date();
                let moved = if back {
                    rules.previous_workday(day)
                } else {
                    rules.next_workday(day)
                };
                Ok(Outputs::from([
                    ("date", self.date_value(moved.and_time(local.time()))),
                    ("shifted", WfValue::Bool(moved != day)),
                ]))
            }
            "date.format" => {
                let local = self.local_input(node, "date")?;
                let pattern = self.input(node, "pattern")?.as_text();
                Ok(single("text", WfValue::Text(format_date(local, &pattern))))
            }
            other => Err(format!("unknown date node {other}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(raw: &str) -> NaiveDate {
        NaiveDate::parse_from_str(raw, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn a_day_past_the_month_end_is_its_last_day() {
        assert_eq!(clamped_date(2026, 2, 31), Some(day("2026-02-28")));
        assert_eq!(clamped_date(2028, 2, 30), Some(day("2028-02-29")));
        assert_eq!(days_in_month(2026, 12), 31);
    }

    #[test]
    fn a_date_is_written_by_its_pattern() {
        let moment = day("2026-11-25").and_hms_opt(14, 5, 0).unwrap();
        assert_eq!(format_date(moment, "ДД.ММ.ГГГГ ЧЧ:мм"), "25.11.2026 14:05");
        assert_eq!(format_date(moment, "ДД МММ, ДН"), "25 ноября, среда");
        assert_eq!(format_date(moment, "YYYY-MM-DD"), "2026-11-25");
        assert_eq!(format_date(moment, ""), "25.11.2026");
    }
}
