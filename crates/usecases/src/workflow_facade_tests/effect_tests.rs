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
use crate::service::Session;
use crate::service::test_support::*;
use crate::workflow::exec::{self, RunContext};
use crate::workflow::host::{
    BudgetReading, Checkpoint, EmergencyReading, GoalReading, RecurringReading, TopupReading,
    WorkflowHost,
};
use crate::workflow::model::{VarScope, Workflow};
use crate::workflow::service::records::WorkflowDraft;
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

fn save(vault: &mut Session, name: &str, nodes: Vec<Value>, edges: Vec<Value>) -> Workflow {
    let draft = WorkflowDraft {
        name: name.into(),
        description: String::new(),
        active: true,
        graph: json!({ "version": 1, "nodes": nodes, "edges": edges }).to_string(),
        react_to_workflows: false,
        max_amount: None,
        max_amount_currency: None,
    };
    crate::usecases::workflow::records::save_workflow(vault, None, draft, now_secs())
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
fn asked_transfer() -> (tempfile::TempDir, Session, String, String, Workflow) {
    asked_transfer_capped(None)
}

/// The same, with the workflow's cap on what a run may move.
fn asked_transfer_capped(
    cap: Option<&str>,
) -> (tempfile::TempDir, Session, String, String, Workflow) {
    let (dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Cash, None);
    let savings = store(&mut vault, "Savings", StoreKind::Bank, None);
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        income(&card, "100", "opening"),
    )
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
            crate::usecases::workflow::records::save_workflow(
                &mut vault,
                Some(&workflow.id),
                draft,
                now_secs(),
            )
            .unwrap()
            .workflow
        }
    };
    (dir, vault, card, savings, workflow)
}

#[test]
fn an_asked_operation_refused_for_lack_of_funds_is_booked_by_a_retry() {
    let (_dir, mut vault, card, savings, workflow) = asked_transfer();
    crate::usecases::workflow::runs::run_workflow_now(
        &mut vault,
        &workflow.id,
        None,
        later(),
        "UTC",
    )
    .unwrap();
    let wait = crate::workflow::service::runs::waits(&vault.doc).remove(0);
    let report = crate::usecases::workflow::runs::answer_workflow_wait(
        &mut vault,
        &wait.id,
        true,
        later(),
        "UTC",
    )
    .unwrap();
    let RunOutcome::Rejected { trigger_node, .. } = &report.run.outcome else {
        panic!("expected a refusal, got {:?}", report.run.outcome);
    };
    assert_eq!(
        trigger_node, "t",
        "the refusal names the trigger, not the question"
    );
    // The refused operation waits in the queue, for the run that was refused.
    let queue = crate::workflow::service::runs::queue(&vault.doc);
    assert_eq!(queue.len(), 1);
    assert_eq!(queue[0].run_id, report.run.id);
    assert_eq!(queue[0].transactions.len(), 1);

    // Still short: the retry says so and keeps the refusal.
    assert!(
        crate::usecases::workflow::runs::retry_workflow_run(
            &mut vault,
            &report.run.id,
            later(),
            "UTC"
        )
        .is_err()
    );
    assert_eq!(crate::workflow::service::runs::queue(&vault.doc).len(), 1);

    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        income(&card, "5000", "salary"),
    )
    .unwrap();
    let pass = crate::usecases::workflow::runs::retry_workflow_run(
        &mut vault,
        &report.run.id,
        later(),
        "UTC",
    )
    .unwrap();
    assert_eq!(pass.created.len(), 1, "{:?}", pass.runs);
    assert_eq!(holding_of(&vault, &savings, &usd()), amount("300"));
    assert!(crate::workflow::service::runs::queue(&vault.doc).is_empty());
    assert!(
        crate::workflow::service::runs::waits(&vault.doc).is_empty(),
        "the retry did not ask again"
    );
    assert!(
        crate::workflow::service::runs::runs(&vault.doc, Some(&workflow.id), 10)
            .iter()
            .all(|run| run.id != report.run.id),
        "the refusal gives way to the new run"
    );
}

