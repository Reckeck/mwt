//! Runs a workflow graph: from the trigger along the flow wires, working out
//! data inputs on demand.
//!
//! - A flow output leads to its targets in the order the wires were drawn.
//! - A data input is computed when a node asks for it, once per run (a pure
//!   data node is computed again on each pass of «Для каждого»).
//! - A wire into an input wins over the field of the same name.
//! - A failing node takes its «Ошибка» way if it has one wired; otherwise the
//!   whole run fails and nothing it did is kept.
//! - Limits keep a mistake from running away: steps, actions, list items.

use std::collections::{BTreeMap, HashMap, HashSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::Amount;
use crate::model::notify::NoticeRecord;
use crate::model::transaction::Transaction;
use crate::service::UnlockedVault;

use super::catalog::{NodeDef, node_def};
use super::graph::{Graph, Node};
use super::model::{QueuedActions, RunOutcome, WorkflowRun, WorkflowWait};
use super::types::{PortType, WfValue};

/// Most nodes one run executes.
pub const MAX_STEPS: usize = 500;
/// Most operation-making nodes one run executes.
pub const MAX_ACTIONS: usize = 20;
/// Most items «Для каждого» goes through.
pub const MAX_ITEMS: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum RunMode {
    /// For real: operations are made, notices shown.
    Live,
    /// Emulation on a copy of the vault: nothing is kept.
    DryRun,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Ok,
    /// Switched off, or let nothing through.
    Skipped,
    /// Its operations went to the confirmation queue.
    Queued,
    /// Failed; the run went on by the «Ошибка» way or stopped.
    Error,
    Stopped,
    Waiting,
}

/// One node of a run, in words.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct TraceStep {
    pub node_id: String,
    pub kind: String,
    pub status: StepStatus,
    pub text_ru: String,
    pub text_en: String,
    /// Output port → its value as text.
    pub values: BTreeMap<String, String>,
}

/// A run asleep to go on later.
#[derive(Debug, Clone, PartialEq)]
pub struct Resume {
    pub node_id: String,
    pub memo: String,
    pub queue_mode: bool,
}

/// What a run starts from.
#[derive(Debug, Clone, PartialEq)]
pub struct RunContext {
    pub workflow_id: String,
    pub workflow_name: String,
    pub run_id: String,
    pub trigger_node: String,
    /// What fired it, for the journal: `schedule 2026-11-01T09:00:00Z`.
    pub trigger_label: String,
    /// The trigger's outputs.
    pub trigger_values: BTreeMap<String, WfValue>,
    pub now: i64,
    pub zone: String,
    pub mode: RunMode,
    /// Most one run may move, in the base currency.
    pub max_amount: Option<Amount>,
    pub resume: Option<Resume>,
}

impl RunContext {
    pub fn new(
        workflow_id: &str,
        workflow_name: &str,
        trigger_node: &str,
        now: i64,
        zone: &str,
    ) -> Self {
        Self {
            workflow_id: workflow_id.to_string(),
            workflow_name: workflow_name.to_string(),
            run_id: uuid::Uuid::new_v4().to_string(),
            trigger_node: trigger_node.to_string(),
            trigger_label: String::new(),
            trigger_values: BTreeMap::new(),
            now,
            zone: zone.to_string(),
            mode: RunMode::Live,
            max_amount: None,
            resume: None,
        }
    }
}

/// An event a run sent with «Отправить событие».
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct EmittedEvent {
    pub name: String,
    pub payload: WfValue,
}

/// Everything a run did. The vault already holds its operations; the caller
/// keeps the run, the queue, the notices and the wait.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct RunReport {
    pub run: WorkflowRun,
    /// Operations made (or, in emulation, that would be made).
    pub transactions: Vec<Transaction>,
    pub queued: Vec<QueuedActions>,
    pub notices: Vec<NoticeRecord>,
    pub events: Vec<EmittedEvent>,
    #[ts(skip)]
    #[serde(skip)]
    pub wait: Option<WorkflowWait>,
    /// Data wire id → the value that went along it, for the editor.
    pub edge_values: BTreeMap<String, String>,
}

/// Why a run ends early.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Halt {
    Stop(String),
    Fail(String),
    Wait(Box<WorkflowWait>),
}

/// How a node can fail.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Fault {
    /// The node's own failure: the «Ошибка» way, if it has one.
    Error(String),
    /// The run ends.
    Halt(Halt),
}

impl From<String> for Fault {
    fn from(message: String) -> Self {
        Self::Error(message)
    }
}

