//! `Effect` and `WorkflowHost` (spec core-strangler, F6): a refused "ask
//! first" operation is retried from the queue (`C2-2`), a run asks the host
//! for data one call at a time without holding the vault lock, and "Call
//! scheme" does not take the lock again (`C2-5`).

use std::cell::RefCell;
use std::collections::HashSet;
use std::sync::Mutex;

use chrono::NaiveDate;
use serde_json::{Value, json};

use crate::Amount;
use crate::calendar::now_secs;
use crate::model::datasource::DataSource;
use crate::model::inflation::InflationReading;
use crate::model::planning::{RejectReason, Reminder};
use crate::model::store::{AssetRef, StoreKind, StoreScope};
use crate::model::transaction::Transaction;
use crate::service::test_support::*;
use crate::service::{UnlockedVault, WorkflowDraft};
use crate::workflow::exec::{self, RunContext};
use crate::workflow::host::{
    BudgetReading, Checkpoint, EmergencyReading, GoalReading, RecurringReading, TopupReading,
    WorkflowHost,
};
use crate::workflow::model::{VarScope, Workflow};
use crate::workflow::types::WfValue;
use crate::workflow::{Graph, RunOutcome};

fn node(id: &str, kind: &str, params: Value) -> Value {
    json!({ "id": id, "kind": kind, "params": params })
}

fn wire(from: &str, from_port: &str, to: &str, to_port: &str) -> Value {
    json!({
        "id": format!("{from}.{from_port}>{to}.{to_port}"),
        "from": from, "from_port": from_port, "to": to, "to_port": to_port,
    })
}

fn save(vault: &mut UnlockedVault, name: &str, nodes: Vec<Value>, edges: Vec<Value>) -> Workflow {
    let draft = WorkflowDraft {
        name: name.into(),
        description: String::new(),
        active: true,
        graph: json!({ "version": 1, "nodes": nodes, "edges": edges }).to_string(),
        react_to_workflows: false,
        max_amount: None,
        max_amount_currency: None,
    };
    vault
        .save_workflow(None, draft, now_secs())
        .unwrap()
        .workflow
}

fn store_ref(id: &str) -> Value {
    json!({ "store": id })
}

/// Fixed in the future, so no run dates an operation before its funds.
fn later() -> i64 {
    now_secs() + 5
}

// -- C2-2 ---------------------------------------------------------------------

/// «Ask first → yes: 300 from the card to savings», with only 100 on the card.
fn asked_transfer() -> (tempfile::TempDir, UnlockedVault, String, String, Workflow) {
    asked_transfer_capped(None)
}

/// The same, with the workflow's cap on what a run may move.
fn asked_transfer_capped(
    cap: Option<&str>,
) -> (tempfile::TempDir, UnlockedVault, String, String, Workflow) {
    let (dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Cash, None);
    let savings = store(&mut vault, "Savings", StoreKind::Bank, None);
    vault
        .add_transaction(income(&card, "100", "opening"))
        .unwrap();
    let workflow = save(
        &mut vault,
        "Ask",
        vec![
            node("t", "trigger.manual", json!({})),
            node("c", "action.confirm", json!({ "question": "Move 300?" })),
            node(
                "tx",
                "action.transaction",
                json!({ "tx_kind": "transfer", "account": store_ref(&card), "to": store_ref(&savings), "asset": "USD", "amount": "300", "on_shortfall": "reject" }),
            ),
        ],
        vec![wire("t", "out", "c", "in"), wire("c", "true", "tx", "in")],
    );
    let workflow = match cap {
        None => workflow,
        Some(cap) => {
            let draft = WorkflowDraft {
                name: workflow.name.clone(),
                description: String::new(),
                active: true,
                graph: workflow.graph.clone(),
                react_to_workflows: false,
                max_amount: Some(amount(cap)),
                max_amount_currency: None,
            };
            vault
                .save_workflow(Some(&workflow.id), draft, now_secs())
                .unwrap()
                .workflow
        }
    };
    (dir, vault, card, savings, workflow)
}

