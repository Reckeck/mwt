//! When triggers fire: the planned moments of a schedule, whether an
//! operation matches a filter, and what a trigger hands on about it.

use std::collections::BTreeMap;

use chrono::{Datelike, Duration, NaiveDate};
use serde_json::{Map, Value};

use crate::Amount;
use crate::calendar::{Calendar, clamp_day};
use crate::model::store::StoreScope;
use crate::model::transaction::{AccountRef, LegRole, Transaction, TxOrigin};
use crate::service::UnlockedVault;

use super::{json_number, json_text};
use crate::workflow::types::WfValue;

/// Most missed moments of a schedule run at once; older ones are dropped.
pub const MAX_SCHEDULE_CATCH_UP: usize = 12;
/// How far back a schedule looks for missed moments.
const LOOKBACK_DAYS: i64 = 400;

fn field<'map>(params: &'map Map<String, Value>, name: &str) -> Option<&'map Value> {
    params.get(name).filter(|value| !value.is_null())
}

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

/// Every moment a schedule plans in `(after, until]`, oldest first, and how
/// many older ones were left out beyond the catch-up limit.
pub fn schedule_moments(
    params: &Map<String, Value>,
    cal: &Calendar,
    after: i64,
    until: i64,
) -> (Vec<i64>, usize) {
    let every = field(params, "every").map_or_else(|| "month".to_string(), json_text);
    let interval = whole(field(params, "interval"), 1).max(1);
    let day = whole(field(params, "day"), 1).clamp(1, 31) as u8;
    let weekday = whole(field(params, "weekday"), 4).clamp(0, 6);
    let (hour, minute) =
        hour_minute(&field(params, "time").map_or_else(|| "09:00".to_string(), json_text));

    let last = cal.date_of(until);
    let first = cal.date_of(after).max(last - Duration::days(LOOKBACK_DAYS));
    let epoch = NaiveDate::from_ymd_opt(1970, 1, 5).unwrap_or_default(); // a Monday
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
            let moment = cal.at_local_hm(date, hour, minute);
            if moment > after && moment <= until {
                out.push(moment);
            }
        }
        date += Duration::days(1);
    }
    let skipped = out.len().saturating_sub(MAX_SCHEDULE_CATCH_UP);
    out.drain(..skipped);
    (out, skipped)
}

