//! Node workflows (plan §16) in the vault: saving them with their versions,
//! firing their triggers, the confirmation queue, variables, the journal, and
//! the vault lookups the executor (`workflow::exec`) runs on.

use std::collections::BTreeMap;
use std::ops::Not;

use chrono::{Datelike, Duration};

use crate::Amount;
use crate::calendar::{Calendar, first_of_month};
use crate::error::{Error, Result};
use crate::model::notify::NoticeRecord;
use crate::model::planning::RejectReason;
use crate::model::store::AssetRef;
use crate::model::transaction::Transaction;
use crate::model::vault::now_iso;
use crate::pricing::history::{secs_or_now, unix_secs};
use crate::workflow::exec::{
    self, EmittedEvent, Resume, RunContext, RunMode, RunReport, StepStatus, TraceStep,
};
use crate::workflow::graph::Graph;
use crate::workflow::model::{
    GraphVersion, MAX_GRAPH_VERSIONS, MAX_WORKFLOW_RUNS, QueuedActions, RunOutcome, TestExpect,
    VarScope, WaitKind, Workflow, WorkflowRun, WorkflowState, WorkflowTest, WorkflowVar,
    WorkflowWait,
};
use crate::workflow::nodes::triggers::{schedule_moments, tx_matches, tx_trigger_values};
use crate::workflow::types::WfValue;
use crate::workflow::validate::{GraphIssue, has_errors, validate};
use crate::workflow::{catalog::node_def, graph::Node};

use super::UnlockedVault;
use super::automations::outflow;
use super::worth::FlowClass;

/// Most live runs of one workflow in an hour; more look like a loop.
pub const MAX_RUNS_PER_HOUR: usize = 60;
/// How deep events may chain one workflow into another.
pub const MAX_EVENT_DEPTH: u32 = 5;
/// The format name of an exported workflow.
pub const EXPORT_FORMAT: &str = "trade-app-workflow";

/// What the user edits of a workflow.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct WorkflowDraft {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub active: bool,
    pub graph: String,
    #[serde(default)]
    pub react_to_workflows: bool,
    #[serde(default)]
    pub max_amount: Option<Amount>,
}

/// A saved workflow with what is wrong with its graph.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct SavedWorkflow {
    pub workflow: Workflow,
    pub issues: Vec<GraphIssue>,
}

/// What one scheduler pass did.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct WorkflowPass {
    pub runs: Vec<WorkflowRun>,
    /// Operations made.
    pub created: Vec<String>,
    /// Operations put into the confirmation queue.
    pub queued: u32,
    /// New notices to show.
    pub notices: Vec<NoticeRecord>,
}

/// How a test went.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct WorkflowTestResult {
    pub test_id: String,
    pub name: String,
    pub passed: bool,
    /// What differed, in both languages.
    pub problems_ru: Vec<String>,
    pub problems_en: Vec<String>,
    pub report: RunReport,
}

const OUTCOMES: [&str; 5] = ["done", "stopped", "failed", "waiting", "partial"];

fn outcome_word(outcome: &crate::workflow::RunOutcome) -> &'static str {
    use crate::workflow::RunOutcome::*;
    match outcome {
        Done => "done",
        Stopped { .. } | Dropped { .. } => "stopped",
        Partial { .. } => "partial",
        Failed { .. } | Rejected { .. } => "failed",
        Waiting { .. } | Asking { .. } => "waiting",
    }
}

/// What a report did that a test did not expect.
fn test_problems(expect: &TestExpect, report: &RunReport) -> (Vec<String>, Vec<String>) {
    let mut ru = Vec::new();
    let mut en = Vec::new();
    let got = outcome_word(&report.run.outcome);
    if got != expect.outcome {
        ru.push(format!("итог «{got}» вместо «{}»", expect.outcome));
        en.push(format!("outcome «{got}» instead of «{}»", expect.outcome));
    }
    let queued = report
        .queued
        .iter()
        .map(|batch| batch.transactions.len() as u32)
        .sum::<u32>();
    let counts = [
        (
            expect.transactions,
            report.transactions.len() as u32,
            "операций",
            "operations",
        ),
        (expect.queued, queued, "в очереди", "queued"),
        (
            expect.notices,
            report.notices.len() as u32,
            "уведомлений",
            "notices",
        ),
    ];
    for (wanted, actual, word_ru, word_en) in counts {
        if let Some(wanted) = wanted
            && wanted != actual
        {
            ru.push(format!("{word_ru}: {actual} вместо {wanted}"));
            en.push(format!("{word_en}: {actual} instead of {wanted}"));
        }
    }
    (ru, en)
}

/// An exported workflow: the graph and its name, without ids or history.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct ExportedWorkflow {
    format: String,
    version: u32,
    name: String,
    #[serde(default)]
    description: String,
    graph: serde_json::Value,
}

fn trigger_label(kind: &str, detail: &str) -> String {
    let short = kind.strip_prefix("trigger.").unwrap_or(kind);
    if detail.is_empty() {
        short.to_string()
    } else {
        format!("{short} {detail}")
    }
}

fn value_of(json: &str) -> WfValue {
    serde_json::from_str(json).unwrap_or_default()
}

impl UnlockedVault {
    // -- lookups for the executor ------------------------------------------------

    /// An asset by its symbol: one the vault holds, else a currency.
    pub(crate) fn wf_asset(&self, symbol: &str) -> AssetRef {
        let symbol = symbol.trim();
        self.doc
            .holdings
            .iter()
            .map(|holding| &holding.asset)
            .find(|asset| asset.symbol.eq_ignore_ascii_case(symbol) && !asset.is_derivative())
            .cloned()
            .unwrap_or_else(|| AssetRef::fiat(symbol, &symbol.to_uppercase()))
    }

    /// The balance of an account with its sub-accounts: of one asset, or the
    /// whole value in the base currency.
    pub(crate) fn wf_balance(
        &self,
        store_id: &str,
        asset: Option<&AssetRef>,
    ) -> std::result::Result<WfValue, String> {
        if self.find_store(store_id).is_none() {
            return Err(format!("account {store_id} not found"));
        }
        match asset {
            Some(asset) => {
                let key = asset.key();
                let mut ids = self.subtree_ids(store_id);
                ids.push(store_id.to_string());
                let quantity = self
                    .doc
                    .holdings
                    .iter()
                    .filter(|holding| ids.contains(&holding.store_id) && holding.asset.key() == key)
                    .fold(Amount::ZERO, |sum, holding| sum + holding.quantity);
                Ok(WfValue::money(quantity, &asset.symbol))
            }
            None => {
                let overview = self
                    .store_overview(store_id, true)
                    .map_err(|err| err.to_string())?;
                Ok(WfValue::money(
                    overview.subtree_base.round(2),
                    &overview.base_currency,
                ))
            }
        }
    }

    /// Price of one unit of an asset in a currency.
    pub(crate) fn wf_price(&self, asset: &AssetRef, currency: &str, secs: i64) -> Option<Amount> {
        let book = self.rate_book();
        let usd = book.price_at(&asset.key(), secs)?;
        book.convert(usd, "usd", currency, secs)
    }

    pub(crate) fn wf_convert(
        &self,
        amount: Amount,
        from: &str,
        to: &str,
        secs: i64,
    ) -> Option<Amount> {
        self.rate_book().convert(amount, from, to, secs)
    }

