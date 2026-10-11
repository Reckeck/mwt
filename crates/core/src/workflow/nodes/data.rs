//! Data nodes: balances, prices, rates, sums, budgets, goals, the date.

use chrono::{Datelike, NaiveDate, Weekday};

use crate::Amount;
use crate::calendar::Calendar;
use crate::model::transaction::AccountRef;
use crate::workflow::exec::Executor;
use crate::workflow::graph::Node;
use crate::workflow::types::WfValue;

use super::{Outputs, json_number, single};

fn number(value: Amount) -> WfValue {
    WfValue::Number(value)
}

fn percent(ratio: Amount) -> Amount {
    (ratio * Amount::new(rust_decimal::Decimal::ONE_HUNDRED)).round(2)
}

impl Executor<'_> {
    pub(crate) fn eval_data(&mut self, node: &Node) -> Result<Outputs, String> {
        let now = self.ctx.now;
        match node.kind.as_str() {
            "data.balance" => {
                let account = self.account_input(node, "account")?;
                let Some(AccountRef::Store(store_id)) = account else {
                    return Err("choose one of your accounts".into());
                };
                let asset = self.asset_input(node, "asset")?;
                Ok(single(
                    "value",
                    self.vault.wf_balance(&store_id, asset.as_ref())?,
                ))
            }
            "data.price" => {
                let asset = self.asset_input(node, "asset")?.ok_or("choose an asset")?;
                let currency = self
                    .currency_input(node, "currency")?
                    .unwrap_or_else(|| "usd".into());
                let price = self.vault.wf_price(&asset, &currency, now).ok_or_else(|| {
                    format!(
                        "no price of {} in {}",
                        asset.symbol,
                        currency.to_uppercase()
                    )
                })?;
                Ok(single("price", WfValue::money(price, &currency)))
            }
            "data.fx" => {
                let from = self
                    .currency_input(node, "from")?
                    .ok_or("choose the currency to convert from")?;
                let to = self
                    .currency_input(node, "to")?
                    .ok_or("choose the currency to convert to")?;
                let rate = self
                    .vault
                    .wf_convert(Amount::new(rust_decimal::Decimal::ONE), &from, &to, now)
                    .ok_or_else(|| {
                        format!("no rate {}→{}", from.to_uppercase(), to.to_uppercase())
                    })?;
                Ok(single("rate", number(rate)))
            }
            "data.constant" => {
                let kind = self.text_param(node, "value_type");
                let value = match kind.as_str() {
                    "money" => {
                        let amount =
                            json_number(&self.raw_param(node, "number")).unwrap_or(Amount::ZERO);
                        WfValue::money(amount, &self.text_param(node, "currency"))
                    }
                    "text" => self.param(node, "text"),
                    "bool" => self.param(node, "bool"),
                    "account" => self.param(node, "account"),
                    "asset" => self.param(node, "asset"),
                    "category" => self.param(node, "category"),
                    _ => self.param(node, "number"),
                };
                Ok(single("value", value))
            }
            "data.sum" => {
                let period = self.text_param(node, "period");
                let flow = self.text_param(node, "flow");
                let category = self.text_param(node, "category");
                let category = (!category.trim().is_empty()).then_some(category);
                let total =
                    self.vault
                        .wf_sum(&period, &flow, category.as_deref(), now, &self.ctx.zone);
                Ok(single(
                    "total",
                    WfValue::money(total, self.vault.base_currency()),
                ))
            }
            "data.budget" => {
                let id = self.text_param(node, "budget");
                let status = self
                    .vault
                    .budget_status(now, &self.ctx.zone)
                    .into_iter()
                    .find(|status| status.budget.id == id)
                    .ok_or("choose an active limit")?;
                Ok(Outputs::from([
                    (
                        "remaining",
                        WfValue::money(status.remaining, &status.budget.currency),
                    ),
                    ("used", number(percent(status.used))),
                ]))
            }
            "data.goal" => {
                let id = self.text_param(node, "goal");
                let progress = self
                    .vault
                    .goal_progress(now, &self.ctx.zone)
                    .into_iter()
                    .find(|progress| progress.goal.id == id)
                    .ok_or("choose an active goal")?;
                Ok(goal_outputs(&progress))
            }
            "data.now" => {
                let date = Calendar::new(&self.ctx.zone).date_of(now);
                Ok(Outputs::from([
                    ("now", WfValue::Date(crate::calendar::iso(now))),
                    ("day", number(Amount::new(date.day().into()))),
                    (
                        "weekday",
                        number(Amount::new(date.weekday().num_days_from_monday().into())),
                    ),
                ]))
            }
            "data.workday" => {
                let cal = Calendar::new(&self.ctx.zone);
                let date = match self.input(node, "date")? {
                    WfValue::Date(text) | WfValue::Text(text) if !text.trim().is_empty() => {
                        parse_date(&text, &cal).ok_or_else(|| format!("«{text}» is not a date"))?
                    }
                    _ => cal.date_of(now),
                };
                let rules = WorkdayRules {
                    holidays: self.text_param(node, "calendar") != "weekends",
                    weekend: self.text_param(node, "weekend"),
                    extra_off: parse_dates(&self.text_param(node, "extra_off"), &cal),
                    extra_work: parse_dates(&self.text_param(node, "extra_work"), &cal),
                };
                let day = rules.classify(date);
                Ok(Outputs::from([
                    ("workday", WfValue::Bool(day.workday)),
                    ("weekend", WfValue::Bool(day.weekend)),
                    ("holiday", WfValue::Bool(day.holiday)),
                    (
                        "next_workday",
                        WfValue::Date(rules.next_workday(date).to_string()),
                    ),
                ]))
            }
            "data.tx_fields" => match self.input(node, "transaction")? {
                WfValue::Transaction(tx) => Ok(super::triggers::tx_fields(&tx)),
                WfValue::Null => Ok(Outputs::new()),
                other => Err(format!("not an operation: {}", other.as_text())),
            },
            other => Err(format!("unknown data node {other}")),
        }
    }
}

