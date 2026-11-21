//! Shim of the `workflow` area (spec core-strangler, F2, D1): its logic moved
//! to `crate::workflow::service::{records, runs, marks, vars, tests_def}` (pure
//! functions) and `crate::usecases::workflow::{records, runs}` (scenarios). The
//! methods here only forward, and the public types keep their old paths.

use std::collections::{BTreeMap, HashMap};

use super::UnlockedVault;
use super::forward::forward;
use crate::Amount;
use crate::error::Result;
use crate::model::store::AssetRef;
use crate::model::transaction::Transaction;
use crate::time::Clock;
use crate::workflow::exec::RunReport;
use crate::workflow::model::{
    QueuedActions, VarScope, Workflow, WorkflowRun, WorkflowTest, WorkflowVar, WorkflowWait,
};
use crate::workflow::types::WfValue;

// The tests of this area name these through `super::*`.
#[cfg(test)]
use crate::pricing::history::unix_secs;
#[cfg(test)]
use crate::workflow::graph::Graph;
#[cfg(test)]
use crate::workflow::model::{TestExpect, WaitKind};
#[cfg(test)]
use crate::workflow::service::records::EXPORT_FORMAT;
#[cfg(test)]
use crate::workflow::validate::validate;

// `service/automations.rs` re-exports `outflow` for this area, the last to use
// it; the lookups call `planning` directly now. Naming it here keeps the
// re-export live until that file drops it.
#[allow(unused_imports)]
use super::automations::outflow;

pub use crate::workflow::service::records::{
    ExportedStore, SavedWorkflow, WorkflowDraft, WorkflowImportPreview,
};
pub use crate::workflow::service::runs::WorkflowPass;
pub use crate::workflow::service::tests_def::WorkflowTestResult;