    /// Income or spending of the current period, in the base currency.
    pub(crate) fn wf_sum(
        &self,
        period: &str,
        flow: &str,
        category: Option<&str>,
        now: i64,
        zone: &str,
    ) -> Amount {
        let cal = Calendar::new(zone);
        let today = cal.date_of(now);
        let start = match period {
            "day" => today,
            "week" => today - Duration::days(i64::from(today.weekday().num_days_from_monday())),
            "year" => today.with_ordinal(1).unwrap_or(today),
            _ => first_of_month(today),
        };
        let (base, _) = self.base_and_fx();
        self.flows_between(&cal, &base, cal.start_of(start), now + 1)
            .into_iter()
            .filter(|item| match flow {
                "income" => item.class == FlowClass::Income,
                _ => matches!(item.class, FlowClass::Fixed | FlowClass::Daily),
            })
            .filter(|item| {
                category.is_none_or(|key| item.categories.iter().any(|known| known == key))
            })
            .fold(Amount::ZERO, |sum, item| sum + item.value)
            .round(2)
    }

    /// Live accounts under a wallet, or every live wallet: id, name and value
    /// in the base currency.
    /// The average amount of the latest `count` operations a workflow made
    /// (or the automation it was before, which kept its id), leaving out the
    /// ones still booked with an approximate amount; with the asset symbol.
    pub(crate) fn wf_recent_average(
        &self,
        workflow_id: &str,
        count: usize,
    ) -> Option<(Amount, String)> {
        let mut made: Vec<&Transaction> = self
            .doc
            .transactions
            .iter()
            .filter(|tx| !tx.meta().estimated && made_by(tx, workflow_id))
            .collect();
        made.sort_by(|left, right| right.meta().timestamp.cmp(&left.meta().timestamp));
        made.truncate(count.max(1));
        let first = made.first()?;
        let (_, asset, _) = crate::workflow::nodes::triggers::tx_main(first);
        let symbol = match asset {
            WfValue::Asset(asset) => asset.symbol,
            _ => String::new(),
        };
        let total = made.iter().fold(Amount::ZERO, |sum, tx| {
            sum + crate::workflow::nodes::triggers::tx_main(tx)
                .0
                .as_number()
                .unwrap_or(Amount::ZERO)
        });
        let len = Amount::new(rust_decimal::Decimal::from(made.len()));
        Some(((total / len).round(2), symbol))
    }

    pub(crate) fn wf_accounts(&self, parent: Option<&str>) -> Vec<(String, String, Amount)> {
        self.doc
            .stores
            .iter()
            .filter(|store| !store.archived && store.parent_id.as_deref() == parent)
            .map(|store| {
                let value = self
                    .store_overview(&store.id, true)
                    .map_or(Amount::ZERO, |overview| overview.subtree_base.round(2));
                (store.id.clone(), store.name.clone(), value)
            })
            .collect()
    }

    /// What an operation takes out of an account, in the base currency.
    pub(crate) fn wf_outflow_base(&self, tx: &Transaction) -> Amount {
        let Some(out) = outflow(tx) else {
            return Amount::ZERO;
        };
        let book = self.rate_book();
        let secs = secs_or_now(&tx.meta().timestamp);
        let (base, _) = self.base_and_fx();
        book.price_at(&out.asset.key(), secs)
            .and_then(|usd| book.convert(out.amount * usd, "usd", &base, secs))
            .unwrap_or(Amount::ZERO)
    }

    /// Check money and rules, then add the operations; all of them or none.
    /// Not persisted: the run's caller writes once.
    pub(crate) fn wf_insert(
        &mut self,
        txs: Vec<Transaction>,
        now: i64,
    ) -> std::result::Result<Vec<Transaction>, String> {
        let backup = self.doc.clone();
        let flow = (txs.len() > 1).then(|| uuid::Uuid::new_v4().to_string());
        let mut saved = Vec::with_capacity(txs.len());
        for mut tx in txs {
            if let Some(flow) = &flow {
                tx.meta_mut().flow_id = Some(flow.clone());
            }
            let secs = secs_or_now(&tx.meta().timestamp);
            let result = match self.funds_shortfall(&tx, secs) {
                Some(reason) => Err(super::automations::describe_reject(&reason)),
                // The run's clock, not the wall one: an emulated run days
                // ahead dates its operations days ahead too.
                None => self
                    .prepare_transaction_at(&mut tx, now.max(secs) * 1000)
                    .map_err(|err| err.to_string()),
            };
            if let Err(message) = result {
                self.doc = backup;
                return Err(message);
            }
            self.doc.transactions.push(tx.clone());
            self.recompute_holdings();
            saved.push(tx);
        }
        Ok(saved)
    }

    fn wf_state_mut(&mut self, workflow_id: &str) -> &mut WorkflowState {
        let index = match self
            .doc
            .workflow_state
            .iter()
            .position(|state| state.workflow_id == workflow_id)
        {
            Some(index) => index,
            None => {
                self.doc.workflow_state.push(WorkflowState {
                    workflow_id: workflow_id.to_string(),
                    ..WorkflowState::default()
                });
                self.doc.workflow_state.len() - 1
            }
        };
        &mut self.doc.workflow_state[index]
    }

    pub(crate) fn wf_mark(&self, workflow_id: &str, key: &str) -> Option<String> {
        self.doc
            .workflow_state
            .iter()
            .find(|state| state.workflow_id == workflow_id)
            .and_then(|state| state.marks.get(key).cloned())
    }

    pub(crate) fn wf_set_mark(&mut self, workflow_id: &str, key: &str, value: &str) {
        self.wf_state_mut(workflow_id)
            .marks
            .insert(key.to_string(), value.to_string());
    }

    pub(crate) fn wf_var(&self, scope: &VarScope, name: &str) -> Option<WfValue> {
        self.doc
            .workflow_vars
            .iter()
            .find(|var| var.scope == *scope && var.name == name)
            .map(|var| value_of(&var.value))
    }

    pub(crate) fn wf_set_var(&mut self, scope: VarScope, name: &str, value: &WfValue) {
        let json = serde_json::to_string(value).unwrap_or_default();
        let updated_at = now_iso();
        match self
            .doc
            .workflow_vars
            .iter_mut()
            .find(|var| var.scope == scope && var.name == name)
        {
            Some(var) => {
                var.value = json;
                var.updated_at = updated_at;
            }
            None => self.doc.workflow_vars.push(WorkflowVar {
                scope,
                name: name.to_string(),
                value: json,
                updated_at,
            }),
        }
    }

    // -- CRUD -----------------------------------------------------------------------

    /// Rewrite stored graphs (and their versions) in an older graph format,
    /// so the editor always gets the current one. Returns whether any changed.
    pub(crate) fn migrate_workflow_graphs(&mut self) -> bool {
        let mut changed = false;
        for workflow in &mut self.doc.workflows {
            if let Some(json) = Graph::migrate_json(&workflow.graph) {
                workflow.graph = json;
                changed = true;
            }
            for version in &mut workflow.versions {
                if let Some(json) = Graph::migrate_json(&version.graph) {
                    version.graph = json;
                    changed = true;
                }
            }
        }
        changed
    }

