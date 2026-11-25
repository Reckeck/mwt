//! What an automation is on the schedule side, over the document and the
//! model types only: the mark and the parameters of its schedule, the
//! estimate its amount varies by, and who made an operation.

use serde_json::{Map, Value, json};

use mwt_base::Amount;

use crate::{
    AccountRef, AssetRef, AutomationSpec, LegRole, Schedule, Transaction, TxOrigin, VaultDoc,
    WorkdayShift,
};

/// Node id of the schedule trigger in a built automation graph.
pub const TRIGGER_NODE: &str = "t";
/// How many latest corrected amounts make the estimate.
pub const ESTIMATE_OF: usize = 3;

/// The mark key of the schedule in `WorkflowState::marks`.
pub fn schedule_key() -> String {
    format!("schedule:{TRIGGER_NODE}")
}

/// The mark a new automation starts from: nothing planned before its start.
pub fn start_mark(spec: &AutomationSpec) -> String {
    mwt_base::time::unix_secs(&spec.starts_at).map_or_else(
        || spec.starts_at.clone(),
        |secs| mwt_base::calendar::iso(secs - 1),
    )
}

/// «HH:MM» as hours and minutes, or `None` when it is not a time of day.
pub fn parse_time(time: &str) -> Option<(u32, u32)> {
    let (hours, minutes) = time.split_once(':')?;
    let (hours, minutes) = (hours.parse::<u32>().ok()?, minutes.parse::<u32>().ok()?);
    (hours < 24 && minutes < 60 && time.len() == 5).then_some((hours, minutes))
}

/// The local time an automation fires, «HH:MM»: 09:00 unless set.
pub fn fire_time(time: Option<&str>) -> String {
    time.filter(|time| parse_time(time).is_some()).map_or_else(
        || format!("{:02}:00", mwt_base::calendar::AUTOMATION_HOUR),
        str::to_string,
    )
}

/// The schedule's parameters, as "on schedule" reads them.
pub fn schedule_params(spec: &AutomationSpec) -> Map<String, Value> {
    when_params(&When {
        schedule: spec.schedule,
        starts_at: &spec.starts_at,
        workday_shift: spec.workday_shift,
        time: spec.time.as_deref(),
    })
}

/// When an automation fires, without what it does: enough to list its dates.
pub struct When<'a> {
    pub schedule: Schedule,
    /// RFC3339, the first planned date.
    pub starts_at: &'a str,
    pub workday_shift: WorkdayShift,
    pub time: Option<&'a str>,
}

/// "on schedule" parameters of a [`When`].
pub fn when_params(when: &When<'_>) -> Map<String, Value> {
    let time = fire_time(when.time);
    let mut params = match when.schedule {
        Schedule::Monthly { day } => json!({ "every": "month", "day": day, "time": time }),
        Schedule::Weekly { weekday } => {
            json!({ "every": "week", "weekday": weekday, "time": time })
        }
        // Counted from the first date, as the form means it.
        Schedule::EveryNDays { days } => json!({
            "every": "day",
            "interval": days.max(1).to_string(),
            "anchor": when.starts_at.get(..10).unwrap_or(when.starts_at),
            "time": time,
        }),
    }
    .as_object()
    .cloned()
    .unwrap_or_default();
    params.insert("workday".into(), json!(when.workday_shift.param()));
    params
}

/// One mark of a workflow in the document.
pub fn workflow_mark(doc: &VaultDoc, workflow_id: &str, key: &str) -> Option<String> {
    doc.workflow_state
        .iter()
        .find(|state| state.workflow_id == workflow_id)
        .and_then(|state| state.marks.get(key).cloned())
}

/// Whether a workflow (or the automation it was migrated from, which kept its
/// id) booked the operation itself. An operation with `income_ref` is an effect
/// of an income (a distribution's transfer), not something the automation
/// booked: it carries the workflow's origin but is never read as its own.
pub fn made_by(tx: &Transaction, workflow_id: &str) -> bool {
    if tx.meta().income_ref.is_some() {
        return false;
    }
    match &tx.meta().origin {
        TxOrigin::Workflow {
            workflow_id: maker, ..
        }
        | TxOrigin::Automation {
            automation_id: maker,
            ..
        } => maker == workflow_id,
        _ => false,
    }
}

/// The account, asset and signed amount an operation is mostly about, or
/// `None` for one without legs.
pub fn main_leg(tx: &Transaction) -> Option<(AccountRef, AssetRef, Amount)> {
    Some(match tx {
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
            let leg = legs
                .iter()
                .find(|leg| leg.role == LegRole::Principal)
                .or_else(|| legs.first())?;
            (
                AccountRef::Store(leg.store_id.clone()),
                leg.asset.clone(),
                leg.amount,
            )
        }
    })
}

/// The average amount of the latest `count` operations a workflow made (or the
/// automation it was before, which kept its id), leaving out the ones still
/// booked with an approximate amount; with the asset symbol.
pub fn recent_average(doc: &VaultDoc, workflow_id: &str, count: usize) -> Option<(Amount, String)> {
    let mut made: Vec<&Transaction> = doc
        .transactions
        .iter()
        .filter(|tx| !tx.meta().origin.is_estimated() && made_by(tx, workflow_id))
        .collect();
    made.sort_by(|left, right| right.meta().timestamp.cmp(&left.meta().timestamp));
    made.truncate(count.max(1));
    let first = made.first()?;
    let symbol = main_leg(first)
        .map(|(_, asset, _)| asset.symbol)
        .unwrap_or_default();
    let total = made.iter().fold(Amount::ZERO, |sum, tx| {
        sum + main_leg(tx).map_or(Amount::ZERO, |(_, _, amount)| amount.abs())
    });
    let len = Amount::new(rust_decimal::Decimal::from(made.len()));
    Some(((total / len).round(2), symbol))
}