impl UnlockedVault {
    // forward:begin
    forward! {
        pub fn workflows(&self) -> Vec<Workflow>
            => crate::workflow::service::records::workflows @doc;
        pub fn workflow(&self, id: &str) -> Result<Workflow>
            => crate::workflow::service::records::workflow @doc;
        /// Create (`id` = `None`) or update a workflow. A changed graph keeps the
        /// one before it among the versions; one switched on starts watching from now.
        pub fn save_workflow(&mut self, id: Option<&str>, draft: WorkflowDraft, now: i64) -> Result<SavedWorkflow>
            => crate::usecases::workflow::records::save_workflow;
        /// Keep the editor's unsaved graph aside: the saved one keeps running.
        pub fn save_workflow_draft(&mut self, id: &str, graph_json: &str, now: i64) -> Result<Workflow>
            => crate::usecases::workflow::records::save_workflow_draft;
        /// Throw the kept-aside edits away.
        pub fn discard_workflow_draft(&mut self, id: &str) -> Result<Workflow>
            => crate::usecases::workflow::records::discard_workflow_draft;
        pub fn set_workflow_active(&mut self, id: &str, active: bool, now: i64) -> Result<Workflow>
            => crate::usecases::workflow::records::set_workflow_active;
        /// Remove a workflow with its runs, own variables, queue, state and waits.
        pub fn delete_workflow(&mut self, id: &str) -> Result<()>
            => crate::usecases::workflow::records::delete_workflow;
        /// Put an earlier graph back; the current one becomes a version itself.
        pub fn restore_workflow_version(&mut self, id: &str, saved_at: &str, now: i64) -> Result<SavedWorkflow>
            => crate::usecases::workflow::records::restore_workflow_version;
        pub fn export_workflow(&self, id: &str) -> Result<String>
            => crate::workflow::service::records::export_workflow @doc;
        /// What a workflow file holds: its name, steps and the wallets this vault lacks.
        pub fn preview_workflow_import(&self, json: &str) -> Result<WorkflowImportPreview>
            => crate::workflow::service::records::preview_import @doc;
        /// Add an exported workflow, switched off: it is checked before it runs.
        /// `stores` swaps wallet ids of the file for wallets of this vault.
        pub fn import_workflow(&mut self, json: &str, stores: &HashMap<String, String>, now: i64) -> Result<SavedWorkflow>
            => crate::usecases::workflow::records::import_workflow;
        /// Add a test (empty id) or replace the one with the same id.
        pub fn save_workflow_test(&mut self, workflow_id: &str, mut test: WorkflowTest) -> Result<Workflow>
            => crate::usecases::workflow::records::save_workflow_test;
        pub fn delete_workflow_test(&mut self, workflow_id: &str, test_id: &str) -> Result<Workflow>
            => crate::usecases::workflow::records::delete_workflow_test;
        /// Run every test of a workflow by emulation; nothing is written.
        pub fn run_workflow_tests(&self, workflow_id: &str, now: i64, zone: &str) -> Result<Vec<WorkflowTestResult>>
            => crate::usecases::workflow::runs::run_workflow_tests;
        /// Runs, newest first, of one workflow or of all.
        pub fn workflow_runs(&self, workflow_id: Option<&str>, limit: usize) -> Vec<WorkflowRun>
            => crate::workflow::service::runs::runs @doc;
        pub fn workflow_queue(&self) -> Vec<QueuedActions>
            => crate::workflow::service::runs::queue @doc;
        /// Make the operations a workflow queued, all together or none.
        pub fn confirm_queued(&mut self, id: &str) -> Result<Vec<Transaction>>
            => crate::usecases::workflow::records::confirm_queued;
        pub fn discard_queued(&mut self, id: &str) -> Result<()>
            => crate::usecases::workflow::records::discard_queued;
        /// Runs asleep: timers and questions waiting for an answer, oldest first.
        pub fn workflow_waits(&self) -> Vec<WorkflowWait>
            => crate::workflow::service::runs::waits @doc;
        /// Answer a "Wait for confirmation" question: the run goes on from "Yes" or
        /// "No" right away.
        pub fn answer_workflow_wait(&mut self, id: &str, yes: bool, now: i64, zone: &str) -> Result<RunReport>
            => crate::usecases::workflow::runs::answer_workflow_wait;
        pub fn workflow_vars(&self) -> Vec<WorkflowVar>
            => crate::workflow::service::vars::variables @doc;
        /// Set a variable by hand; `value` is a `WfValue` in JSON.
        pub fn set_workflow_var(&mut self, scope: VarScope, name: &str, value: &str) -> Result<()>
            => crate::usecases::workflow::records::set_workflow_var;
        pub fn delete_workflow_var(&mut self, scope: &VarScope, name: &str) -> Result<()>
            => crate::usecases::workflow::records::delete_workflow_var;
        /// Run a refused run again now: the money is checked now and the
        /// operation dated now, still naming the moment it was planned for. On
        /// success the refused entry gives way to the new run; still short, it
        /// stays and the error says why.
        pub fn retry_workflow_run(&mut self, run_id: &str, now: i64, zone: &str) -> Result<WorkflowPass>
            => crate::usecases::workflow::runs::retry_workflow_run;
        /// Dismiss a refused run: it stays in the journal as skipped.
        pub fn skip_workflow_run(&mut self, run_id: &str) -> Result<WorkflowRun>
            => crate::usecases::workflow::runs::skip_workflow_run;
        /// Run a workflow now from a trigger (a "Run manually" one by default).
        pub fn run_workflow_now(&mut self, id: &str, trigger_node: Option<&str>, now: i64, zone: &str) -> Result<RunReport>
            => crate::usecases::workflow::runs::run_workflow_now;
        /// Emulate a graph — saved or still in the editor — on a copy of the vault.
        pub fn dry_run_workflow(&self, graph_json: &str, workflow_id: Option<&str>, trigger_node: Option<&str>, answers: BTreeMap<String, bool>, now: i64, zone: &str) -> Result<RunReport>
            => crate::usecases::workflow::runs::dry_run_workflow;
        /// Fire every trigger that is due: waits that are over, schedules,
        /// new operations, thresholds crossed, then the events they sent.
        pub fn run_due_workflows(&mut self, now: i64, zone: &str) -> Result<WorkflowPass>
            => crate::usecases::workflow::runs::run_due_workflows;
        /// [`Self::run_due_workflows`] at the moment `clock` reads.
        pub fn run_due_workflows_at(&mut self, clock: &dyn Clock, zone: &str) -> Result<WorkflowPass>
            => crate::usecases::workflow::runs::run_due_workflows_at;
    }
    // forward:end