    /// Active workflows a required field now stops (they used to run with it
    /// empty and fail, or never fire): switched off on open, each with a line
    /// in the journal and a notice, so `run_due_workflows` does not skip them
    /// in silence. Returns whether anything changed.
    pub(crate) fn disable_incomplete_workflows(&mut self, now: i64) -> bool {
        let mut notices: Vec<NoticeRecord> = Vec::new();
        for index in 0..self.doc.workflows.len() {
            let workflow = &self.doc.workflows[index];
            if !workflow.active {
                continue;
            }
            let Ok(graph) = Graph::parse(&workflow.graph) else {
                continue;
            };
            let Some(issue) = validate(&graph)
                .into_iter()
                .find(|issue| issue.code == "param_missing")
            else {
                continue;
            };
            let (id, name) = (workflow.id.clone(), workflow.name.clone());
            self.doc.workflows[index].active = false;
            self.doc.workflow_runs.push(WorkflowRun {
                id: uuid::Uuid::new_v4().to_string(),
                workflow_id: id.clone(),
                trigger: String::new(),
                started_at: crate::calendar::iso(now),
                outcome: RunOutcome::Dropped {
                    reason: "missing_params".to_string(),
                },
                created: Vec::new(),
                queued: 0,
                trace: vec![TraceStep {
                    node_id: issue.node_id.clone().unwrap_or_default(),
                    kind: String::new(),
                    status: StepStatus::Skipped,
                    text_ru: format!("Схема выключена: {}", issue.text_ru),
                    text_en: format!("The workflow was switched off: {}", issue.text_en),
                    values: Default::default(),
                }],
                dry_run: false,
            });
            notices.push(crate::service::notice(
                crate::model::notify::NoticeCategory::Automation,
                format!("wf-off:{id}"),
                &name,
                format!("Схема выключена: {}", issue.text_ru),
                Some(format!("workflow-edit:{id}")),
                crate::calendar::iso(now),
            ));
        }
        if notices.is_empty() {
            return false;
        }
        let overflow = self
            .doc
            .workflow_runs
            .len()
            .saturating_sub(MAX_WORKFLOW_RUNS);
        self.doc.workflow_runs.drain(..overflow);
        if self.notices_enabled(crate::model::notify::NoticeCategory::Automation) {
            self.doc.notification_log.extend(notices);
        }
        true
    }

    pub fn workflows(&self) -> Vec<Workflow> {
        self.doc.workflows.clone()
    }

    pub fn workflow(&self, id: &str) -> Result<Workflow> {
        self.doc
            .workflows
            .iter()
            .find(|workflow| workflow.id == id)
            .cloned()
            .ok_or_else(|| Error::Missing(format!("workflow {id}")))
    }

    fn check_draft(draft: &WorkflowDraft) -> Result<(Graph, Vec<GraphIssue>)> {
        if draft.name.trim().is_empty() {
            return Err(Error::InvalidInput("name the workflow".into()));
        }
        if draft.max_amount.is_some_and(|cap| !cap.is_positive()) {
            return Err(Error::InvalidInput("the cap must be above zero".into()));
        }
        let graph = Graph::parse(&draft.graph)?;
        let issues = validate(&graph);
        if draft.active && has_errors(&issues) {
            let first = issues
                .first()
                .map(|issue| issue.text_en.clone())
                .unwrap_or_default();
            return Err(Error::InvalidInput(format!(
                "fix the workflow's errors before switching it on: {first}"
            )));
        }
        Ok((graph, issues))
    }

    /// Create (`id` = `None`) or update a workflow. A changed graph keeps the
    /// one before it among the versions; one switched on starts watching from now.
    pub fn save_workflow(
        &mut self,
        id: Option<&str>,
        draft: WorkflowDraft,
        now: i64,
    ) -> Result<SavedWorkflow> {
        let (graph, issues) = Self::check_draft(&draft)?;
        let graph_json = graph.to_json();
        let stamp = crate::calendar::iso(now);
        let (workflow, arm) = match id {
            None => {
                let workflow = Workflow {
                    id: uuid::Uuid::new_v4().to_string(),
                    name: draft.name.trim().to_string(),
                    description: draft.description,
                    active: draft.active,
                    graph: graph_json,
                    created_at: stamp.clone(),
                    updated_at: stamp,
                    react_to_workflows: draft.react_to_workflows,
                    max_amount: draft.max_amount,
                    versions: Vec::new(),
                    tests: Vec::new(),
                    draft: None,
                };
                self.doc.workflows.push(workflow.clone());
                let arm = workflow.active;
                (workflow, arm)
            }
            Some(id) => {
                let workflow = self
                    .doc
                    .workflows
                    .iter_mut()
                    .find(|workflow| workflow.id == id)
                    .ok_or_else(|| Error::Missing(format!("workflow {id}")))?;
                let arm = draft.active && !workflow.active;
                if workflow.graph != graph_json && !workflow.graph.is_empty() {
                    workflow.versions.push(GraphVersion {
                        saved_at: workflow.updated_at.clone(),
                        graph: std::mem::take(&mut workflow.graph),
                    });
                    let overflow = workflow.versions.len().saturating_sub(MAX_GRAPH_VERSIONS);
                    workflow.versions.drain(..overflow);
                }
                workflow.name = draft.name.trim().to_string();
                workflow.description = draft.description;
                // Saved for real: the edits kept aside are in it now.
                workflow.draft = None;
                workflow.active = draft.active;
                workflow.graph = graph_json;
                workflow.updated_at = stamp;
                workflow.react_to_workflows = draft.react_to_workflows;
                workflow.max_amount = draft.max_amount;
                (workflow.clone(), arm)
            }
        };
        if arm {
            self.arm_workflow(&workflow.id, &graph, now);
        }
        self.persist()?;
        Ok(SavedWorkflow { workflow, issues })
    }

    /// Start watching from now: schedules count from this moment, operations
    /// from the next one added.
    fn arm_workflow(&mut self, workflow_id: &str, graph: &Graph, now: i64) {
        let stamp = crate::calendar::iso(now);
        let (index, cursor) = (
            self.doc.transactions.len(),
            self.doc.transactions.last().map(|tx| tx.meta().id.clone()),
        );
        let state = self.wf_state_mut(workflow_id);
        state.tx_index = index;
        state.tx_cursor = cursor;
        for node in graph
            .nodes
            .iter()
            .filter(|node| node.kind == "trigger.schedule")
        {
            state
                .marks
                .insert(format!("schedule:{}", node.id), stamp.clone());
        }
    }

    /// Keep the editor's unsaved graph aside: the saved one keeps running.
    pub fn save_workflow_draft(
        &mut self,
        id: &str,
        graph_json: &str,
        now: i64,
    ) -> Result<Workflow> {
        let graph = Graph::parse(graph_json)?;
        let workflow = self
            .doc
            .workflows
            .iter_mut()
            .find(|workflow| workflow.id == id)
            .ok_or_else(|| Error::Missing(format!("workflow {id}")))?;
        workflow.draft = Some(GraphVersion {
            saved_at: crate::calendar::iso(now),
            graph: graph.to_json(),
        });
        let out = workflow.clone();
        self.persist()?;
        Ok(out)
    }

    /// Throw the kept-aside edits away.
    pub fn discard_workflow_draft(&mut self, id: &str) -> Result<Workflow> {
        let workflow = self
            .doc
            .workflows
            .iter_mut()
            .find(|workflow| workflow.id == id)
            .ok_or_else(|| Error::Missing(format!("workflow {id}")))?;
        workflow.draft = None;
        let out = workflow.clone();
        self.persist()?;
        Ok(out)
    }

