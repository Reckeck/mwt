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
use crate::model::planning::Reminder;
use crate::model::transaction::Transaction;
use crate::service::UnlockedVault;

use super::catalog::{NodeDef, node_def};
use super::graph::{Graph, Node};
use super::model::{QueuedActions, RunOutcome, WaitKind, WorkflowRun, WorkflowWait};
use super::types::{PortType, WfValue};

/// Most nodes one run executes.
pub const MAX_STEPS: usize = 500;
/// Most operation-making nodes one run executes.
pub const MAX_ACTIONS: usize = 20;
/// Most items «Для каждого» goes through.
pub const MAX_ITEMS: usize = 200;
/// How deep «Вызвать схему» may nest.
pub const MAX_CALL_DEPTH: u32 = 5;

/// Nodes that change what data nodes read (variables, balances): the values
/// computed before them are worked out again after them.
const WRITES: [&str; 4] = [
    "var.set",
    "var.add",
    "action.transaction",
    "action.distribute",
];

/// Nodes that may stand apart from the flow: with no wire into «Вход» they
/// still run, once, after the rest of the run.
pub const FREE_RUNNING: [&str; 5] = [
    "var.set",
    "var.add",
    "action.notify",
    "action.reminder",
    "action.emit",
];

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
    /// The output it goes on from: «Дальше», or «Да»/«Нет» after a question.
    pub port: String,
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
    /// How many «Вызвать схему» deep this run is.
    pub call_depth: u32,
    /// Emulation only: answers already given to «Ждать подтверждения»
    /// nodes, by node id. A question without an answer pauses the emulation.
    pub answers: BTreeMap<String, bool>,
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
            call_depth: 0,
            answers: BTreeMap::new(),
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
    /// Reminders it set (or, in emulation, would set).
    #[serde(default)]
    pub reminders: Vec<Reminder>,
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
    /// «Вернуть значение»: a called workflow ends with this value.
    Return(Box<WfValue>),
    /// «Прервать цикл»: the nearest loop goes on to its «Готово».
    Break,
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

impl From<&str> for Fault {
    fn from(message: &str) -> Self {
        Self::Error(message.to_string())
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
    /// Values data nodes had before they were worked out again (after a
    /// write, on a loop pass): the editor still shows them on the wires.
    shown: HashMap<(String, String), WfValue>,
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
    pub reminders: Vec<Reminder>,
    trace: Vec<TraceStep>,
}

/// Run a workflow on `vault`. A live run that fails leaves the vault as it was.
pub fn run(vault: &mut UnlockedVault, graph: &Graph, ctx: &RunContext) -> RunReport {
    let backup = (ctx.mode == RunMode::Live).then(|| vault.doc.clone());
    let started_at = crate::calendar::iso(ctx.now);
    let mut exec = Executor::new(vault, graph, ctx);
    let result = exec.start();
    let edge_values = exec.edge_values();
    let mut wait = None;
    let outcome = match result {
        Ok(()) => RunOutcome::Done,
        Err(Halt::Return(_) | Halt::Break) => RunOutcome::Done,
        Err(Halt::Stop(reason)) => RunOutcome::Stopped { reason },
        Err(Halt::Fail(message)) => RunOutcome::Failed { message },
        Err(Halt::Wait(asleep)) => {
            let outcome = match asleep.kind {
                WaitKind::Timer => RunOutcome::Waiting {
                    until: asleep.resume_at.clone(),
                },
                WaitKind::Confirm => RunOutcome::Asking {
                    node_id: asleep.node_id.clone(),
                    question: asleep.question.clone(),
                    until: Some(asleep.resume_at.clone()).filter(|until| !until.is_empty()),
                },
            };
            wait = Some(*asleep);
            outcome
        }
    };
    let failed = matches!(outcome, RunOutcome::Failed { .. });
    let Executor {
        created,
        queued,
        notices,
        events,
        reminders,
        trace,
        ..
    } = exec;
    if failed && let Some(doc) = backup {
        vault.doc = doc;
    }
    let (created, queued, notices, events, reminders) = if failed {
        (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new())
    } else {
        (created, queued, notices, events, reminders)
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
        reminders,
        wait,
        edge_values,
    }
}

fn memo_key(node: &str, port: &str) -> (String, String) {
    (node.to_string(), port.to_string())
}

impl<'run> Executor<'run> {
    pub(crate) fn new(
        vault: &'run mut UnlockedVault,
        graph: &'run Graph,
        ctx: &'run RunContext,
    ) -> Self {
        Executor {
            vault,
            graph,
            ctx,
            memo: HashMap::new(),
            shown: HashMap::new(),
            evaluating: HashSet::new(),
            steps: 0,
            actions: 0,
            queue_mode: false,
            moved: Amount::ZERO,
            created: Vec::new(),
            queued: Vec::new(),
            notices: Vec::new(),
            events: Vec::new(),
            reminders: Vec::new(),
            trace: Vec::new(),
        }
    }

