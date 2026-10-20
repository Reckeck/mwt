//! An automation is a workflow the app builds from the automation form
//! ([`AutomationSpec`]): «По расписанию» → «Создать операцию», with
//! «Средняя сумма прошлых операций» in between when the amount differs every
//! time. The form stays the place to edit it; the graph is rebuilt on save.

use serde_json::{Map, Value, json};

use crate::model::planning::{AmountMode, AutomationSpec, Schedule, TransactionDraft};
use crate::model::transaction::AccountRef;

use super::graph::{Edge, GRAPH_VERSION, Graph, Node, Position};

/// Node ids of the built graph.
pub const TRIGGER: &str = "t";
pub const AVERAGE: &str = "avg";
pub const ACTION: &str = "tx";
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

/// The schedule's parameters, as «По расписанию» reads them.
pub fn schedule_params(spec: &AutomationSpec) -> Map<String, Value> {
    let time = format!("{:02}:00", crate::calendar::AUTOMATION_HOUR);
    let mut params = match spec.schedule {
        Schedule::Monthly { day } => json!({ "every": "month", "day": day, "time": time }),
        Schedule::Weekly { weekday } => {
            json!({ "every": "week", "weekday": weekday, "time": time })
        }
        // Counted from the first date, as the form means it.
        Schedule::EveryNDays { days } => json!({
            "every": "day",
            "interval": days.max(1).to_string(),
            "anchor": spec.starts_at.get(..10).unwrap_or(&spec.starts_at),
            "time": time,
        }),
    }
    .as_object()
    .cloned()
    .unwrap_or_default();
    params.insert("workday".into(), json!(spec.workday_shift.param()));
    params
}

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

/// The readable fields of «Создать операцию» for a template: what the editor
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
    let mut edges = vec![
        edge(TRIGGER, "out", ACTION, "in"),
        edge(TRIGGER, "at", ACTION, "date"),
    ];
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