#[test]
fn an_asked_operation_refused_for_lack_of_funds_is_booked_by_a_retry() {
    let (_dir, mut vault, card, savings, workflow) = asked_transfer();
    vault
        .run_workflow_now(&workflow.id, None, later(), "UTC")
        .unwrap();
    let wait = vault.workflow_waits().remove(0);
    let report = vault
        .answer_workflow_wait(&wait.id, true, later(), "UTC")
        .unwrap();
    let RunOutcome::Rejected { trigger_node, .. } = &report.run.outcome else {
        panic!("expected a refusal, got {:?}", report.run.outcome);
    };
    assert_eq!(
        trigger_node, "t",
        "the refusal names the trigger, not the question"
    );
    // The refused operation waits in the queue, for the run that was refused.
    let queue = vault.workflow_queue();
    assert_eq!(queue.len(), 1);
    assert_eq!(queue[0].run_id, report.run.id);
    assert_eq!(queue[0].transactions.len(), 1);

    // Still short: the retry says so and keeps the refusal.
    assert!(
        vault
            .retry_workflow_run(&report.run.id, later(), "UTC")
            .is_err()
    );
    assert_eq!(vault.workflow_queue().len(), 1);

    vault
        .add_transaction(income(&card, "5000", "salary"))
        .unwrap();
    let pass = vault
        .retry_workflow_run(&report.run.id, later(), "UTC")
        .unwrap();
    assert_eq!(pass.created.len(), 1, "{:?}", pass.runs);
    assert_eq!(holding_of(&vault, &savings, &usd()), amount("300"));
    assert!(vault.workflow_queue().is_empty());
    assert!(
        vault.workflow_waits().is_empty(),
        "the retry did not ask again"
    );
    assert!(
        vault
            .workflow_runs(Some(&workflow.id), 10)
            .iter()
            .all(|run| run.id != report.run.id),
        "the refusal gives way to the new run"
    );
}

fn refused_asked_transfer() -> (tempfile::TempDir, UnlockedVault, String, String, String) {
    let (dir, mut vault, card, savings, workflow) = asked_transfer();
    vault
        .run_workflow_now(&workflow.id, None, later(), "UTC")
        .unwrap();
    let wait = vault.workflow_waits().remove(0);
    let report = vault
        .answer_workflow_wait(&wait.id, true, later(), "UTC")
        .unwrap();
    assert!(matches!(report.run.outcome, RunOutcome::Rejected { .. }));
    vault
        .add_transaction(income(&card, "5000", "salary"))
        .unwrap();
    (dir, vault, savings, report.run.id, workflow.id)
}

#[test]
fn a_confirmed_refusal_closes_its_run_and_a_retry_books_nothing_more() {
    let (_dir, mut vault, savings, run_id, workflow_id) = refused_asked_transfer();
    let batch = vault.workflow_queue().remove(0);
    vault.confirm_queued(&batch.id).unwrap();
    let booked = vault.doc.transactions.len();
    let holding = holding_of(&vault, &savings, &usd());
    let runs = vault.workflow_runs(Some(&workflow_id), 10);
    assert!(runs.iter().all(|run| run.id != run_id));
    assert!(
        runs.iter()
            .any(|run| matches!(run.outcome, RunOutcome::Done))
    );
    assert!(vault.retry_workflow_run(&run_id, later(), "UTC").is_err());
    assert_eq!(vault.doc.transactions.len(), booked);
    assert_eq!(holding_of(&vault, &savings, &usd()), holding);
}

#[test]
fn a_discarded_refusal_is_no_longer_retryable() {
    let (_dir, mut vault, savings, run_id, workflow_id) = refused_asked_transfer();
    let batch = vault.workflow_queue().remove(0);
    vault.discard_queued(&batch.id).unwrap();
    let runs = vault.workflow_runs(Some(&workflow_id), 10);
    let run = runs.iter().find(|run| run.id == run_id).unwrap();
    assert!(matches!(run.outcome, RunOutcome::Dropped { .. }));
    assert!(vault.retry_workflow_run(&run_id, later(), "UTC").is_err());
    assert_eq!(holding_of(&vault, &savings, &usd()), Amount::ZERO);
}

