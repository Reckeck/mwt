//! Working days: weekends, public holidays of Russia and the user's own
//! dates. Used by the workflow date nodes and by schedules that move a
//! payment off a day off.

use chrono::{Datelike, NaiveDate, Weekday};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::Calendar;

/// A date written as `2026-03-08` or as an RFC3339 moment (its local day).
pub fn parse_date(text: &str, cal: &Calendar) -> Option<NaiveDate> {
    let text = text.trim();
    crate::pricing::history::unix_secs(text)
        .map(|secs| cal.date_of(secs))
        .or_else(|| NaiveDate::parse_from_str(text.get(..10).unwrap_or(text), "%Y-%m-%d").ok())
}

pub fn parse_dates(text: &str, cal: &Calendar) -> Vec<NaiveDate> {
    text.split([',', ';', ' ', '\n'])
        .filter(|part| !part.trim().is_empty())
        .filter_map(|part| parse_date(part, cal))
        .collect()
}

/// Public holidays of Russia by `(month, day)` (Labour Code art. 112).
const RU_HOLIDAYS: [(u32, u32); 14] = [
    (1, 1),
    (1, 2),
    (1, 3),
    (1, 4),
    (1, 5),
    (1, 6),
    (1, 7),
    (1, 8),
    (2, 23),
    (3, 8),
    (5, 1),
    (5, 9),
    (6, 12),
    (11, 4),
];

/// How many days ahead the next working day is looked for.
const WORKDAY_SEARCH_DAYS: i64 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DayKind {
    pub workday: bool,
    pub weekend: bool,
    pub holiday: bool,
}

/// What makes a day a day off: weekdays, holidays and the user's own dates.
pub struct WorkdayRules {
    pub holidays: bool,
    /// `sat_sun`, `sun` or `fri_sat`.
    pub weekend: String,
    pub extra_off: Vec<NaiveDate>,
    pub extra_work: Vec<NaiveDate>,
}

impl WorkdayRules {
    pub fn classify(&self, date: NaiveDate) -> DayKind {
        let weekday = date.weekday();
        let weekend = match self.weekend.as_str() {
            "sun" => weekday == Weekday::Sun,
            "fri_sat" => matches!(weekday, Weekday::Fri | Weekday::Sat),
            _ => matches!(weekday, Weekday::Sat | Weekday::Sun),
        };
        let holiday = (self.holidays && RU_HOLIDAYS.contains(&(date.month(), date.day())))
            || self.extra_off.contains(&date);
        let workday = self.extra_work.contains(&date) || !(weekend || holiday);
        DayKind {
            workday,
            weekend,
            holiday,
        }
    }

    /// The day itself if it is a working one, else the last working day before it.
    pub fn previous_workday(&self, date: NaiveDate) -> NaiveDate {
        (0..WORKDAY_SEARCH_DAYS)
            .map(|offset| date - chrono::Duration::days(offset))
            .find(|day| self.classify(*day).workday)
            .unwrap_or(date)
    }

    /// The day itself if it is a working one, else the first working day after it.
    pub fn next_workday(&self, date: NaiveDate) -> NaiveDate {
        (0..WORKDAY_SEARCH_DAYS)
            .map(|offset| date + chrono::Duration::days(offset))
            .find(|day| self.classify(*day).workday)
            .unwrap_or(date)
    }
}

impl WorkdayRules {
    /// Saturday and Sunday off plus the public holidays of Russia.
    pub fn ru_default() -> Self {
        Self {
            holidays: true,
            weekend: "sat_sun".into(),
            extra_off: Vec::new(),
            extra_work: Vec::new(),
        }
    }
}

/// Where a planned date that falls on a day off goes.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum WorkdayShift {
    /// Stays on the day off.
    #[default]
    None,
    /// To the last working day before it, the way salaries are paid.
    Back,
    /// To the first working day after it.
    Forward,
}

impl WorkdayShift {
    pub fn apply(self, date: NaiveDate, rules: &WorkdayRules) -> NaiveDate {
        match self {
            Self::None => date,
            Self::Back => rules.previous_workday(date),
            Self::Forward => rules.next_workday(date),
        }
    }

    /// The name a schedule parameter stores: `none`, `back` or `forward`.
    pub fn from_param(raw: &str) -> Self {
        match raw.trim() {
            "back" => Self::Back,
            "forward" => Self::Forward,
            _ => Self::None,
        }
    }

    pub fn param(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Back => "back",
            Self::Forward => "forward",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(raw: &str) -> NaiveDate {
        NaiveDate::parse_from_str(raw, "%Y-%m-%d").unwrap()
    }

    fn russia() -> WorkdayRules {
        WorkdayRules::ru_default()
    }

    #[test]
    fn weekends_and_holidays_are_days_off() {
        let rules = russia();
        // Tuesday.
        assert!(rules.classify(day("2026-03-10")).workday);
        // Saturday.
        let saturday = rules.classify(day("2026-03-14"));
        assert!(!saturday.workday && saturday.weekend && !saturday.holiday);
        // International Women's Day on a Sunday.
        let march = rules.classify(day("2026-03-08"));
        assert!(!march.workday && march.holiday);
        // June 12 on a Friday.
        assert!(!rules.classify(day("2026-06-12")).workday);
        // The New Year holidays run to the 8th; the 9th of January 2026 is a Friday.
        assert_eq!(rules.next_workday(day("2026-01-01")), day("2026-01-09"));
        assert_eq!(rules.next_workday(day("2026-03-10")), day("2026-03-10"));
    }

    #[test]
    fn the_users_dates_win() {
        let rules = WorkdayRules {
            extra_off: vec![day("2026-01-09")],
            extra_work: vec![day("2026-01-10")],
            ..russia()
        };
        assert!(!rules.classify(day("2026-01-09")).workday);
        let saturday = rules.classify(day("2026-01-10"));
        assert!(saturday.workday && saturday.weekend);
        assert_eq!(rules.next_workday(day("2026-01-01")), day("2026-01-10"));
        let only_weekends = WorkdayRules {
            holidays: false,
            ..russia()
        };
        assert!(only_weekends.classify(day("2026-06-12")).workday);
    }

    #[test]
    fn dates_parse_as_days_or_moments() {
        let cal = Calendar::new("Europe/Moscow");
        assert_eq!(parse_date("2026-03-08", &cal), Some(day("2026-03-08")));
        // 22:00 UTC is already the next day in Moscow.
        assert_eq!(
            parse_date("2026-03-07T22:00:00Z", &cal),
            Some(day("2026-03-08"))
        );
        assert_eq!(parse_dates("2026-01-09, 2026-12-31;bad", &cal).len(), 2);
    }

    #[test]
    fn a_payday_on_a_day_off_moves_to_a_working_day() {
        let rules = russia();
        // 2026-12-20 is a Sunday: the salary comes on Friday the 18th.
        assert_eq!(
            WorkdayShift::Back.apply(day("2026-12-20"), &rules),
            day("2026-12-18")
        );
        assert_eq!(
            WorkdayShift::Forward.apply(day("2026-12-20"), &rules),
            day("2026-12-21")
        );
        assert_eq!(
            WorkdayShift::None.apply(day("2026-12-20"), &rules),
            day("2026-12-20")
        );
        // January 5 is a holiday: back to the last working day of December.
        assert_eq!(
            WorkdayShift::Back.apply(day("2027-01-05"), &rules),
            day("2026-12-31")
        );
    }
}