impl From<Halt> for Fault {
    fn from(halt: Halt) -> Self {
        Self::Halt(halt)
    }
}

/// What a flow node did: where to go next and how to say it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Step {
    pub next: Vec<&'static str>,
    pub status: StepStatus,
    pub ru: String,
    pub en: String,
}

impl Step {
    pub fn next(port: &'static str, ru: impl Into<String>, en: impl Into<String>) -> Self {
        Self {
            next: vec![port],
            status: StepStatus::Ok,
            ru: ru.into(),
            en: en.into(),
        }
    }

    pub fn with_status(mut self, status: StepStatus) -> Self {
        self.status = status;
        self
    }
}

pub(crate) struct Executor<'run> {
    pub vault: &'run mut UnlockedVault,
    pub graph: &'run Graph,
    pub ctx: &'run RunContext,
    memo: HashMap<(String, String), WfValue>,
    evaluating: HashSet<String>,
    steps: usize,
    pub actions: usize,
    pub queue_mode: bool,
    /// Value moved so far, in the base currency, against `max_amount`.
    pub moved: Amount,
    pub created: Vec<Transaction>,
    pub queued: Vec<QueuedActions>,
    pub notices: Vec<NoticeRecord>,
    pub events: Vec<EmittedEvent>,
    trace: Vec<TraceStep>,
}

/// Run a workflow on `vault`. A live run that fails leaves the vault as it was.
pub fn run(vault: &mut UnlockedVault, graph: &Graph, ctx: &RunContext) -> RunReport {
    let backup = (ctx.mode == RunMode::Live).then(|| vault.doc.clone());
    let started_at = crate::calendar::iso(ctx.now);
    let mut exec = Executor {
        vault,
        graph,
        ctx,
        memo: HashMap::new(),
        evaluating: HashSet::new(),
        steps: 0,
        actions: 0,
        queue_mode: false,
        moved: Amount::ZERO,
        created: Vec::new(),
        queued: Vec::new(),
        notices: Vec::new(),
        events: Vec::new(),
        trace: Vec::new(),
    };
    let result = exec.start();
    let edge_values = exec.edge_values();
    let mut wait = None;
    let outcome = match result {
        Ok(()) => RunOutcome::Done,
        Err(Halt::Stop(reason)) => RunOutcome::Stopped { reason },
        Err(Halt::Fail(message)) => RunOutcome::Failed { message },
        Err(Halt::Wait(asleep)) => {
            let until = asleep.resume_at.clone();
            wait = Some(*asleep);
            RunOutcome::Waiting { until }
        }
    };
    let failed = matches!(outcome, RunOutcome::Failed { .. });
    let Executor {
        created,
        queued,
        notices,
        events,
        trace,
        ..
    } = exec;
    if failed && let Some(doc) = backup {
        vault.doc = doc;
    }
    let (created, queued, notices, events) = if failed {
        (Vec::new(), Vec::new(), Vec::new(), Vec::new())
    } else {
        (created, queued, notices, events)
    };
    RunReport {
        run: WorkflowRun {
            id: ctx.run_id.clone(),
            workflow_id: ctx.workflow_id.clone(),
            trigger: ctx.trigger_label.clone(),
            started_at,
            outcome,
            created: created.iter().map(|tx| tx.meta().id.clone()).collect(),
            queued: queued
                .iter()
                .map(|batch| batch.transactions.len() as u32)
                .sum(),
            trace,
            dry_run: ctx.mode == RunMode::DryRun,
        },
        transactions: created,
        queued,
        notices,
        events,
        wait,
        edge_values,
    }
}

fn memo_key(node: &str, port: &str) -> (String, String) {
    (node.to_string(), port.to_string())
}

