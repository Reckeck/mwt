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