#[test]
fn an_asked_transfer_over_the_cap_is_not_queued_and_books_nothing() {
    let (_dir, mut vault, card, savings, workflow) = asked_transfer_capped(Some("100"));
    vault
        .run_workflow_now(&workflow.id, None, later(), "UTC")
        .unwrap();
    let wait = vault.workflow_waits().remove(0);
    let report = vault
        .answer_workflow_wait(&wait.id, true, later(), "UTC")
        .unwrap();
    assert!(
        matches!(&report.run.outcome, RunOutcome::Failed { message } if message.contains("cap")),
        "{:?}",
        report.run.outcome
    );
    assert!(
        vault.workflow_queue().is_empty(),
        "over the cap: nothing to retry"
    );
    vault
        .add_transaction(income(&card, "5000", "salary"))
        .unwrap();
    assert!(
        vault
            .retry_workflow_run(&report.run.id, later(), "UTC")
            .is_err()
    );
    assert_eq!(holding_of(&vault, &savings, &usd()), Amount::ZERO);
}

// -- C2-5 and the lock --------------------------------------------------------

/// A host that keeps the vault behind a lock: every call takes it, uses it and
/// gives it back before returning. A call that comes while it is held panics,
/// as a second take of a real lock would block forever.
struct LockedHost {
    vault: Mutex<UnlockedVault>,
    takes: RefCell<Vec<&'static str>>,
}

impl LockedHost {
    fn new(vault: UnlockedVault) -> Self {
        Self {
            vault: Mutex::new(vault),
            takes: RefCell::new(Vec::new()),
        }
    }

    fn take(&self, call: &'static str) -> std::sync::MutexGuard<'_, UnlockedVault> {
        self.takes.borrow_mut().push(call);
        self.vault
            .try_lock()
            .unwrap_or_else(|_| panic!("{call} came while the vault lock was held"))
    }
}

/// Reads: one take each.
macro_rules! locked_reads {
    ($(fn $name:ident(&self $(, $arg:ident: $ty:ty)*) -> $ret:ty;)*) => {$(
        fn $name(&self $(, $arg: $ty)*) -> $ret {
            WorkflowHost::$name(&*self.take(stringify!($name)) $(, $arg)*)
        }
    )*};
}

impl WorkflowHost for LockedHost {
    locked_reads! {
        fn base_currency(&self) -> String;
        fn wf_asset(&self, symbol: &str) -> AssetRef;
        fn wf_balance(&self, store_id: &str, asset: Option<&AssetRef>) -> Result<WfValue, String>;
        fn wf_price(&self, asset: &AssetRef, currency: &str, secs: i64) -> Option<Amount>;
        fn wf_convert(&self, amount: Amount, from: &str, to: &str, secs: i64) -> Option<Amount>;
        fn wf_sum(&self, period: &str, flow: &str, category: Option<&str>, now: i64, zone: &str) -> Amount;
        fn wf_recent_average(&self, workflow_id: &str, count: usize) -> Option<(Amount, String)>;
        fn wf_accounts(&self, parent: Option<&str>) -> Vec<(String, String, Amount)>;
        fn wf_outflow_base(&self, tx: &Transaction) -> Amount;
        fn wf_outflow_in(&self, tx: &Transaction, currency: &str) -> Option<Amount>;
        fn wf_mark(&self, workflow_id: &str, key: &str) -> Option<String>;
        fn wf_var(&self, scope: &VarScope, name: &str) -> Option<WfValue>;
        fn store_name(&self, store_id: &str) -> Option<String>;
        fn contact_name(&self, contact_id: &str) -> Option<String>;
        fn callee(&self, workflow_id: &str) -> Option<Workflow>;
        fn funds_shortfall(&self, tx: &Transaction, secs: i64) -> Option<RejectReason>;
        fn resolve_scope(&self, scope: &StoreScope) -> HashSet<String>;
        fn key_rate_on(&self, day: NaiveDate) -> Option<Amount>;
        fn inflation_series(&self, currency: &str) -> Vec<InflationReading>;
        fn inflation_on(&self, currency: &str, month: &str) -> Option<Amount>;
        fn recurring_check(&self, tx: &Transaction) -> RecurringReading;
        fn topup_advice(&self, amount: Option<Amount>, now: i64, zone: &str) -> TopupReading;
        fn emergency_fund_status(&self, now: i64, zone: &str) -> EmergencyReading;
        fn budget_status(&self, now: i64, zone: &str) -> Vec<BudgetReading>;
        fn data_source(&self, id: &str) -> Option<DataSource>;
        fn goal_progress(&self, now: i64, zone: &str) -> Vec<GoalReading>;
        fn checkpoint(&self) -> Checkpoint;
    }