    pub fn set_workflow_active(&mut self, id: &str, active: bool, now: i64) -> Result<Workflow> {
        let workflow = self.workflow(id)?;
        let graph = Graph::parse(&workflow.graph)?;
        if active && has_errors(&validate(&graph)) {
            return Err(Error::InvalidInput(
                "fix the workflow's errors before switching it on".into(),
            ));
        }
        if active && !workflow.active {
            self.arm_workflow(id, &graph, now);
        }
        let stored = self
            .doc
            .workflows
            .iter_mut()
            .find(|workflow| workflow.id == id)
            .ok_or_else(|| Error::Missing(format!("workflow {id}")))?;
        stored.active = active;
        let out = stored.clone();
        self.persist()?;
        Ok(out)
    }

    /// Remove a workflow with its runs, own variables, queue, state and waits.
    pub fn delete_workflow(&mut self, id: &str) -> Result<()> {
        let before = self.doc.workflows.len();
        self.doc.workflows.retain(|workflow| workflow.id != id);
        if self.doc.workflows.len() == before {
            return Err(Error::Missing(format!("workflow {id}")));
        }
        self.doc.workflow_runs.retain(|run| run.workflow_id != id);
        self.doc
            .workflow_vars
            .retain(|var| var.scope != VarScope::Workflow(id.to_string()));
        self.doc
            .workflow_queue
            .retain(|batch| batch.workflow_id != id);
        self.doc
            .workflow_state
            .retain(|state| state.workflow_id != id);
        self.doc
            .workflow_waits
            .retain(|wait| wait.workflow_id != id);
        self.persist()
    }

    /// Put an earlier graph back; the current one becomes a version itself.
    pub fn restore_workflow_version(
        &mut self,
        id: &str,
        saved_at: &str,
        now: i64,
    ) -> Result<SavedWorkflow> {
        let workflow = self.workflow(id)?;
        let version = workflow
            .versions
            .iter()
            .find(|version| version.saved_at == saved_at)
            .ok_or_else(|| Error::Missing(format!("version {saved_at}")))?;
        let graph = Graph::parse(&version.graph)?;
        let active = workflow.active && !has_errors(&validate(&graph));
        let draft = WorkflowDraft {
            name: workflow.name.clone(),
            description: workflow.description.clone(),
            active,
            graph: version.graph.clone(),
            react_to_workflows: workflow.react_to_workflows,
            max_amount: workflow.max_amount,
        };
        self.save_workflow(Some(id), draft, now)
    }

    pub fn export_workflow(&self, id: &str) -> Result<String> {
        let workflow = self.workflow(id)?;
        let graph: serde_json::Value = serde_json::from_str(&workflow.graph).unwrap_or_default();
        serde_json::to_string_pretty(&ExportedWorkflow {
            format: EXPORT_FORMAT.into(),
            version: 1,
            name: workflow.name,
            description: workflow.description,
            graph,
        })
        .map_err(|err| Error::InvalidInput(err.to_string()))
    }

    /// Add an exported workflow, switched off: it is checked before it runs.
    pub fn import_workflow(&mut self, json: &str, now: i64) -> Result<SavedWorkflow> {
        let exported: ExportedWorkflow = serde_json::from_str(json)
            .map_err(|err| Error::InvalidInput(format!("not a workflow file: {err}")))?;
        if exported.format != EXPORT_FORMAT {
            return Err(Error::InvalidInput("not a workflow file".into()));
        }
        let draft = WorkflowDraft {
            name: exported.name,
            description: exported.description,
            active: false,
            graph: exported.graph.to_string(),
            react_to_workflows: false,
            max_amount: None,
        };
        self.save_workflow(None, draft, now)
    }

    // -- tests ------------------------------------------------------------------------

    /// Add a test (empty id) or replace the one with the same id.
    pub fn save_workflow_test(
        &mut self,
        workflow_id: &str,
        mut test: WorkflowTest,
    ) -> Result<Workflow> {
        if test.name.trim().is_empty() {
            return Err(Error::InvalidInput("name the test".into()));
        }
        if !OUTCOMES.contains(&test.expect.outcome.as_str()) {
            return Err(Error::InvalidInput(format!(
                "unknown outcome {}",
                test.expect.outcome
            )));
        }
        if let Some(at) = &test.at
            && !at.trim().is_empty()
            && unix_secs(at).is_none()
        {
            return Err(Error::InvalidInput(format!("not a moment: {at}")));
        }
        let workflow = self
            .doc
            .workflows
            .iter_mut()
            .find(|workflow| workflow.id == workflow_id)
            .ok_or_else(|| Error::Missing(format!("workflow {workflow_id}")))?;
        if test.id.trim().is_empty() {
            test.id = uuid::Uuid::new_v4().to_string();
        }
        test.name = test.name.trim().to_string();
        match workflow.tests.iter_mut().find(|known| known.id == test.id) {
            Some(known) => *known = test,
            None => workflow.tests.push(test),
        }
        let out = workflow.clone();
        self.persist()?;
        Ok(out)
    }

    pub fn delete_workflow_test(&mut self, workflow_id: &str, test_id: &str) -> Result<Workflow> {
        let workflow = self
            .doc
            .workflows
            .iter_mut()
            .find(|workflow| workflow.id == workflow_id)
            .ok_or_else(|| Error::Missing(format!("workflow {workflow_id}")))?;
        workflow.tests.retain(|test| test.id != test_id);
        let out = workflow.clone();
        self.persist()?;
        Ok(out)
    }

    /// Run every test of a workflow by emulation; nothing is written.
    pub fn run_workflow_tests(
        &self,
        workflow_id: &str,
        now: i64,
        zone: &str,
    ) -> Result<Vec<WorkflowTestResult>> {
        let workflow = self.workflow(workflow_id)?;
        workflow
            .tests
            .iter()
            .map(|test| {
                let at = test.at.as_deref().and_then(unix_secs).unwrap_or(now);
                let report = self.dry_run_workflow(
                    &workflow.graph,
                    Some(workflow_id),
                    test.trigger_node.as_deref(),
                    test.answers.clone().unwrap_or_default(),
                    at,
                    zone,
                )?;
                let (problems_ru, problems_en) = test_problems(&test.expect, &report);
                Ok(WorkflowTestResult {
                    test_id: test.id.clone(),
                    name: test.name.clone(),
                    passed: problems_en.is_empty(),
                    problems_ru,
                    problems_en,
                    report,
                })
            })
            .collect()
    }

    // -- journal, queue, variables ---------------------------------------------------

    /// Runs, newest first, of one workflow or of all.
    pub fn workflow_runs(&self, workflow_id: Option<&str>, limit: usize) -> Vec<WorkflowRun> {
        self.doc
            .workflow_runs
            .iter()
            .rev()
            .filter(|run| workflow_id.is_none_or(|id| run.workflow_id == id))
            .take(limit)
            .cloned()
            .collect()
    }

    pub fn workflow_queue(&self) -> Vec<QueuedActions> {
        self.doc.workflow_queue.clone()
    }

    /// Make the operations a workflow queued, all together or none.
    pub fn confirm_queued(&mut self, id: &str) -> Result<Vec<Transaction>> {
        let batch = self
            .doc
            .workflow_queue
            .iter()
            .find(|batch| batch.id == id)
            .cloned()
            .ok_or_else(|| Error::Missing(format!("queued {id}")))?;
        let saved = if batch.transactions.len() == 1 {
            let mut txs = batch.transactions;
            vec![self.add_transaction(txs.remove(0))?]
        } else {
            self.add_linked_transactions(batch.transactions, None)?
        };
        self.doc.workflow_queue.retain(|batch| batch.id != id);
        self.persist()?;
        Ok(saved)
    }