impl Executor<'_> {
    fn start(&mut self) -> Result<(), Halt> {
        let ctx = self.ctx;
        if let Some(resume) = &ctx.resume {
            let saved: Vec<(String, String, WfValue)> =
                serde_json::from_str(&resume.memo).unwrap_or_default();
            for (node, port, value) in saved {
                self.memo.insert((node, port), value);
            }
            self.queue_mode = resume.queue_mode;
            let node_id = resume.node_id.clone();
            self.push_trace(
                &node_id,
                StepStatus::Ok,
                "Ожидание закончилось, запуск продолжается".into(),
                "The wait is over; the run goes on".into(),
            );
            return self.follow(&node_id, "out");
        }
        let trigger = ctx.trigger_node.clone();
        let Some(node) = self.graph.node(&trigger) else {
            return Err(Halt::Fail(format!("trigger {trigger} is not in the graph")));
        };
        let title = node_def(&node.kind).map(|def| def.title);
        for (port, value) in &ctx.trigger_values {
            self.memo.insert(memo_key(&trigger, port), value.clone());
        }
        let (ru, en) = match title {
            Some(title) => (
                format!("«{}» сработал", title.ru),
                format!("«{}» fired", title.en),
            ),
            None => ("Запуск".into(), "Run".into()),
        };
        self.push_trace(&trigger, StepStatus::Ok, ru, en);
        self.follow(&trigger, "out")
    }

    fn push_trace(&mut self, node_id: &str, status: StepStatus, ru: String, en: String) {
        let kind = self
            .graph
            .node(node_id)
            .map(|node| node.kind.clone())
            .unwrap_or_default();
        let values = self
            .memo
            .iter()
            .filter(|((node, _), _)| node == node_id)
            .map(|((_, port), value)| (port.clone(), value.as_text()))
            .collect();
        self.trace.push(TraceStep {
            node_id: node_id.to_string(),
            kind,
            status,
            text_ru: ru,
            text_en: en,
            values,
        });
    }

    /// Run every node a flow output leads to, in the order drawn.
    pub(crate) fn follow(&mut self, node_id: &str, port: &str) -> Result<(), Halt> {
        let targets: Vec<String> = self
            .graph
            .edges_from(node_id, port)
            .map(|edge| edge.to.clone())
            .collect();
        for target in targets {
            self.visit(&target)?;
        }
        Ok(())
    }

    fn count_step(&mut self) -> Result<(), Halt> {
        self.steps += 1;
        if self.steps > MAX_STEPS {
            return Err(Halt::Fail(format!(
                "more than {MAX_STEPS} steps in one run: check the loops"
            )));
        }
        Ok(())
    }

    fn visit(&mut self, node_id: &str) -> Result<(), Halt> {
        self.count_step()?;
        let graph = self.graph;
        let Some(node) = graph.node(node_id) else {
            return Ok(());
        };
        let Some(def) = node_def(&node.kind) else {
            return Err(Halt::Fail(format!("unknown node {}", node.kind)));
        };
        if node.disabled {
            self.push_trace(
                node_id,
                StepStatus::Skipped,
                format!("«{}» выключен — пропущен", def.title.ru),
                format!("«{}» is off: skipped", def.title.en),
            );
            return self.follow(node_id, "out");
        }
        if node.kind == "list.for_each" {
            return self.for_each(node, def);
        }
        match self.run_flow_node(node, def) {
            Ok(step) => {
                self.push_trace(node_id, step.status, step.ru, step.en);
                for port in step.next {
                    self.follow(node_id, port)?;
                }
                Ok(())
            }
            Err(Fault::Error(message)) => {
                let has_error_way = def.output("error").is_some()
                    && graph.edges_from(node_id, "error").next().is_some();
                if has_error_way {
                    self.memo
                        .insert(memo_key(node_id, "reason"), WfValue::Text(message.clone()));
                    self.push_trace(
                        node_id,
                        StepStatus::Error,
                        format!("«{}»: {message} — дальше по ветке «Ошибка»", def.title.ru),
                        format!("«{}»: {message}; going the «Error» way", def.title.en),
                    );
                    return self.follow(node_id, "error");
                }
                self.push_trace(
                    node_id,
                    StepStatus::Error,
                    format!("«{}»: {message}", def.title.ru),
                    format!("«{}»: {message}", def.title.en),
                );
                Err(Halt::Fail(format!("{}: {message}", def.title.en)))
            }
            Err(Fault::Halt(halt)) => {
                let (status, ru, en) = match &halt {
                    Halt::Stop(reason) => (
                        StepStatus::Stopped,
                        format!("«{}»: остановка — {reason}", def.title.ru),
                        format!("«{}»: stopped: {reason}", def.title.en),
                    ),
                    Halt::Wait(wait) => (
                        StepStatus::Waiting,
                        format!("«{}»: продолжу {}", def.title.ru, wait.resume_at),
                        format!("«{}»: goes on at {}", def.title.en, wait.resume_at),
                    ),
                    Halt::Fail(message) => (
                        StepStatus::Error,
                        format!("«{}»: {message}", def.title.ru),
                        format!("«{}»: {message}", def.title.en),
                    ),
                };
                self.push_trace(node_id, status, ru, en);
                Err(halt)
            }
        }
    }

    fn for_each(&mut self, node: &Node, def: &NodeDef) -> Result<(), Halt> {
        let items = match self.input(node, "items") {
            Ok(WfValue::List(items)) => items,
            Ok(WfValue::Null) => Vec::new(),
            Ok(other) => vec![other],
            Err(message) => {
                self.push_trace(
                    &node.id,
                    StepStatus::Error,
                    format!("«{}»: {message}", def.title.ru),
                    format!("«{}»: {message}", def.title.en),
                );
                return Err(Halt::Fail(message));
            }
        };
        let total = items.len();
        let taken = total.min(MAX_ITEMS);
        let (ru, en) = if total > MAX_ITEMS {
            (
                format!(
                    "«{}»: {total} элементов, беру первые {MAX_ITEMS}",
                    def.title.ru
                ),
                format!(
                    "«{}»: {total} items, taking the first {MAX_ITEMS}",
                    def.title.en
                ),
            )
        } else {
            (
                format!("«{}»: {total} элементов", def.title.ru),
                format!("«{}»: {total} items", def.title.en),
            )
        };
        self.push_trace(&node.id, StepStatus::Ok, ru, en);
        for (index, item) in items.into_iter().take(taken).enumerate() {
            self.forget_pure_data();
            self.memo.insert(memo_key(&node.id, "item"), item);
            self.memo.insert(
                memo_key(&node.id, "index"),
                WfValue::Number(Amount::new(rust_decimal::Decimal::from(index as u64))),
            );
            self.follow(&node.id, "each")?;
        }
        self.forget_pure_data();
        self.follow(&node.id, "done")
    }

    /// Drop what pure data nodes computed, so they see the next list item.
    fn forget_pure_data(&mut self) {
        let graph = self.graph;
        self.memo.retain(|(node, _), _| {
            graph
                .node(node)
                .and_then(|node| node_def(&node.kind))
                .is_some_and(NodeDef::has_flow)
        });
    }

    /// The value of a node's input: what its wire brings, else its field.
    pub(crate) fn input(&mut self, node: &Node, name: &str) -> Result<WfValue, String> {
        if let Some(edge) = self.graph.edge_into(&node.id, name) {
            let (from, port) = (edge.from.clone(), edge.from_port.clone());
            return self.output_of(&from, &port);
        }
        Ok(self.param(node, name))
    }

    /// Whether an input has a wire.
    pub(crate) fn wired(&self, node: &Node, name: &str) -> bool {
        self.graph.edge_into(&node.id, name).is_some()
    }

    /// A node's output: remembered, or worked out now for a data node. A flow
    /// node that has not run yet gives nothing.
    pub(crate) fn output_of(&mut self, node_id: &str, port: &str) -> Result<WfValue, String> {
        let key = memo_key(node_id, port);
        if let Some(value) = self.memo.get(&key) {
            return Ok(value.clone());
        }
        let graph = self.graph;
        let Some(node) = graph.node(node_id) else {
            return Ok(WfValue::Null);
        };
        let Some(def) = node_def(&node.kind) else {
            return Err(format!("unknown node {}", node.kind));
        };
        if def.has_flow() || node.disabled {
            return Ok(WfValue::Null);
        }
        self.steps += 1;
        if self.steps > MAX_STEPS {
            return Err(format!("more than {MAX_STEPS} steps in one run"));
        }
        if !self.evaluating.insert(node_id.to_string()) {
            return Err("data depends on itself".into());
        }
        let result = self.eval_data_node(node, def);
        self.evaluating.remove(node_id);
        for (out_port, value) in result? {
            self.memo.insert(memo_key(node_id, out_port), value);
        }
        Ok(self.memo.get(&key).cloned().unwrap_or_default())
    }

    /// Set a flow node's output for the nodes after it.
    pub(crate) fn set_output(&mut self, node: &Node, port: &str, value: WfValue) {
        self.memo.insert(memo_key(&node.id, port), value);
    }

    /// Node outputs so far, as JSON, for a run that sleeps.
    pub(crate) fn memo_json(&self) -> String {
        let saved: Vec<(&String, &String, &WfValue)> = self
            .memo
            .iter()
            .map(|((node, port), value)| (node, port, value))
            .collect();
        serde_json::to_string(&saved).unwrap_or_default()
    }

    /// What went along each data wire.
    fn edge_values(&self) -> BTreeMap<String, String> {
        self.graph
            .edges
            .iter()
            .filter_map(|edge| {
                let def = node_def(&self.graph.node(&edge.from)?.kind)?;
                if def.output(&edge.from_port)?.ty == PortType::Flow {
                    return None;
                }
                let value = self.memo.get(&memo_key(&edge.from, &edge.from_port))?;
                Some((edge.id.clone(), value.as_text()))
            })
            .collect()
    }
}