    fn insert_transactions(
        &mut self,
        txs: Vec<Transaction>,
        now: i64,
    ) -> Result<Vec<Transaction>, String> {
        let mut vault = self.take("insert_transactions");
        WorkflowHost::insert_transactions(&mut *vault, txs, now)
    }

    fn set_mark(&mut self, workflow_id: &str, key: &str, value: &str) {
        let mut vault = self.take("set_mark");
        WorkflowHost::set_mark(&mut *vault, workflow_id, key, value);
    }

    fn set_var(&mut self, scope: VarScope, name: &str, value: &WfValue) {
        let mut vault = self.take("set_var");
        WorkflowHost::set_var(&mut *vault, scope, name, value);
    }

    fn add_reminder(&mut self, reminder: Reminder) {
        let mut vault = self.take("add_reminder");
        WorkflowHost::add_reminder(&mut *vault, reminder);
    }

    fn restore(&mut self, checkpoint: Checkpoint) {
        let mut vault = self.take("restore");
        WorkflowHost::restore(&mut *vault, checkpoint);
    }
}

fn run_on(host: &mut LockedHost, workflow: &Workflow) -> crate::workflow::RunReport {
    let graph = Graph::parse(&workflow.graph).unwrap();
    let ctx = RunContext::new(&workflow.id, &workflow.name, "t", later(), "UTC");
    exec::run(host, &graph, &ctx)
}

#[test]
fn a_node_that_asks_for_data_gets_it_without_the_lock_held() {
    let (_dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Cash, None);
    vault
        .add_transaction(income(&card, "100", "opening"))
        .unwrap();
    let workflow = save(
        &mut vault,
        "Balance",
        vec![
            node("t", "trigger.manual", json!({})),
            node(
                "b",
                "data.balance",
                json!({ "account": store_ref(&card), "asset": "USD" }),
            ),
            node("n", "action.notify", json!({})),
        ],
        vec![wire("t", "out", "n", "in"), wire("b", "value", "n", "text")],
    );
    let mut host = LockedHost::new(vault);
    let report = run_on(&mut host, &workflow);
    assert_eq!(
        report.run.outcome,
        RunOutcome::Done,
        "{:?}",
        report.run.trace
    );
    assert!(
        report.notices[0].body.contains("100"),
        "{}",
        report.notices[0].body
    );
    assert!(host.takes.borrow().contains(&"wf_balance"));
    assert!(host.vault.try_lock().is_ok(), "the run left the lock held");
}

#[test]
fn a_call_scheme_node_takes_the_lock_once_and_not_again() {
    let (_dir, mut vault) = empty_vault();
    let helper = save(
        &mut vault,
        "Helper",
        vec![
            node("t", "trigger.called", json!({})),
            node("r", "action.return", json!({ "value": "done" })),
        ],
        vec![wire("t", "out", "r", "in")],
    );
    let caller = save(
        &mut vault,
        "Caller",
        vec![
            node("t", "trigger.manual", json!({})),
            node("c", "action.call", json!({ "workflow": helper.id })),
        ],
        vec![wire("t", "out", "c", "in")],
    );
    let mut host = LockedHost::new(vault);
    let report = run_on(&mut host, &caller);
    assert_eq!(
        report.run.outcome,
        RunOutcome::Done,
        "{:?}",
        report.run.trace
    );
    // One take for the checkpoint of the run and one to read the helper; the
    // helper's own run goes on without taking the lock for itself.
    assert_eq!(*host.takes.borrow(), vec!["checkpoint", "callee"]);
}