#[test]
fn a_refused_retry_is_insufficient_funds_by_variant() {
    let (_dir, mut vault, _card, _savings, workflow) = asked_transfer();
    crate::usecases::workflow::runs::run_workflow_now(
        &mut vault,
        &workflow.id,
        None,
        later(),
        "UTC",
    )
    .unwrap();
    let wait = crate::workflow::service::runs::waits(&vault.doc).remove(0);
    let report = crate::usecases::workflow::runs::answer_workflow_wait(
        &mut vault,
        &wait.id,
        true,
        later(),
        "UTC",
    )
    .unwrap();
    let refusal = crate::usecases::workflow::runs::retry_workflow_run(
        &mut vault,
        &report.run.id,
        later(),
        "UTC",
    );
    assert!(
        matches!(refusal, Err(crate::Error::InsufficientFunds(_))),
        "{refusal:?}"
    );
}

fn refused_asked_transfer() -> (tempfile::TempDir, Session, String, String, String) {
    let (dir, mut vault, card, savings, workflow) = asked_transfer();
    crate::usecases::workflow::runs::run_workflow_now(
        &mut vault,
        &workflow.id,
        None,
        later(),
        "UTC",
    )
    .unwrap();
    let wait = crate::workflow::service::runs::waits(&vault.doc).remove(0);
    let report = crate::usecases::workflow::runs::answer_workflow_wait(
        &mut vault,
        &wait.id,
        true,
        later(),
        "UTC",
    )
    .unwrap();
    assert!(matches!(report.run.outcome, RunOutcome::Rejected { .. }));
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        income(&card, "5000", "salary"),
    )
    .unwrap();
    (dir, vault, savings, report.run.id, workflow.id)
}

#[test]
fn a_confirmed_refusal_closes_its_run_and_a_retry_books_nothing_more() {
    let (_dir, mut vault, savings, run_id, workflow_id) = refused_asked_transfer();
    let batch = crate::workflow::service::runs::queue(&vault.doc).remove(0);
    crate::usecases::workflow::records::confirm_queued(&mut vault, &batch.id).unwrap();
    let booked = vault.doc.transactions.len();
    let holding = holding_of(&vault, &savings, &usd());
    let runs = crate::workflow::service::runs::runs(&vault.doc, Some(&workflow_id), 10);
    assert!(runs.iter().all(|run| run.id != run_id));
    assert!(
        runs.iter()
            .any(|run| matches!(run.outcome, RunOutcome::Done))
    );
    assert!(
        crate::usecases::workflow::runs::retry_workflow_run(&mut vault, &run_id, later(), "UTC")
            .is_err()
    );
    assert_eq!(vault.doc.transactions.len(), booked);
    assert_eq!(holding_of(&vault, &savings, &usd()), holding);
}

#[test]
fn a_confirmed_refusal_keeps_the_started_at_of_the_refused_run() {
    let (_dir, mut vault, _savings, run_id, workflow_id) = refused_asked_transfer();
    let started_at = refused_started_at(&vault, &workflow_id, &run_id);
    let batch = crate::workflow::service::runs::queue(&vault.doc).remove(0);
    crate::usecases::workflow::records::confirm_queued(&mut vault, &batch.id).unwrap();
    let runs = crate::workflow::service::runs::runs(&vault.doc, Some(&workflow_id), 10);
    let done = runs
        .iter()
        .find(|run| matches!(run.outcome, RunOutcome::Done))
        .unwrap();
    assert_eq!(done.started_at, started_at);
}

#[test]
fn a_retried_refusal_keeps_the_started_at_of_the_refused_run() {
    let (_dir, mut vault, _savings, run_id, workflow_id) = refused_asked_transfer();
    let started_at = refused_started_at(&vault, &workflow_id, &run_id);
    // A retry a day later: a fresh stamp would differ from the refusal's.
    crate::usecases::workflow::runs::retry_workflow_run(
        &mut vault,
        &run_id,
        later() + 86_400,
        "UTC",
    )
    .unwrap();
    let runs = crate::workflow::service::runs::runs(&vault.doc, Some(&workflow_id), 10);
    let done = runs
        .iter()
        .find(|run| matches!(run.outcome, RunOutcome::Done))
        .unwrap();
    assert_eq!(done.started_at, started_at);
}

fn refused_started_at(vault: &Session, workflow_id: &str, run_id: &str) -> String {
    crate::workflow::service::runs::runs(&vault.doc, Some(workflow_id), 10)
        .iter()
        .find(|run| run.id == run_id)
        .unwrap()
        .started_at
        .clone()
}

