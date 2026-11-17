//! Payload v2, frozen: what changed in v3 is copied here as it was.
//!
//! v3 runs automations as workflows: the `automations` and `automation_runs`
//! collections are gone, each automation became a workflow built from its
//! form (`Workflow::automation`), keeping its id, and each run a workflow run.
//! Everything else of the document is read with the current types (v2 files
//! carry field names, so a later field with a default reads fine).
//!
//! Never edit these definitions: they describe files already on users' disks.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::model::planning::{
    AmountMode, AutomationOutcome, AutomationPreset, AutomationSpec, RejectReason, Schedule,
    TransactionDraft, WorkdayShift, describe_reject,
};
use crate::model::transaction::AccountRef;
use crate::model::vault::VaultDoc;
use crate::model::workflow::{
    MAX_WORKFLOW_RUNS, RunOutcome, StepStatus, TraceStep, Workflow, WorkflowRun, WorkflowState,
};

/// Node ids of the automation graph, as v2 files carry them.
const TRIGGER: &str = "t";
const ACTION: &str = "tx";

/// An automation as v1 and v2 stored it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[cfg_attr(test, derive(serde::Serialize))]
pub struct AutomationV2 {
    pub id: String,
    pub name: String,
    pub active: bool,
    #[serde(default)]
    pub preset: Option<AutomationPreset>,
    pub schedule: Schedule,
    pub starts_at: String,
    pub template: TransactionDraft,
    pub last_planned_at: Option<String>,
    pub created_at: String,
}

/// A processed planned date as v1 and v2 stored it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[cfg_attr(test, derive(serde::Serialize))]
pub struct AutomationRunV2 {
    pub id: String,
    pub automation_id: String,
    pub planned_at: String,
    pub processed_at: String,
    pub outcome: AutomationOutcome,
}

/// The v2 document: today's one plus the two automation collections.
#[derive(Deserialize)]
pub struct VaultDocV2 {
    #[serde(flatten)]
    pub doc: VaultDoc,
    #[serde(default)]
    pub automations: Vec<AutomationV2>,
    #[serde(default)]
    pub automation_runs: Vec<AutomationRunV2>,
}

/// v2 → v3: each automation becomes a workflow with the same id, so the
/// operations it made (`TxOrigin::Automation`) still point at it; its last
/// processed date becomes the schedule's mark, so nothing runs twice; each
/// run becomes a workflow run with the same outcome.
impl From<VaultDocV2> for VaultDoc {
    fn from(old: VaultDocV2) -> Self {
        let mut doc = old.doc;
        doc.version = 3;
        for automation in old.automations {
            let spec = AutomationSpec {
                time: None,
                asks: false,
                for_month_offset: None,
                preset: automation.preset,
                schedule: automation.schedule,
                starts_at: automation.starts_at.clone(),
                template: automation.template,
                amount_mode: AmountMode::Fixed,
                workday_shift: WorkdayShift::None,
                property_id: None,
            };
            let graph = frozen_graph(&spec);
            let mark = automation
                .last_planned_at
                .clone()
                .unwrap_or_else(|| start_mark(&spec));
            doc.workflow_state.push(WorkflowState {
                workflow_id: automation.id.clone(),
                marks: BTreeMap::from([(format!("schedule:{TRIGGER}"), mark)]),
                tx_cursor: doc.transactions.last().map(|tx| tx.meta().id.clone()),
                tx_index: doc.transactions.len(),
            });
            doc.workflows.push(Workflow {
                id: automation.id,
                name: automation.name,
                description: String::new(),
                active: automation.active,
                graph,
                created_at: automation.created_at.clone(),
                updated_at: automation.created_at,
                react_to_workflows: false,
                max_amount: None,
                max_amount_currency: None,
                versions: Vec::new(),
                tests: Vec::new(),
                draft: None,
                automation: Some(spec),
            });
        }
        let mut runs: Vec<WorkflowRun> = old.automation_runs.into_iter().map(run_of).collect();
        runs.extend(std::mem::take(&mut doc.workflow_runs));
        runs.sort_by(|left, right| left.started_at.cmp(&right.started_at));
        let overflow = runs.len().saturating_sub(MAX_WORKFLOW_RUNS);
        runs.drain(..overflow);
        doc.workflow_runs = runs;
        doc
    }
}

