//! An automation is a workflow the app builds from the automation form
//! ([`AutomationSpec`]): "on schedule" → "Create transaction", with
//! "Average of past transactions" in between when the amount differs every
//! time. The form stays the place to edit it; the graph is rebuilt on save.

use serde_json::{Map, Value, json};

use crate::model::planning::{
    AmountMode, AutomationSpec, Schedule, TransactionDraft, WorkdayShift,
};
use crate::model::transaction::AccountRef;

use super::graph::{Edge, GRAPH_VERSION, Graph, Node, Position};

/// Node ids of the built graph.
pub const TRIGGER: &str = "t";
pub const AVERAGE: &str = "avg";
pub const ACTION: &str = "tx";
pub const ASK: &str = "ask";
/// How long "yes" is waited for before the run gives up.
const ASK_HOURS: u32 = 72;
/// How many latest corrected amounts make the estimate.
pub const ESTIMATE_OF: usize = 3;

/// The mark key of the schedule in `WorkflowState::marks`.
pub fn schedule_key() -> String {
    format!("schedule:{TRIGGER}")
}

/// The mark a new automation starts from: nothing planned before its start.
pub fn start_mark(spec: &AutomationSpec) -> String {
    crate::pricing::history::unix_secs(&spec.starts_at).map_or_else(
        || spec.starts_at.clone(),
        |secs| crate::calendar::iso(secs - 1),
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
        || format!("{:02}:00", crate::calendar::AUTOMATION_HOUR),
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

/// The first `count` dates a schedule fires on after `now`, not before its
/// start: "Next: 2 Nov (Mon), 1 Dec (Tue), 31 Dec (Thu)" under the form.
pub fn upcoming(
    when: &When<'_>,
    cal: &crate::calendar::Calendar,
    now: i64,
    count: usize,
) -> Vec<i64> {
    let start = crate::pricing::history::unix_secs(when.starts_at).map_or(now, |secs| secs - 1);
    let after = now.max(start);
    let params = when_params(when);
    let mut moments = Vec::new();
    let mut from = after;
    // The schedule reader looks back a bounded span: a year at a time.
    for _ in 0..UPCOMING_YEARS {
        let until = from + chrono::Duration::days(366).num_seconds();
        let (found, _) = crate::workflow::nodes::triggers::schedule_moments_limited(
            &params,
            cal,
            from,
            until,
            usize::MAX,
        );
        moments.extend(found);
        if moments.len() >= count {
            break;
        }
        from = until;
    }
    moments.truncate(count);
    moments
}

/// How many years ahead [`upcoming`] looks: three yearly dates fit.
const UPCOMING_YEARS: usize = 4;

/// The template's main amount: what a top-up, a payment or a transfer moves.
pub fn template_amount(template: &TransactionDraft) -> crate::Amount {
    match template {
        TransactionDraft::Topup(inner) => inner.amount,
        TransactionDraft::Payment(inner) => inner.amount,
        TransactionDraft::Transfer(inner) => inner.amount,
        TransactionDraft::Convert(inner) => inner.from.amount,
    }
}

fn store(id: &str) -> Value {
    serde_json::to_value(AccountRef::Store(id.to_string())).unwrap_or(Value::Null)
}

fn account(account: &AccountRef) -> Value {
    serde_json::to_value(account).unwrap_or(Value::Null)
}

/// The readable fields of "Create transaction" for a template: what the editor
/// shows. The operation itself comes from `template`, fee and all.
fn action_params(spec: &AutomationSpec) -> Map<String, Value> {
    let template = &spec.template;
    let (kind, from, to, asset, meta) = match template {
        TransactionDraft::Topup(inner) => (
            "topup",
            store(&inner.store_id),
            inner.from.as_ref().map_or(Value::Null, account),
            &inner.asset,
            &inner.meta,
        ),
        TransactionDraft::Payment(inner) => (
            "payment",
            store(&inner.store_id),
            inner.to.as_ref().map_or(Value::Null, account),
            &inner.asset,
            &inner.meta,
        ),
        TransactionDraft::Transfer(inner) => (
            "transfer",
            account(&inner.from),
            account(&inner.to),
            &inner.asset,
            &inner.meta,
        ),
        TransactionDraft::Convert(inner) => (
            "convert",
            account(&inner.from.account),
            account(&inner.to.account),
            &inner.from.asset,
            &inner.meta,
        ),
    };
    let mut params = Map::new();
    params.insert("tx_kind".into(), json!(kind));
    params.insert("account".into(), from);
    params.insert("to".into(), to);
    params.insert(
        "asset".into(),
        serde_json::to_value(asset).unwrap_or(Value::Null),
    );
    params.insert(
        "amount".into(),
        json!(template_amount(template).to_string()),
    );
    if let TransactionDraft::Convert(inner) = template {
        params.insert(
            "to_asset".into(),
            serde_json::to_value(&inner.to.asset).unwrap_or(Value::Null),
        );
        params.insert("to_amount".into(), json!(inner.to.amount.to_string()));
    }
    params.insert(
        "category".into(),
        meta.categories
            .first()
            .map_or(Value::Null, |key| json!(key)),
    );
    params.insert("comment".into(), json!(meta.comment));
    params.insert(
        "template".into(),
        json!(serde_json::to_string(template).unwrap_or_default()),
    );
    params.insert(
        "estimated".into(),
        json!(spec.amount_mode == AmountMode::Varies),
    );
    params.insert("on_shortfall".into(), json!("reject"));
    if let Some(offset) = spec.for_month_offset {
        // "For which month" of an income, relative to the planned month.
        params.insert("for_month_offset".into(), json!(offset));
        if let Schedule::Monthly { day } = spec.schedule {
            params.insert("for_month_day".into(), json!(day));
        }
    }
    params
}

fn node(id: &str, kind: &str, params: Map<String, Value>, column: f64) -> Node {
    Node {
        id: id.into(),
        kind: kind.into(),
        params,
        position: Position {
            left: column * 280.0,
            top: 0.0,
        },
        disabled: false,
        label: String::new(),
    }
}

fn edge(from: &str, from_port: &str, to: &str, to_port: &str) -> Edge {
    Edge {
        id: format!("{from}.{from_port}>{to}.{to_port}"),
        from: from.into(),
        from_port: from_port.into(),
        to: to.into(),
        to_port: to_port.into(),
    }
}

/// The workflow graph an automation runs as.
pub fn automation_graph(spec: &AutomationSpec) -> Graph {
    let mut nodes = vec![
        node(TRIGGER, "trigger.schedule", schedule_params(spec), 0.0),
        node(ACTION, "action.transaction", action_params(spec), 2.0),
    ];
    let mut edges = vec![edge(TRIGGER, "at", ACTION, "date")];
    if spec.asks {
        // "Ask yes before booking": the operation waits for the answer.
        let mut params = Map::new();
        params.insert("timeout_hours".into(), json!(ASK_HOURS));
        let mut ask = node(ASK, "action.confirm", params, 1.0);
        ask.position.top = -160.0;
        nodes.push(ask);
        edges.push(edge(TRIGGER, "out", ASK, "in"));
        edges.push(edge(ASK, "true", ACTION, "in"));
    } else {
        edges.push(edge(TRIGGER, "out", ACTION, "in"));
    }
    if spec.amount_mode == AmountMode::Varies {
        let mut params = Map::new();
        params.insert("count".into(), json!(ESTIMATE_OF.to_string()));
        params.insert(
            "fallback".into(),
            json!(template_amount(&spec.template).to_string()),
        );
        let mut average = node(AVERAGE, "data.recent_average", params, 1.0);
        average.position.top = 160.0;
        nodes.push(average);
        edges.push(edge(AVERAGE, "amount", ACTION, "amount"));
    }
    Graph {
        version: GRAPH_VERSION,
        nodes,
        edges,
        comments: Vec::new(),
    }
}