#[test]
fn a_discarded_refusal_is_no_longer_retryable() {
    let (_dir, mut vault, savings, run_id, workflow_id) = refused_asked_transfer();
    let batch = crate::workflow::service::runs::queue(&vault.doc).remove(0);
    crate::usecases::workflow::records::discard_queued(&mut vault, &batch.id).unwrap();
    let runs = crate::workflow::service::runs::runs(&vault.doc, Some(&workflow_id), 10);
    let run = runs.iter().find(|run| run.id == run_id).unwrap();
    assert!(matches!(run.outcome, RunOutcome::Dropped { .. }));
    assert!(
        crate::usecases::workflow::runs::retry_workflow_run(&mut vault, &run_id, later(), "UTC")
            .is_err()
    );
    assert_eq!(holding_of(&vault, &savings, &usd()), Amount::ZERO);
}

#[test]
fn an_asked_transfer_over_the_cap_is_not_queued_and_books_nothing() {
    let (_dir, mut vault, card, savings, workflow) = asked_transfer_capped(Some("100"));
    crate::usecases::workflow::runs::run_workflow_now(
        &mut vault,
        &workflow.id,
        None,
        later(),
        "UTC",
    )
    .unwrap();
    let wait = crate::workflow::service::runs::waits(&vault.doc).remove(0);
    let report = crate::usecases::workflow::runs::answer_workflow_wait(
        &mut vault,
        &wait.id,
        true,
        later(),
        "UTC",
    )
    .unwrap();
    assert!(
        matches!(&report.run.outcome, RunOutcome::Failed { message } if message.contains("cap")),
        "{:?}",
        report.run.outcome
    );
    assert!(
        crate::workflow::service::runs::queue(&vault.doc).is_empty(),
        "over the cap: nothing to retry"
    );
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        income(&card, "5000", "salary"),
    )
    .unwrap();
    assert!(
        crate::usecases::workflow::runs::retry_workflow_run(
            &mut vault,
            &report.run.id,
            later(),
            "UTC"
        )
        .is_err()
    );
    assert_eq!(holding_of(&vault, &savings, &usd()), Amount::ZERO);
}

#[test]
fn a_plain_refusal_under_a_cap_ends_rejected_even_without_a_price() {
    let (_dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Cash, None);
    let savings = store(&mut vault, "Savings", StoreKind::Bank, None);
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        income(&card, "100", "opening"),
    )
    .unwrap();
    let workflow = save(
        &mut vault,
        "Plain",
        vec![
            node("t", "trigger.manual", json!({})),
            node(
                "tx",
                "action.transaction",
                json!({ "tx_kind": "transfer", "account": store_ref(&card), "to": store_ref(&savings), "asset": "NOPRICE", "amount": "300", "on_shortfall": "reject" }),
            ),
        ],
        vec![wire("t", "out", "tx", "in")],
    );
    let draft = WorkflowDraft {
        name: workflow.name.clone(),
        description: String::new(),
        active: true,
        graph: workflow.graph.clone(),
        react_to_workflows: false,
        max_amount: Some(amount("100")),
        max_amount_currency: None,
    };
    let workflow = crate::usecases::workflow::records::save_workflow(
        &mut vault,
        Some(&workflow.id),
        draft,
        now_secs(),
    )
    .unwrap()
    .workflow;
    let report = crate::usecases::workflow::runs::run_workflow_now(
        &mut vault,
        &workflow.id,
        None,
        later(),
        "UTC",
    )
    .unwrap();
    assert!(
        matches!(report.run.outcome, RunOutcome::Rejected { .. }),
        "{:?}",
        report.run.outcome
    );
    assert!(crate::workflow::service::runs::queue(&vault.doc).is_empty());
}

// -- C2-5 and the lock --------------------------------------------------------

/// A host that keeps the vault behind a lock: every call takes it, uses it and
/// gives it back before returning. A call that comes while it is held panics,
/// as a second take of a real lock would block forever.
struct LockedHost {
    vault: Mutex<Session>,
    takes: RefCell<Vec<&'static str>>,
}

impl LockedHost {
    fn new(vault: Session) -> Self {
        Self {
            vault: Mutex::new(vault),
            takes: RefCell::new(Vec::new()),
        }
    }

    fn take(&self, call: &'static str) -> std::sync::MutexGuard<'_, Session> {
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
        fn migrate_export_graph(&self, json: &str) -> Option<String>;
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
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        income(&card, "100", "opening"),
    )
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