    pub fn discard_queued(&mut self, id: &str) -> Result<()> {
        let before = self.doc.workflow_queue.len();
        self.doc.workflow_queue.retain(|batch| batch.id != id);
        if self.doc.workflow_queue.len() == before {
            return Err(Error::Missing(format!("queued {id}")));
        }
        self.persist()
    }

    /// Runs asleep: timers and questions waiting for an answer, oldest first.
    pub fn workflow_waits(&self) -> Vec<WorkflowWait> {
        self.doc.workflow_waits.clone()
    }

    /// Answer a «Ждать подтверждения» question: the run goes on from «Да» or
    /// «Нет» right away.
    pub fn answer_workflow_wait(
        &mut self,
        id: &str,
        yes: bool,
        now: i64,
        zone: &str,
    ) -> Result<RunReport> {
        let wait = self
            .doc
            .workflow_waits
            .iter()
            .find(|wait| wait.id == id && wait.kind == WaitKind::Confirm)
            .cloned()
            .ok_or_else(|| Error::Missing(format!("question {id}")))?;
        let workflow = self.workflow(&wait.workflow_id)?;
        if !workflow.active {
            return Err(Error::InvalidInput(
                "the workflow is switched off: switch it on to answer".into(),
            ));
        }
        if self.runs_last_hour(&workflow.id, now) >= MAX_RUNS_PER_HOUR {
            return Err(Error::InvalidInput(format!(
                "the workflow ran {MAX_RUNS_PER_HOUR} times this hour: answer a bit later"
            )));
        }
        self.doc.workflow_waits.retain(|known| known.id != id);
        let graph = wait_graph(&wait, &workflow)?;
        if graph.node(&wait.node_id).is_none() {
            self.persist()?;
            return Err(Error::InvalidInput(
                "the question's node was removed from the workflow; the run is dropped".into(),
            ));
        }
        let mut ctx = RunContext::new(&workflow.id, &workflow.name, &wait.node_id, now, zone);
        ctx.max_amount = workflow.max_amount;
        ctx.trigger_label = format!("{} → {}", wait.trigger, if yes { "yes" } else { "no" });
        ctx.resume = Some(Resume {
            node_id: wait.node_id.clone(),
            port: if yes { "true" } else { "false" }.into(),
            memo: wait.memo.clone(),
        });
        let mut pass = WorkflowPass::default();
        let report = exec::run(self, &graph, &ctx);
        self.keep_run(&report, &mut pass);
        let events = report.events.clone();
        self.run_events(events, now, zone, &mut pass);
        self.recompute_holdings();
        self.persist()?;
        if !pass.notices.is_empty() {
            self.log_notices(pass.notices)?;
        }
        Ok(report)
    }

    pub fn workflow_vars(&self) -> Vec<WorkflowVar> {
        self.doc.workflow_vars.clone()
    }