fn kind_name(tx: &Transaction) -> String {
    serde_json::to_value(tx.kind())
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// The amount, asset and account an operation is mostly about.
pub(crate) fn tx_main(tx: &Transaction) -> (WfValue, WfValue, WfValue) {
    let (account, asset, amount) = match tx {
        Transaction::Topup(inner) => (
            AccountRef::Store(inner.store_id.clone()),
            inner.asset.clone(),
            inner.amount,
        ),
        Transaction::Payment(inner) => (
            AccountRef::Store(inner.store_id.clone()),
            inner.asset.clone(),
            inner.amount,
        ),
        Transaction::Transfer(inner) => (inner.from.clone(), inner.asset.clone(), inner.amount),
        Transaction::Convert(inner) => (
            inner.from.account.clone(),
            inner.from.asset.clone(),
            inner.from.amount,
        ),
        other => {
            let legs = other.legs();
            let Some(leg) = legs
                .iter()
                .find(|leg| leg.role == LegRole::Principal)
                .or_else(|| legs.first())
            else {
                return (WfValue::Null, WfValue::Null, WfValue::Null);
            };
            (
                AccountRef::Store(leg.store_id.clone()),
                leg.asset.clone(),
                leg.amount,
            )
        }
    };
    (
        WfValue::money(amount.abs(), &asset.symbol),
        WfValue::Asset(asset),
        WfValue::Account(account),
    )
}

fn first_category(tx: &Transaction) -> WfValue {
    tx.meta()
        .categories
        .first()
        .map_or(WfValue::Null, |key| WfValue::Category(key.clone()))
}

/// What «Добавлена операция» hands on.
pub fn tx_trigger_values(tx: &Transaction) -> BTreeMap<String, WfValue> {
    let (amount, _, account) = tx_main(tx);
    BTreeMap::from([
        (
            "transaction".to_string(),
            WfValue::Transaction(Box::new(tx.clone())),
        ),
        ("amount".to_string(), amount),
        ("account".to_string(), account),
        ("category".to_string(), first_category(tx)),
    ])
}

/// The fields «Разобрать операцию» gives.
pub(crate) fn tx_fields(tx: &Transaction) -> super::Outputs {
    let (amount, asset, account) = tx_main(tx);
    super::Outputs::from([
        ("amount", amount),
        ("asset", asset),
        ("account", account),
        ("category", first_category(tx)),
        ("comment", WfValue::Text(tx.meta().comment.clone())),
        ("date", WfValue::Date(tx.meta().timestamp.clone())),
    ])
}

/// Whether an operation passes the filter of a «Добавлена операция» node.
pub fn tx_matches(vault: &UnlockedVault, params: &Map<String, Value>, tx: &Transaction) -> bool {
    if matches!(tx.meta().origin, TxOrigin::Workflow { .. })
        && !field(params, "include_workflows")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    {
        return false;
    }
    let kind = field(params, "tx_kind").map_or_else(|| "any".to_string(), json_text);
    if kind != "any" && kind != kind_name(tx) {
        return false;
    }
    if let Some(store) = field(params, "store")
        .and_then(super::json_account)
        .and_then(|account| match account {
            AccountRef::Store(id) => Some(id),
            _ => None,
        })
    {
        let ids = vault.resolve_scope(&StoreScope {
            store_ids: vec![store],
            include_children: true,
        });
        if !ids.iter().any(|id| tx.touches(id)) {
            return false;
        }
    }
    if let Some(category) = field(params, "category")
        .map(json_text)
        .filter(|key| !key.trim().is_empty())
        && !tx.meta().categories.contains(&category)
    {
        return false;
    }
    if let Some(min) = field(params, "min_amount").and_then(json_number) {
        let amount = tx_main(tx).0.as_number().unwrap_or(Amount::ZERO);
        if amount < min {
            return false;
        }
    }
    let text = field(params, "text").map(json_text).unwrap_or_default();
    if !text.trim().is_empty()
        && !tx
            .meta()
            .comment
            .to_lowercase()
            .contains(&text.trim().to_lowercase())
    {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn params(value: Value) -> Map<String, Value> {
        value.as_object().cloned().unwrap_or_default()
    }

    fn secs(iso: &str) -> i64 {
        crate::pricing::history::unix_secs(iso).unwrap()
    }

    #[test]
    fn a_monthly_schedule_plans_its_day_and_clamps_it() {
        let cal = Calendar::new("UTC");
        let plan = params(json!({ "every": "month", "day": 31, "time": "09:30" }));
        let (moments, skipped) = schedule_moments(
            &plan,
            &cal,
            secs("2026-01-31T10:00:00Z"),
            secs("2026-04-30T12:00:00Z"),
        );
        let dates: Vec<String> = moments
            .iter()
            .map(|moment| crate::calendar::iso(*moment))
            .collect();
        assert_eq!(
            dates,
            vec![
                "2026-02-28T09:30:00Z",
                "2026-03-31T09:30:00Z",
                "2026-04-30T09:30:00Z"
            ]
        );
        assert_eq!(skipped, 0);
    }

    #[test]
    fn a_weekly_schedule_and_the_catch_up_limit() {
        let cal = Calendar::new("UTC");
        // Friday = 4.
        let plan = params(json!({ "every": "week", "weekday": 4, "time": "08:00" }));
        let (moments, skipped) = schedule_moments(
            &plan,
            &cal,
            secs("2026-01-01T00:00:00Z"),
            secs("2026-06-01T00:00:00Z"),
        );
        assert_eq!(moments.len(), MAX_SCHEDULE_CATCH_UP);
        assert!(skipped > 0);
        for moment in moments {
            assert_eq!(cal.date_of(moment).weekday(), chrono::Weekday::Fri);
        }
    }

    #[test]
    fn nothing_is_planned_before_the_mark() {
        let cal = Calendar::new("UTC");
        let plan = params(json!({ "every": "day", "time": "09:00" }));
        let (moments, _) = schedule_moments(
            &plan,
            &cal,
            secs("2026-03-01T09:00:00Z"),
            secs("2026-03-01T20:00:00Z"),
        );
        assert!(moments.is_empty());
        let (moments, _) = schedule_moments(
            &plan,
            &cal,
            secs("2026-03-01T09:00:00Z"),
            secs("2026-03-02T09:00:00Z"),
        );
        assert_eq!(moments.len(), 1);
    }
}