    // What the executor reads and applies through the host, and what the open
    // and the sync read; not in the baseline (crate-private), so outside the
    // regenerated block.
    forward! {
        /// An asset by its symbol: one the vault holds, else a currency.
        pub(crate) fn wf_asset(&self, symbol: &str) -> AssetRef
            => crate::workflow::service::vars::asset @doc;
        /// The balance of an account with its sub-accounts: of one asset, or the
        /// whole value in the base currency.
        pub(crate) fn wf_balance(&self, store_id: &str, asset: Option<&AssetRef>) -> std::result::Result<WfValue, String>
            => crate::usecases::workflow::vars::balance @books;
        /// Price of one unit of an asset in a currency.
        pub(crate) fn wf_price(&self, asset: &AssetRef, currency: &str, secs: i64) -> Option<Amount>
            => crate::workflow::service::vars::price @books;
        pub(crate) fn wf_convert(&self, amount: Amount, from: &str, to: &str, secs: i64) -> Option<Amount>
            => crate::workflow::service::vars::convert @books;
        /// Income or spending of the current period, in the base currency.
        pub(crate) fn wf_sum(&self, period: &str, flow: &str, category: Option<&str>, now: i64, zone: &str) -> Amount
            => crate::usecases::workflow::runs::wf_sum @books;
        /// The average amount of the latest `count` operations a workflow made.
        pub(crate) fn wf_recent_average(&self, workflow_id: &str, count: usize) -> Option<(Amount, String)>
            => crate::workflow::service::vars::recent_average @doc;
        /// Live accounts under a wallet, or every live wallet: id, name and value
        /// in the base currency.
        pub(crate) fn wf_accounts(&self, parent: Option<&str>) -> Vec<(String, String, Amount)>
            => crate::usecases::workflow::vars::accounts @books;
        /// What an operation takes out of an account, in the base currency.
        pub(crate) fn wf_outflow_base(&self, tx: &Transaction) -> Amount
            => crate::usecases::workflow::runs::wf_outflow_base @books;
        /// What an operation takes out of an account, in `currency`.
        pub(crate) fn wf_outflow_in(&self, tx: &Transaction, currency: &str) -> Option<Amount>
            => crate::usecases::workflow::runs::wf_outflow_in @books;
        /// Check money and rules, then add the operations; all of them or none.
        /// Not persisted: the run's caller writes once.
        pub(crate) fn wf_insert(&mut self, txs: Vec<Transaction>, now: i64) -> std::result::Result<Vec<Transaction>, String>
            => crate::usecases::workflow::runs::wf_insert;
        pub(crate) fn wf_mark(&self, workflow_id: &str, key: &str) -> Option<String>
            => crate::workflow::service::marks::mark @doc;
        pub(crate) fn wf_set_mark(&mut self, workflow_id: &str, key: &str, value: &str)
            => crate::usecases::workflow::runs::wf_set_mark;
        pub(crate) fn wf_var(&self, scope: &VarScope, name: &str) -> Option<WfValue>
            => crate::workflow::service::vars::var @doc;
        pub(crate) fn wf_set_var(&mut self, scope: VarScope, name: &str, value: &WfValue)
            => crate::usecases::workflow::runs::wf_set_var;
        /// A schedule mark never stays below a moment whose run key is booked.
        pub(crate) fn keep_schedule_marks(&mut self)
            => crate::usecases::workflow::runs::keep_schedule_marks;
        /// Active workflows a required field now stops: switched off on open.
        pub(crate) fn disable_incomplete_workflows(&mut self, now: i64) -> bool
            => crate::usecases::workflow::records::disable_incomplete_workflows;
    }
}

#[cfg(test)]
mod tests;