/// Saved, left and percent of a goal.
pub(crate) fn goal_outputs(progress: &crate::service::GoalProgress) -> Outputs {
    let currency = &progress.goal.currency;
    let left = (progress.goal.target_amount - progress.saved).max(Amount::ZERO);
    Outputs::from([
        ("saved", WfValue::money(progress.saved, currency)),
        ("left", WfValue::money(left, currency)),
        ("pct", number(progress.pct.round(2))),
    ])
}

/// A date written as `2026-03-08` or as an RFC3339 moment (its local day).
pub(crate) fn parse_date(text: &str, cal: &Calendar) -> Option<NaiveDate> {
    let text = text.trim();
    crate::pricing::history::unix_secs(text)
        .map(|secs| cal.date_of(secs))
        .or_else(|| NaiveDate::parse_from_str(text.get(..10).unwrap_or(text), "%Y-%m-%d").ok())
}

fn parse_dates(text: &str, cal: &Calendar) -> Vec<NaiveDate> {
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
pub(crate) struct DayKind {
    pub workday: bool,
    pub weekend: bool,
    pub holiday: bool,
}

/// What makes a day a day off: weekdays, holidays and the user's own dates.
pub(crate) struct WorkdayRules {
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

    /// The day itself if it is a working one, else the first working day after it.
    pub fn next_workday(&self, date: NaiveDate) -> NaiveDate {
        (0..WORKDAY_SEARCH_DAYS)
            .map(|offset| date + chrono::Duration::days(offset))
            .find(|day| self.classify(*day).workday)
            .unwrap_or(date)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(raw: &str) -> NaiveDate {
        NaiveDate::parse_from_str(raw, "%Y-%m-%d").unwrap()
    }

    fn russia() -> WorkdayRules {
        WorkdayRules {
            holidays: true,
            weekend: "sat_sun".into(),
            extra_off: Vec::new(),
            extra_work: Vec::new(),
        }
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
}