    /// The steps so far, handed over when a called workflow returns.
    pub(crate) fn take_trace(&mut self) -> Vec<TraceStep> {
        std::mem::take(&mut self.trace)
    }

    pub(crate) fn extend_trace(&mut self, steps: Vec<TraceStep>) {
        self.trace.extend(steps);
    }
}

impl Executor<'_> {
    pub(crate) fn start(&mut self) -> Result<(), Halt> {
        let ctx = self.ctx;
        if let Some(resume) = &ctx.resume {
            let saved: Vec<(String, String, WfValue)> =
                serde_json::from_str(&resume.memo).unwrap_or_default();
            for (node, port, value) in saved {
                self.memo.insert((node, port), value);
            }
            self.queue_mode = resume.queue_mode;
            let node_id = resume.node_id.clone();
            let (ru, en) = match resume.port.as_str() {
                "true" => (
                    "Ответ «Да» — запуск продолжается",
                    "Answered «Yes»: the run goes on",
                ),
                "false" => (
                    "Ответ «Нет» — запуск идёт по ветке «Нет»",
                    "Answered «No»: the run goes the «No» way",
                ),
                _ => (
                    "Ожидание закончилось, запуск продолжается",
                    "The wait is over; the run goes on",
                ),
            };
            self.push_trace(&node_id, StepStatus::Ok, ru.into(), en.into());
            let port = resume.port.clone();
            return self.follow(&node_id, &port);
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
        self.follow(&trigger, "out")?;
        self.run_free_nodes()
    }

    /// Write nodes drawn without a flow wire into them: they need no place in
    /// the chain and run once the flow is through, top to bottom on the canvas.
    fn run_free_nodes(&mut self) -> Result<(), Halt> {
        let graph = self.graph;
        let mut free: Vec<&Node> = graph
            .nodes
            .iter()
            .filter(|node| !node.disabled && FREE_RUNNING.contains(&node.kind.as_str()))
            .filter(|node| {
                graph
                    .edges
                    .iter()
                    .all(|edge| !(edge.to == node.id && edge.to_port == "in"))
            })
            .collect();
        free.sort_by(|left, right| {
            left.position
                .top
                .total_cmp(&right.position.top)
                .then(left.position.left.total_cmp(&right.position.left))
        });
        for node in free {
            self.visit(&node.id)?;
        }
        Ok(())
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
        if node.kind.starts_with("loop.") && node.kind != "loop.break" {
            return self.run_loop(node, def);
        }
        match self.run_flow_node(node, def) {
            Ok(step) => {
                if WRITES.contains(&node.kind.as_str()) {
                    // What was read before the write is stale now: a variable,
                    // a balance read after this node sees the new value.
                    self.forget_pure_data();
                }
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
                    Halt::Wait(wait) if wait.kind == WaitKind::Confirm => (
                        StepStatus::Waiting,
                        format!("«{}»: жду вашего ответа — {}", def.title.ru, wait.question),
                        format!(
                            "«{}»: waiting for your answer: {}",
                            def.title.en, wait.question
                        ),
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
                    Halt::Return(value) => (
                        StepStatus::Ok,
                        format!("«{}»: {}", def.title.ru, value.as_text()),
                        format!("«{}»: {}", def.title.en, value.as_text()),
                    ),
                    Halt::Break => (
                        StepStatus::Ok,
                        format!("«{}»: выход из цикла", def.title.ru),
                        format!("«{}»: leaving the loop", def.title.en),
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
            Err(message) => return Err(self.loop_failed(node, def, message)),
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
            if !self.loop_pass(node, index, ("item", item))? {
                break;
            }
        }
        self.forget_pure_data();
        self.follow(&node.id, "done")
    }

    fn loop_failed(&mut self, node: &Node, def: &NodeDef, message: String) -> Halt {
        self.push_trace(
            &node.id,
            StepStatus::Error,
            format!("«{}»: {message}", def.title.ru),
            format!("«{}»: {message}", def.title.en),
        );
        Halt::Fail(message)
    }

    /// One pass of a loop: its outputs set, its «each» way run. `false` when
    /// «Прервать цикл» ended the loop.
    fn loop_pass(
        &mut self,
        node: &Node,
        index: usize,
        (port, value): (&str, WfValue),
    ) -> Result<bool, Halt> {
        self.forget_pure_data();
        self.memo.insert(memo_key(&node.id, port), value);
        self.memo.insert(
            memo_key(&node.id, "index"),
            WfValue::Number(Amount::new(rust_decimal::Decimal::from(index as u64))),
        );
        match self.follow(&node.id, "each") {
            Ok(()) => Ok(true),
            Err(Halt::Break) => Ok(false),
            Err(other) => Err(other),
        }
    }

    /// «Повторить N раз», «Цикл по числам», «Пока условие да».
    fn run_loop(&mut self, node: &Node, def: &NodeDef) -> Result<(), Halt> {
        let number = |value: i64| WfValue::Number(Amount::new(rust_decimal::Decimal::from(value)));
        match node.kind.as_str() {
            "loop.repeat" => {
                let count = match self.number_input(node, "count") {
                    Ok(count) => count.unwrap_or(Amount::ZERO),
                    Err(message) => return Err(self.loop_failed(node, def, message)),
                };
                let count = count
                    .into_inner()
                    .trunc()
                    .to_string()
                    .parse::<i64>()
                    .unwrap_or(0)
                    .max(0);
                let taken = count.min(MAX_ITEMS as i64);
                self.push_trace(
                    &node.id,
                    StepStatus::Ok,
                    format!("«{}»: {taken} раз", def.title.ru),
                    format!("«{}»: {taken} times", def.title.en),
                );
                for index in 0..taken {
                    if !self.loop_pass(node, index as usize, ("value", number(index + 1)))? {
                        break;
                    }
                }
            }
            "loop.range" => {
                let read = |exec: &mut Self,
                            name: &str,
                            fallback: i64|
                 -> Result<rust_decimal::Decimal, String> {
                    Ok(exec
                        .number_input(node, name)?
                        .map_or(rust_decimal::Decimal::from(fallback), Amount::into_inner))
                };
                let bounds = read(self, "from", 1)
                    .and_then(|from| Ok((from, read(self, "to", 10)?, read(self, "step", 1)?)));
                let (from, to, step) = match bounds {
                    Ok(bounds) => bounds,
                    Err(message) => return Err(self.loop_failed(node, def, message)),
                };
                if step.is_zero() {
                    return Err(self.loop_failed(node, def, "the step cannot be 0".into()));
                }
                let mut values = Vec::new();
                let mut current = from;
                while (step.is_sign_positive() && current <= to)
                    || (step.is_sign_negative() && current >= to)
                {
                    if values.len() >= MAX_ITEMS {
                        break;
                    }
                    values.push(current);
                    current += step;
                }
                self.push_trace(
                    &node.id,
                    StepStatus::Ok,
                    format!(
                        "«{}»: {from}…{to}, шаг {step} — {} значений",
                        def.title.ru,
                        values.len()
                    ),
                    format!(
                        "«{}»: {from}…{to} by {step}: {} values",
                        def.title.en,
                        values.len()
                    ),
                );
                for (index, value) in values.into_iter().enumerate() {
                    if !self.loop_pass(
                        node,
                        index,
                        ("value", WfValue::Number(Amount::new(value))),
                    )? {
                        break;
                    }
                }
            }
            _ => {
                let limit = super::nodes::json_number(&self.raw_param(node, "max"))
                    .and_then(|max| max.into_inner().trunc().to_string().parse::<usize>().ok())
                    .unwrap_or(50)
                    .clamp(1, MAX_ITEMS);
                let mut passes = 0;
                loop {
                    self.forget_pure_data();
                    let going = match self.input(node, "condition") {
                        Ok(value) => value.as_bool().unwrap_or(true),
                        Err(message) => return Err(self.loop_failed(node, def, message)),
                    };
                    if !going {
                        break;
                    }
                    if passes >= limit {
                        self.push_trace(
                            &node.id,
                            StepStatus::Stopped,
                            format!("«{}»: {limit} раз — больше нельзя, выхожу", def.title.ru),
                            format!(
                                "«{}»: {limit} times, the most allowed: leaving",
                                def.title.en
                            ),
                        );
                        break;
                    }
                    if !self.loop_pass(node, passes, ("value", number(passes as i64 + 1)))? {
                        break;
                    }
                    passes += 1;
                }
                self.push_trace(
                    &node.id,
                    StepStatus::Ok,
                    format!("«{}»: повторено {passes} раз", def.title.ru),
                    format!("«{}»: repeated {passes} times", def.title.en),
                );
            }
        }
        self.forget_pure_data();
        self.follow(&node.id, "done")
    }

    /// Drop what pure data nodes computed, so they see the next list item.
    fn forget_pure_data(&mut self) {
        let graph = self.graph;
        let shown = &mut self.shown;
        self.memo.retain(|key, value| {
            let keep = graph
                .node(&key.0)
                .and_then(|node| node_def(&node.kind))
                .is_some_and(NodeDef::has_flow);
            if !keep {
                shown.insert(key.clone(), value.clone());
            }
            keep
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
                let key = memo_key(&edge.from, &edge.from_port);
                let value = self.memo.get(&key).or_else(|| self.shown.get(&key))?;
                Some((edge.id.clone(), value.as_text()))
            })
            .collect()
    }
}
