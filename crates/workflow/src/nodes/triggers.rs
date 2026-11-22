//! When triggers fire: the planned moments of a schedule, whether an
//! operation matches a filter, and what a trigger hands on about it.

use std::collections::BTreeMap;

use serde_json::{Map, Value};

use mwt_base::Amount;
use mwt_base::calendar::Calendar;
use mwt_model::StoreScope;
use mwt_model::{AccountRef, Transaction, TxOrigin};

use super::{json_field, json_number, json_text};
use crate::host::WorkflowHost;
use crate::types::WfValue;

pub use mwt_model::schedule_moments_limited;

/// Most missed moments of a schedule run at once; older ones are dropped.
pub const MAX_SCHEDULE_CATCH_UP: usize = 12;

/// Every moment a schedule plans in `(after, until]`, oldest first, and how
/// many older ones were left out beyond the catch-up limit.
pub fn schedule_moments(
    params: &Map<String, Value>,
    cal: &Calendar,
    after: i64,
    until: i64,
) -> (Vec<i64>, usize) {
    schedule_moments_limited(params, cal, after, until, MAX_SCHEDULE_CATCH_UP)
}

fn kind_name(tx: &Transaction) -> String {
    serde_json::to_value(tx.kind())
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// The amount, asset and account an operation is mostly about.
pub(crate) fn tx_main(tx: &Transaction) -> (WfValue, WfValue, WfValue) {
    let Some((account, asset, amount)) = mwt_model::main_leg(tx) else {
        return (WfValue::Null, WfValue::Null, WfValue::Null);
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

/// What "Transaction added" hands on.
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

/// The fields "Parse transaction" gives.
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

/// Whether an operation passes the filter of a "Transaction added" node.
pub fn tx_matches(host: &dyn WorkflowHost, params: &Map<String, Value>, tx: &Transaction) -> bool {
    if matches!(tx.meta().origin, TxOrigin::Workflow { .. })
        && !json_field(params, "include_workflows")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    {
        return false;
    }
    let kind = json_field(params, "tx_kind").map_or_else(|| "any".to_string(), json_text);
    if kind != "any" && kind != kind_name(tx) {
        return false;
    }
    if let Some(store) = json_field(params, "store")
        .and_then(super::json_account)
        .and_then(|account| match account {
            AccountRef::Store(id) => Some(id),
            _ => None,
        })
    {
        let ids = host.resolve_scope(&StoreScope {
            store_ids: vec![store],
            include_children: true,
        });
        if !ids.iter().any(|id| tx.touches(id)) {
            return false;
        }
    }
    if let Some(category) = json_field(params, "category")
        .map(json_text)
        .filter(|key| !key.trim().is_empty())
        && !tx.meta().categories.contains(&category)
    {
        return false;
    }
    if let Some(min) = json_field(params, "min_amount").and_then(json_number) {
        let amount = tx_main(tx).0.as_number().unwrap_or(Amount::ZERO);
        if amount < min {
            return false;
        }
    }
    let text = json_field(params, "text")
        .map(json_text)
        .unwrap_or_default();
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
    use chrono::Datelike;
    use serde_json::json;

    fn params(value: Value) -> Map<String, Value> {
        value.as_object().cloned().unwrap_or_default()
    }

    fn secs(iso: &str) -> i64 {
        mwt_base::time::unix_secs(iso).unwrap()
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
            .map(|moment| mwt_base::calendar::iso(*moment))
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

    fn dates_of(moments: &[i64]) -> Vec<String> {
        moments
            .iter()
            .map(|moment| mwt_base::calendar::iso(*moment))
            .collect()
    }

    #[test]
    fn a_payday_on_a_day_off_moves_back_to_friday() {
        let cal = Calendar::new("UTC");
        let plan =
            params(json!({ "every": "month", "day": 20, "time": "09:00", "workday": "back" }));
        // 2026-12-20 is a Sunday.
        let (moments, _) = schedule_moments(
            &plan,
            &cal,
            secs("2026-11-21T00:00:00Z"),
            secs("2026-12-31T00:00:00Z"),
        );
        assert_eq!(dates_of(&moments), vec!["2026-12-18T09:00:00Z"]);
        let forward = params(json!({ "every": "month", "day": 20, "workday": "forward" }));
        let (moments, _) = schedule_moments(
            &forward,
            &cal,
            secs("2026-11-21T00:00:00Z"),
            secs("2026-12-31T00:00:00Z"),
        );
        assert_eq!(dates_of(&moments), vec!["2026-12-21T09:00:00Z"]);
    }

    #[test]
    fn a_date_moved_back_to_today_fires_today() {
        let cal = Calendar::new("UTC");
        // 2027-01-05 is a holiday: the payment comes on 2026-12-31.
        let plan = params(json!({ "every": "month", "day": 5, "workday": "back" }));
        let (moments, _) = schedule_moments(
            &plan,
            &cal,
            secs("2026-12-30T12:00:00Z"),
            secs("2026-12-31T10:00:00Z"),
        );
        assert_eq!(dates_of(&moments), vec!["2026-12-31T09:00:00Z"]);
        // Not again once its moment is behind the mark.
        let (moments, _) = schedule_moments(
            &plan,
            &cal,
            secs("2026-12-31T09:00:00Z"),
            secs("2027-01-06T10:00:00Z"),
        );
        assert!(moments.is_empty());
    }

    #[test]
    fn days_moved_onto_one_working_day_fire_once() {
        let cal = Calendar::new("UTC");
        let plan = params(json!({ "every": "day", "workday": "back" }));
        // Friday to Monday morning: Saturday and Sunday move to Friday.
        let (moments, _) = schedule_moments(
            &plan,
            &cal,
            secs("2026-03-12T12:00:00Z"),
            secs("2026-03-16T10:00:00Z"),
        );
        assert_eq!(
            dates_of(&moments),
            vec!["2026-03-13T09:00:00Z", "2026-03-16T09:00:00Z"]
        );
    }

    #[test]
    fn the_catch_up_limit_can_be_raised() {
        let cal = Calendar::new("UTC");
        let plan = params(json!({ "every": "week", "weekday": 4 }));
        let (moments, skipped) = schedule_moments_limited(
            &plan,
            &cal,
            secs("2026-01-01T00:00:00Z"),
            secs("2026-06-01T00:00:00Z"),
            24,
        );
        // Fridays from January 2 to May 29: 22, over the default 12.
        assert_eq!(moments.len(), 22);
        assert_eq!(skipped, 0);
    }
}