    /// Set a variable by hand; `value` is a `WfValue` in JSON.
    pub fn set_workflow_var(&mut self, scope: VarScope, name: &str, value: &str) -> Result<()> {
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::InvalidInput("name the variable".into()));
        }
        let parsed: WfValue = serde_json::from_str(value)
            .map_err(|err| Error::InvalidInput(format!("variable value: {err}")))?;
        self.wf_set_var(scope, name, &parsed);
        self.persist()
    }

    pub fn delete_workflow_var(&mut self, scope: &VarScope, name: &str) -> Result<()> {
        self.doc
            .workflow_vars
            .retain(|var| !(var.scope == *scope && var.name == name));
        self.persist()
    }

    // -- running -------------------------------------------------------------------

    fn context(&self, workflow: &Workflow, trigger: &Node, now: i64, zone: &str) -> RunContext {
        let mut ctx = RunContext::new(&workflow.id, &workflow.name, &trigger.id, now, zone);
        ctx.max_amount = workflow.max_amount;
        ctx
    }

    /// Keep what a live run left: its journal entry, queue, wait and notices.
    fn keep_run(&mut self, report: &RunReport, pass: &mut WorkflowPass) {
        self.doc.workflow_runs.push(report.run.clone());
        let overflow = self
            .doc
            .workflow_runs
            .len()
            .saturating_sub(MAX_WORKFLOW_RUNS);
        self.doc.workflow_runs.drain(..overflow);
        self.doc
            .workflow_queue
            .extend(report.queued.iter().cloned());
        if let Some(wait) = &report.wait {
            self.doc.workflow_waits.push(wait.clone());
            if wait.kind == WaitKind::Confirm && report.run.dry_run.not() {
                let title = self
                    .doc
                    .workflows
                    .iter()
                    .find(|workflow| workflow.id == wait.workflow_id)
                    .map(|workflow| workflow.name.clone())
                    .unwrap_or_default();
                pass.notices.push(crate::service::notice(
                    crate::model::notify::NoticeCategory::Automation,
                    format!("wf-ask:{}", wait.id),
                    &title,
                    if wait.question.is_empty() {
                        "Нужен ваш ответ, чтобы продолжить".to_string()
                    } else {
                        format!("Нужен ваш ответ: {}", wait.question)
                    },
                    Some("workflows-queue".to_string()),
                    wait.created_at.clone(),
                ));
            }
        }
        pass.created.extend(report.run.created.iter().cloned());
        pass.queued += report.run.queued;
        pass.notices.extend(report.notices.iter().cloned());
        pass.runs.push(report.run.clone());
    }

    /// A run that never happened, in the journal with its reason, so nothing
    /// is lost without a trace.
    fn keep_dropped(
        &mut self,
        workflow_id: &str,
        trigger: &str,
        reason: &str,
        now: i64,
        pass: &mut WorkflowPass,
    ) {
        let (ru, en) = match reason {
            "off" => (
                "Схема выключена, пока запуск ждал: продолжения не будет",
                "The workflow was switched off while the run slept: it does not go on",
            ),
            "node_removed" => (
                "Узел ожидания удалён из схемы: продолжения не будет",
                "The wait node was removed from the workflow: the run does not go on",
            ),
            "skipped" => (
                "Пропущено: запуск не выполнялся",
                "Skipped: the run was not made",
            ),
            _ => (
                "Больше 60 запусков за час: этот пропущен",
                "More than 60 runs this hour: this one is skipped",
            ),
        };
        let run = WorkflowRun {
            id: uuid::Uuid::new_v4().to_string(),
            workflow_id: workflow_id.to_string(),
            trigger: trigger.to_string(),
            started_at: crate::calendar::iso(now),
            outcome: RunOutcome::Dropped {
                reason: reason.to_string(),
            },
            created: Vec::new(),
            queued: 0,
            trace: vec![TraceStep {
                node_id: String::new(),
                kind: String::new(),
                status: StepStatus::Skipped,
                text_ru: ru.into(),
                text_en: en.into(),
                values: Default::default(),
            }],
            dry_run: false,
        };
        self.doc.workflow_runs.push(run.clone());
        pass.runs.push(run);
    }

    fn runs_last_hour(&self, workflow_id: &str, now: i64) -> usize {
        self.doc
            .workflow_runs
            .iter()
            .filter(|run| run.workflow_id == workflow_id && !run.dry_run)
            .filter(|run| !matches!(run.outcome, RunOutcome::Dropped { .. }))
            .filter(|run| unix_secs(&run.started_at).is_some_and(|secs| now - secs < 3600))
            .count()
    }

    /// Run a refused run again now: the money is checked now and the
    /// operation dated now, still naming the moment it was planned for. On
    /// success the refused entry gives way to the new run; still short, it
    /// stays and the error says why.
    pub fn retry_workflow_run(
        &mut self,
        run_id: &str,
        now: i64,
        zone: &str,
    ) -> Result<WorkflowPass> {
        let run = self
            .doc
            .workflow_runs
            .iter()
            .find(|run| run.id == run_id)
            .cloned()
            .ok_or_else(|| Error::Missing(format!("workflow run {run_id}")))?;
        let RunOutcome::Rejected {
            trigger_node,
            planned_at,
            ..
        } = &run.outcome
        else {
            return Err(Error::InvalidInput(
                "only a refused run can be retried".into(),
            ));
        };
        let workflow = self.workflow(&run.workflow_id)?;
        let graph = Graph::parse(&workflow.graph)?;
        let trigger = graph
            .node(trigger_node)
            .cloned()
            .ok_or_else(|| Error::Missing(format!("trigger {trigger_node}")))?;
        let mut ctx = self.context(&workflow, &trigger, now, zone);
        ctx.trigger_label.clone_from(&run.trigger);
        ctx.retry = true;
        if let Some(at) = planned_at {
            ctx.trigger_values = BTreeMap::from([("at".to_string(), WfValue::Date(at.clone()))]);
        }
        ctx.mode = RunMode::Live;
        let report = exec::run(self, &graph, &ctx);
        if let RunOutcome::Rejected { reason, .. } = &report.run.outcome {
            return Err(match reason {
                RejectReason::Invalid { message } => Error::InvalidInput(message.clone()),
                other => Error::InsufficientFunds(self.describe_shortfall(other)),
            });
        }
        self.doc.workflow_runs.retain(|stored| stored.id != run_id);
        let mut pass = WorkflowPass::default();
        self.keep_run(&report, &mut pass);
        self.run_events(report.events.clone(), now, zone, &mut pass);
        self.recompute_holdings();
        self.persist()?;
        if !pass.notices.is_empty() {
            pass.notices = self.log_notices(std::mem::take(&mut pass.notices))?;
        }
        Ok(pass)
    }

    /// Dismiss a refused run: it stays in the journal as skipped.
    pub fn skip_workflow_run(&mut self, run_id: &str) -> Result<WorkflowRun> {
        let run = self
            .doc
            .workflow_runs
            .iter_mut()
            .find(|run| run.id == run_id)
            .ok_or_else(|| Error::Missing(format!("workflow run {run_id}")))?;
        if !matches!(run.outcome, RunOutcome::Rejected { .. }) {
            return Err(Error::InvalidInput(
                "only a refused run can be skipped".into(),
            ));
        }
        run.outcome = RunOutcome::Dropped {
            reason: "skipped".into(),
        };
        let out = run.clone();
        self.persist()?;
        Ok(out)
    }

    /// Run one trigger for real and keep the result. Returns the events sent.
    fn fire(
        &mut self,
        workflow: &Workflow,
        graph: &Graph,
        mut ctx: RunContext,
        pass: &mut WorkflowPass,
    ) -> Vec<EmittedEvent> {
        if self.runs_last_hour(&workflow.id, ctx.now) >= MAX_RUNS_PER_HOUR {
            self.keep_dropped(
                &workflow.id,
                &ctx.trigger_label,
                "rate_limit",
                ctx.now,
                pass,
            );
            return Vec::new();
        }
        ctx.mode = RunMode::Live;
        let report = exec::run(self, graph, &ctx);
        self.keep_run(&report, pass);
        report.events
    }

    /// Several firings of one trigger in a pass: with a «Очередь» in the
    /// workflow, one run that hands them on one by one; otherwise a run each.
    fn fire_all(
        &mut self,
        workflow: &Workflow,
        graph: &Graph,
        firings: Vec<RunContext>,
        pass: &mut WorkflowPass,
    ) -> Vec<EmittedEvent> {
        if firings.len() > 1 && has_queue(graph) {
            let batch: Vec<_> = firings
                .iter()
                .map(|ctx| ctx.trigger_values.clone())
                .collect();
            let mut first = firings.into_iter().next().unwrap_or_else(|| unreachable!());
            first.trigger_label = format!("{} ×{}", first.trigger_label, batch.len());
            first.batch = batch;
            return self.fire(workflow, graph, first, pass);
        }
        let mut events = Vec::new();
        for ctx in firings {
            events.extend(self.fire(workflow, graph, ctx, pass));
        }
        events
    }

    /// An example batch for «Очередь» when run by hand or emulated: the
    /// latest firings of an operation trigger (up to 3, oldest first).
    fn sample_batch(
        &self,
        graph: &Graph,
        trigger: &Node,
        now: i64,
        zone: &str,
    ) -> Vec<BTreeMap<String, WfValue>> {
        if !has_queue(graph) {
            return Vec::new();
        }
        if trigger.kind == "trigger.transaction" {
            let mut latest: Vec<_> = self
                .doc
                .transactions
                .iter()
                .rev()
                .filter(|tx| tx_matches(self, &trigger.params, tx))
                .take(3)
                .map(tx_trigger_values)
                .collect();
            latest.reverse();
            if !latest.is_empty() {
                return latest;
            }
        }
        vec![self.sample_trigger_values(trigger, now, zone)]
    }

    /// Run a workflow now from a trigger (a «Запуск вручную» one by default).
    pub fn run_workflow_now(
        &mut self,
        id: &str,
        trigger_node: Option<&str>,
        now: i64,
        zone: &str,
    ) -> Result<RunReport> {
        let workflow = self.workflow(id)?;
        let graph = Graph::parse(&workflow.graph)?;
        if let Some(issue) = validate(&graph)
            .into_iter()
            .find(|issue| issue.level == crate::workflow::IssueLevel::Error)
        {
            return Err(Error::InvalidInput(issue.text_en));
        }
        let trigger = pick_trigger(&graph, trigger_node)?;
        let mut ctx = self.context(&workflow, &trigger, now, zone);
        // «Запуск вручную» is the button's own trigger; any other is started
        // by hand to try it, and the journal says so.
        ctx.trigger_label = if trigger.kind == "trigger.manual" {
            trigger_label("manual", "")
        } else {
            trigger_label(&trigger.kind, "by hand")
        };
        ctx.trigger_values = self.sample_trigger_values(&trigger, now, zone);
        ctx.batch = self.sample_batch(&graph, &trigger, now, zone);
        let mut pass = WorkflowPass::default();
        let report = exec::run(self, &graph, &ctx);
        self.keep_run(&report, &mut pass);
        let events = report.events.clone();
        self.run_events(events, now, zone, &mut pass);
        self.recompute_holdings();
        self.persist()?;
        if !pass.notices.is_empty() {
            self.log_notices(pass.notices)?;
        }
        Ok(report)
    }

    /// Emulate a graph — saved or still in the editor — on a copy of the vault.
    pub fn dry_run_workflow(
        &self,
        graph_json: &str,
        workflow_id: Option<&str>,
        trigger_node: Option<&str>,
        answers: BTreeMap<String, bool>,
        now: i64,
        zone: &str,
    ) -> Result<RunReport> {
        let graph = Graph::parse(graph_json)?;
        let trigger = pick_trigger(&graph, trigger_node)?;
        let mut sandbox = self.sandbox();
        let (id, name, max_amount) = match workflow_id.and_then(|id| self.workflow(id).ok()) {
            Some(workflow) => (workflow.id, workflow.name, workflow.max_amount),
            None => ("draft".to_string(), "Черновик".to_string(), None),
        };
        let mut ctx = RunContext::new(&id, &name, &trigger.id, now, zone);
        ctx.max_amount = max_amount;
        ctx.mode = RunMode::DryRun;
        ctx.trigger_label = trigger_label(&trigger.kind, "(emulation)");
        ctx.trigger_values = self.sample_trigger_values(&trigger, now, zone);
        ctx.batch = self.sample_batch(&graph, &trigger, now, zone);
        ctx.answers = answers;
        let mut report = exec::run(&mut sandbox, &graph, &ctx);
        // The workflows its events start run too, on the same copy, so the
        // emulation shows the whole chain a real run would set off.
        let mut pass = WorkflowPass::default();
        sandbox.run_events(report.events.clone(), now, zone, &mut pass);
        report.event_runs = pass
            .runs
            .into_iter()
            .map(|run| WorkflowRun {
                dry_run: true,
                ..run
            })
            .collect();
        Ok(report)
    }

    /// What a trigger would hand on now: for an operation trigger the latest
    /// matching operation, for thresholds the current value.
    fn sample_trigger_values(
        &self,
        trigger: &Node,
        now: i64,
        zone: &str,
    ) -> BTreeMap<String, WfValue> {
        let at = || BTreeMap::from([("at".to_string(), WfValue::Date(crate::calendar::iso(now)))]);
        match trigger.kind.as_str() {
            // As if the schedule fired at its latest planned moment.
            "trigger.schedule" => {
                let (moments, _) = schedule_moments(&trigger.params, &Calendar::new(zone), 0, now);
                let moment = moments.last().copied().unwrap_or(now);
                BTreeMap::from([(
                    "at".to_string(),
                    WfValue::Date(crate::calendar::iso(moment)),
                )])
            }
            "trigger.transaction" => self
                .doc
                .transactions
                .iter()
                .rev()
                .find(|tx| tx_matches(self, &trigger.params, tx))
                .map(tx_trigger_values)
                .unwrap_or_default(),
            "trigger.balance" => self
                .threshold_value(trigger, now)
                .map(|(value, _)| BTreeMap::from([("balance".to_string(), value)]))
                .unwrap_or_default(),
            "trigger.price" => self
                .threshold_value(trigger, now)
                .map(|(value, _)| BTreeMap::from([("price".to_string(), value)]))
                .unwrap_or_default(),
            _ => at(),
        }
    }

    /// The watched value of a threshold trigger and which side of it it is on.
    fn threshold_value(&self, trigger: &Node, now: i64) -> Option<(WfValue, bool)> {
        let params = &trigger.params;
        let threshold = params
            .get("value")
            .and_then(crate::workflow::nodes::json_number)
            .unwrap_or(Amount::ZERO);
        let asset = params
            .get("asset")
            .filter(|value| !value.is_null())
            .and_then(|value| match value {
                serde_json::Value::String(symbol) => Some(self.wf_asset(symbol)),
                other => serde_json::from_value::<AssetRef>(other.clone()).ok(),
            });
        let value = if trigger.kind == "trigger.price" {
            let currency = params
                .get("currency")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("usd")
                .to_lowercase();
            let price = self.wf_price(asset.as_ref()?, &currency, now)?;
            WfValue::money(price, &currency)
        } else {
            let store = params
                .get("store")
                .and_then(crate::workflow::nodes::json_account)?;
            self.wf_balance(store.store_id()?, asset.as_ref()).ok()?
        };
        let above = value.as_number()? >= threshold;
        Some((value, above))
    }

    /// Run the workflows listening for events, as deep as `MAX_EVENT_DEPTH`.
    fn run_events(
        &mut self,
        mut events: Vec<EmittedEvent>,
        now: i64,
        zone: &str,
        pass: &mut WorkflowPass,
    ) {
        for _depth in 0..MAX_EVENT_DEPTH {
            if events.is_empty() {
                return;
            }
            let mut next = Vec::new();
            let round = std::mem::take(&mut events);
            for workflow in self.active_workflows() {
                let Ok(graph) = Graph::parse(&workflow.graph) else {
                    continue;
                };
                let listeners: Vec<Node> = graph
                    .nodes
                    .iter()
                    .filter(|node| !node.disabled && node.kind == "trigger.event")
                    .cloned()
                    .collect();
                for trigger in listeners {
                    let listens = trigger
                        .params
                        .get("name")
                        .and_then(serde_json::Value::as_str)
                        .map(str::trim)
                        .unwrap_or_default()
                        .to_string();
                    // The same event sent several times this round: one queue.
                    let firings: Vec<RunContext> = round
                        .iter()
                        .filter(|event| event.name == listens)
                        .map(|event| {
                            let mut ctx = self.context(&workflow, &trigger, now, zone);
                            ctx.trigger_label = trigger_label("event", &event.name);
                            ctx.trigger_values =
                                BTreeMap::from([("payload".to_string(), event.payload.clone())]);
                            ctx
                        })
                        .collect();
                    next.extend(self.fire_all(&workflow, &graph, firings, pass));
                }
            }
            events = next;
        }
    }

    fn active_workflows(&self) -> Vec<Workflow> {
        self.doc
            .workflows
            .iter()
            .filter(|workflow| workflow.active)
            .cloned()
            .collect()
    }

    /// Operations added since a workflow last looked, and where it is now.
    fn new_transactions(&self, workflow_id: &str) -> (Vec<Transaction>, usize, Option<String>) {
        let len = self.doc.transactions.len();
        let last = self.doc.transactions.last().map(|tx| tx.meta().id.clone());
        let Some(state) = self
            .doc
            .workflow_state
            .iter()
            .find(|state| state.workflow_id == workflow_id)
        else {
            return (Vec::new(), len, last);
        };
        let start = match &state.tx_cursor {
            Some(cursor) => self
                .doc
                .transactions
                .iter()
                .position(|tx| tx.meta().id == *cursor)
                .map_or(state.tx_index.min(len), |index| index + 1),
            None => state.tx_index.min(len),
        };
        (self.doc.transactions[start..].to_vec(), len, last)
    }

    /// Fire every trigger that is due: waits that are over, schedules,
    /// new operations, thresholds crossed, then the events they sent.
    pub fn run_due_workflows(&mut self, now: i64, zone: &str) -> Result<WorkflowPass> {
        let mut pass = WorkflowPass::default();
        let mut events = Vec::new();

        let due: Vec<_> = self
            .doc
            .workflow_waits
            .iter()
            .filter(|wait| unix_secs(&wait.resume_at).is_some_and(|secs| secs <= now))
            .cloned()
            .collect();
        for wait in due {
            let Ok(workflow) = self.workflow(&wait.workflow_id) else {
                self.doc.workflow_waits.retain(|known| known.id != wait.id);
                continue;
            };
            // Over the hourly limit: the run sleeps on and goes on at a later tick.
            if workflow.active && self.runs_last_hour(&workflow.id, now) >= MAX_RUNS_PER_HOUR {
                continue;
            }
            self.doc.workflow_waits.retain(|known| known.id != wait.id);
            let graph = match wait_graph(&wait, &workflow) {
                Ok(graph) if graph.node(&wait.node_id).is_some() => graph,
                _ => {
                    self.keep_dropped(&workflow.id, &wait.trigger, "node_removed", now, &mut pass);
                    continue;
                }
            };
            if !workflow.active {
                self.keep_dropped(&workflow.id, &wait.trigger, "off", now, &mut pass);
                continue;
            }
            let mut ctx = RunContext::new(&workflow.id, &workflow.name, &wait.node_id, now, zone);
            ctx.max_amount = workflow.max_amount;
            ctx.trigger_label = format!("{} → wait", wait.trigger);
            ctx.resume = Some(Resume {
                node_id: wait.node_id.clone(),
                // A question nobody answered in time counts as «No».
                port: match wait.kind {
                    WaitKind::Timer => "out".into(),
                    WaitKind::Confirm => "false".into(),
                },
                memo: wait.memo.clone(),
            });
            events.extend(self.fire(&workflow, &graph, ctx, &mut pass));
        }

        for workflow in self.active_workflows() {
            let Ok(graph) = Graph::parse(&workflow.graph) else {
                continue;
            };
            if has_errors(&validate(&graph)) {
                continue;
            }
            let triggers: Vec<Node> = graph
                .nodes
                .iter()
                .filter(|node| {
                    !node.disabled && node_def(&node.kind).is_some_and(|def| def.is_trigger())
                })
                .cloned()
                .collect();
            let (fresh, len, last) = self.new_transactions(&workflow.id);
            for trigger in &triggers {
                match trigger.kind.as_str() {
                    "trigger.schedule" => {
                        let key = format!("schedule:{}", trigger.id);
                        let Some(after) = self
                            .wf_mark(&workflow.id, &key)
                            .and_then(|mark| unix_secs(&mark))
                        else {
                            self.wf_set_mark(&workflow.id, &key, &crate::calendar::iso(now));
                            continue;
                        };
                        let (moments, _skipped) =
                            schedule_moments(&trigger.params, &Calendar::new(zone), after, now);
                        let firings: Vec<RunContext> = moments
                            .iter()
                            .map(|moment| {
                                let at = crate::calendar::iso(*moment);
                                let mut ctx = self.context(&workflow, trigger, now, zone);
                                ctx.trigger_label = trigger_label(&trigger.kind, &at);
                                ctx.trigger_values =
                                    BTreeMap::from([("at".to_string(), WfValue::Date(at))]);
                                ctx
                            })
                            .collect();
                        events.extend(self.fire_all(&workflow, &graph, firings, &mut pass));
                        if let Some(last) = moments.last() {
                            self.wf_set_mark(&workflow.id, &key, &crate::calendar::iso(*last));
                        }
                    }
                    "trigger.transaction" => {
                        let mut params = trigger.params.clone();
                        if workflow.react_to_workflows {
                            params
                                .insert("include_workflows".into(), serde_json::Value::Bool(true));
                        }
                        let matching: Vec<&Transaction> = fresh
                            .iter()
                            .filter(|tx| tx_matches(self, &params, tx))
                            .collect();
                        let mut firings = Vec::new();
                        for tx in matching {
                            // A workflow never reacts to what it made itself.
                            if matches!(&tx.meta().origin, crate::model::transaction::TxOrigin::Workflow { workflow_id, .. } if *workflow_id == workflow.id)
                            {
                                continue;
                            }
                            let mut ctx = self.context(&workflow, trigger, now, zone);
                            ctx.trigger_label = trigger_label(&trigger.kind, &tx.meta().id);
                            ctx.trigger_values = tx_trigger_values(tx);
                            firings.push(ctx);
                        }
                        events.extend(self.fire_all(&workflow, &graph, firings, &mut pass));
                    }
                    "trigger.balance" | "trigger.price" => {
                        let Some((value, above)) = self.threshold_value(trigger, now) else {
                            continue;
                        };
                        let key = format!("threshold:{}", trigger.id);
                        let side = if above { "above" } else { "below" };
                        let previous = self.wf_mark(&workflow.id, &key);
                        if previous.as_deref() == Some(side) {
                            continue;
                        }
                        let wanted = trigger
                            .params
                            .get("condition")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("below");
                        if wanted == side {
                            let port = if trigger.kind == "trigger.price" {
                                "price"
                            } else {
                                "balance"
                            };
                            let mut ctx = self.context(&workflow, trigger, now, zone);
                            ctx.trigger_label = trigger_label(&trigger.kind, &value.as_text());
                            ctx.trigger_values = BTreeMap::from([(port.to_string(), value)]);
                            events.extend(self.fire(&workflow, &graph, ctx, &mut pass));
                        }
                        self.wf_set_mark(&workflow.id, &key, side);
                    }
                    _ => {}
                }
            }
            // Everything up to here was looked at; what the runs added is seen next pass.
            let state = self.wf_state_mut(&workflow.id);
            state.tx_index = len;
            state.tx_cursor = last;
        }

        self.run_events(events, now, zone, &mut pass);
        self.recompute_holdings();
        self.persist()?;
        if !pass.notices.is_empty() {
            pass.notices = self.log_notices(std::mem::take(&mut pass.notices))?;
        }
        Ok(pass)
    }
}