/// The mark the schedule starts from: nothing planned before the start.
fn start_mark(spec: &AutomationSpec) -> String {
    frozen_unix_secs(&spec.starts_at).map_or_else(
        || spec.starts_at.clone(),
        |secs| crate::calendar::iso(secs - 1),
    )
}

/// Unix seconds of an RFC3339 timestamp, as v2 read the start of a schedule.
fn frozen_unix_secs(iso: &str) -> Option<i64> {
    use time::format_description::well_known::Rfc3339;
    time::OffsetDateTime::parse(iso.trim(), &Rfc3339)
        .ok()
        .map(|moment| moment.unix_timestamp())
}

/// The graph JSON the v2 → v3 step stores, as the builder made it at v3
/// (graph version 3) for an automation of fixed amount, without a question
/// or a month offset: "on schedule" → "Create transaction". A copy, so a later
/// change of the live builder cannot change what a v2 file migrates to.
fn frozen_graph(spec: &AutomationSpec) -> String {
    let graph = FrozenGraph {
        version: 3,
        nodes: vec![
            frozen_node(
                TRIGGER,
                "trigger.schedule",
                frozen_schedule_params(spec),
                0.0,
            ),
            frozen_node(
                ACTION,
                "action.transaction",
                frozen_action_params(spec),
                2.0,
            ),
        ],
        edges: vec![
            frozen_edge(TRIGGER, "at", ACTION, "date"),
            frozen_edge(TRIGGER, "out", ACTION, "in"),
        ],
        comments: Vec::new(),
    };
    serde_json::to_string(&graph).unwrap_or_default()
}

// The fields follow the order the live graph writes its JSON in.
#[derive(Serialize)]
struct FrozenGraph {
    version: u32,
    nodes: Vec<FrozenNode>,
    edges: Vec<FrozenEdge>,
    comments: Vec<Value>,
}

#[derive(Serialize)]
struct FrozenNode {
    id: String,
    kind: String,
    params: Map<String, Value>,
    position: FrozenPosition,
    disabled: bool,
    label: String,
}

#[derive(Serialize)]
struct FrozenPosition {
    #[serde(rename = "x")]
    horizontal: f64,
    #[serde(rename = "y")]
    vertical: f64,
}

#[derive(Serialize)]
struct FrozenEdge {
    id: String,
    from: String,
    from_port: String,
    to: String,
    to_port: String,
}

fn frozen_node(id: &str, kind: &str, params: Map<String, Value>, column: f64) -> FrozenNode {
    FrozenNode {
        id: id.into(),
        kind: kind.into(),
        params,
        position: FrozenPosition {
            horizontal: column * 280.0,
            vertical: 0.0,
        },
        disabled: false,
        label: String::new(),
    }
}

fn frozen_edge(from: &str, from_port: &str, to: &str, to_port: &str) -> FrozenEdge {
    FrozenEdge {
        id: format!("{from}.{from_port}>{to}.{to_port}"),
        from: from.into(),
        from_port: from_port.into(),
        to: to.into(),
        to_port: to_port.into(),
    }
}