/// The trigger to start from: the one named, else a manual one, else the first.
fn pick_trigger(graph: &Graph, wanted: Option<&str>) -> Result<Node> {
    let triggers: Vec<&Node> = graph
        .nodes
        .iter()
        .filter(|node| node_def(&node.kind).is_some_and(|def| def.is_trigger()))
        .collect();
    if let Some(id) = wanted {
        return triggers
            .into_iter()
            .find(|node| node.id == id)
            .cloned()
            .ok_or_else(|| Error::Missing(format!("trigger {id}")));
    }
    triggers
        .iter()
        .find(|node| node.kind == "trigger.manual")
        .or_else(|| triggers.first())
        .map(|node| (*node).clone())
        .ok_or_else(|| Error::InvalidInput("the workflow has no trigger".into()))
}

#[cfg(test)]
mod tests;

/// The graph a sleeping run goes on along: the one it fell asleep on, or the
/// workflow's current one for a wait from an older app.
fn wait_graph(wait: &WorkflowWait, workflow: &Workflow) -> Result<Graph> {
    if wait.graph.trim().is_empty() {
        Graph::parse(&workflow.graph)
    } else {
        Graph::parse(&wait.graph)
    }
}

/// Whether a workflow gathers the firings of a pass: it has a «Очередь» that is on.
fn has_queue(graph: &Graph) -> bool {
    graph
        .nodes
        .iter()
        .any(|node| node.kind == "flow.queue" && !node.disabled)
}

/// Whether a workflow (or the automation it was migrated from, which kept
/// its id) made the operation.
pub(crate) fn made_by(tx: &Transaction, workflow_id: &str) -> bool {
    match &tx.meta().origin {
        crate::model::transaction::TxOrigin::Workflow {
            workflow_id: maker, ..
        }
        | crate::model::transaction::TxOrigin::Automation {
            automation_id: maker,
            ..
        } => maker == workflow_id,
        _ => false,
    }
}