/// "On schedule" parameters; the migrated spec has no time and no shift.
fn frozen_schedule_params(spec: &AutomationSpec) -> Map<String, Value> {
    let time = "09:00";
    let mut params = match spec.schedule {
        Schedule::Monthly { day } => json!({ "every": "month", "day": day, "time": time }),
        Schedule::Weekly { weekday } => {
            json!({ "every": "week", "weekday": weekday, "time": time })
        }
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
    params.insert("workday".into(), json!("none"));
    params
}

fn frozen_account(account: &AccountRef) -> Value {
    serde_json::to_value(account).unwrap_or(Value::Null)
}

/// "Create transaction" parameters of a template, fixed amount.
fn frozen_action_params(spec: &AutomationSpec) -> Map<String, Value> {
    let template = &spec.template;
    let (kind, from, to, asset, meta, amount) = match template {
        TransactionDraft::Topup(inner) => (
            "topup",
            frozen_account(&AccountRef::Store(inner.store_id.clone())),
            inner.from.as_ref().map_or(Value::Null, frozen_account),
            &inner.asset,
            &inner.meta,
            inner.amount,
        ),
        TransactionDraft::Payment(inner) => (
            "payment",
            frozen_account(&AccountRef::Store(inner.store_id.clone())),
            inner.to.as_ref().map_or(Value::Null, frozen_account),
            &inner.asset,
            &inner.meta,
            inner.amount,
        ),
        TransactionDraft::Transfer(inner) => (
            "transfer",
            frozen_account(&inner.from),
            frozen_account(&inner.to),
            &inner.asset,
            &inner.meta,
            inner.amount,
        ),
        TransactionDraft::Convert(inner) => (
            "convert",
            frozen_account(&inner.from.account),
            frozen_account(&inner.to.account),
            &inner.from.asset,
            &inner.meta,
            inner.from.amount,
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
    params.insert("amount".into(), json!(amount.to_string()));
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
    params.insert("estimated".into(), json!(false));
    params.insert("on_shortfall".into(), json!("reject"));
    params
}

fn run_of(run: AutomationRunV2) -> WorkflowRun {
    let (outcome, created, status) = match run.outcome {
        AutomationOutcome::Created { tx_id } => (RunOutcome::Done, vec![tx_id], StepStatus::Ok),
        AutomationOutcome::Rejected { reason } => (
            RunOutcome::Rejected {
                reason,
                trigger_node: TRIGGER.into(),
                planned_at: Some(run.planned_at.clone()),
            },
            Vec::new(),
            StepStatus::Error,
        ),
        AutomationOutcome::Skipped => (
            RunOutcome::Dropped {
                reason: "skipped".into(),
            },
            Vec::new(),
            StepStatus::Skipped,
        ),
    };
    let text = match &outcome {
        RunOutcome::Rejected {
            reason: RejectReason::Invalid { message },
            ..
        } => message.clone(),
        RunOutcome::Rejected { reason, .. } => describe_reject(reason),
        _ => String::new(),
    };
    WorkflowRun {
        id: run.id,
        workflow_id: run.automation_id,
        trigger: format!("schedule {}", run.planned_at),
        started_at: run.processed_at,
        outcome,
        created,
        queued: 0,
        trace: vec![TraceStep {
            node_id: ACTION.into(),
            kind: "action.transaction".into(),
            status,
            text_ru: text.clone(),
            text_en: text,
            values: Default::default(),
        }],
        dry_run: false,
        planned_date: None,
    }
}

/// Where v2 sits in the chain (`storage::chain`): one step up is v3 (the
/// `From` above builds the document this step hands over).
impl super::chain::Legacy for VaultDocV2 {
    type Next = super::v3::VaultDocV3;
    const VERSION: u32 = 2;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Amount;
    use crate::model::store::AssetRef;
    use crate::model::transaction::{KindDetails, PaymentTx, PriceStamp, TxMeta};

    fn spec_with(schedule: Schedule) -> AutomationSpec {
        let bill = PaymentTx {
            meta: TxMeta {
                id: "bill".into(),
                timestamp: "2026-01-01T00:00:00Z".into(),
                comment: "rent".into(),
                categories: vec!["rent_mortgage".into()],
                ..TxMeta::default()
            },
            store_id: "store".into(),
            asset: AssetRef::fiat("USD", "US Dollar"),
            amount: Amount::parse("10").unwrap(),
            fee: None,
            to: None,
            details: KindDetails::default(),
            price: PriceStamp::default(),
        };
        AutomationSpec {
            time: None,
            asks: false,
            for_month_offset: None,
            preset: None,
            schedule,
            starts_at: "2026-01-01T00:00:00Z".into(),
            template: TransactionDraft::Payment(bill),
            amount_mode: AmountMode::Fixed,
            workday_shift: WorkdayShift::None,
            property_id: None,
        }
    }

    #[test]
    fn v2_automation_graph_matches_the_live_builder() {
        for schedule in [
            Schedule::Monthly { day: 5 },
            Schedule::Weekly { weekday: 2 },
        ] {
            let spec = spec_with(schedule);
            let live = crate::workflow::automation::automation_graph(&spec);
            assert_eq!(frozen_graph(&spec), serde_json::to_string(&live).unwrap());
            assert_eq!(
                start_mark(&spec),
                crate::workflow::automation::start_mark(&spec)
            );
        }
    }
}
