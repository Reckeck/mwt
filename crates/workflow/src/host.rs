//! What a workflow run needs from the vault, as a trait (spec core-strangler,
//! F6, `target-structure` §6). The executor and the nodes know only this: the
//! vault implements it in `usecases::workflow::host`, a test implements it with
//! a fake. Every lookup returns an owned value, so a host that guards the vault
//! with a lock takes it for one call and gives it back before the node goes on.

use chrono::NaiveDate;

use mwt_base::Amount;
use mwt_model::DataSource;
use mwt_model::InflationReading;
use mwt_model::Transaction;
use mwt_model::VaultDoc;
use mwt_model::{AssetRef, StoreScope};
use mwt_model::{RejectReason, Reminder};

use super::model::{VarScope, Workflow};
use super::types::WfValue;

/// The state of the host at one moment, to be put back when a branch fails.
#[derive(Debug, Clone)]
pub struct Checkpoint(pub VaultDoc);

/// What a "Limit" node reads of a budget: how much is left and used.
#[derive(Debug, Clone, PartialEq)]
pub struct BudgetReading {
    pub id: String,
    pub currency: String,
    pub remaining: Amount,
    pub used: Amount,
    pub period_start: String,
    pub period_end: String,
}

/// What a goal node reads of a goal.
#[derive(Debug, Clone, PartialEq)]
pub struct GoalReading {
    pub id: String,
    pub name: String,
    pub currency: String,
    pub target_amount: Amount,
    pub saved: Amount,
    pub pct: Amount,
}

/// What "Emergency fund gap" reads of the fund.
#[derive(Debug, Clone, PartialEq)]
pub struct EmergencyReading {
    pub currency: String,
    pub target_amount: Option<Amount>,
    pub total: Amount,
    /// The first account of the fund.
    pub first_store_id: Option<String>,
}

/// What "Recurring payment" reads of the history of one payment.
#[derive(Debug, Clone, PartialEq)]
pub struct RecurringReading {
    pub new: bool,
    pub grew: bool,
    pub previous: Option<Amount>,
    pub change_pct: Option<Amount>,
}

/// One basket of the top-up advice.
#[derive(Debug, Clone, PartialEq)]
pub struct TopupBasket {
    pub name: String,
    pub add: Amount,
    pub capped: bool,
}

/// What "Top-up advice" reads of the advice.
#[derive(Debug, Clone, PartialEq)]
pub struct TopupReading {
    pub currency: String,
    pub amount: Amount,
    pub baskets: Vec<TopupBasket>,
}

pub trait WorkflowHost {
    // -- lookups ------------------------------------------------------------------

    fn base_currency(&self) -> String;
    /// An asset by its symbol: one the host holds, else a currency.
    fn wf_asset(&self, symbol: &str) -> AssetRef;
    fn wf_balance(&self, store_id: &str, asset: Option<&AssetRef>) -> Result<WfValue, String>;
    fn wf_price(&self, asset: &AssetRef, currency: &str, secs: i64) -> Option<Amount>;
    fn wf_convert(&self, amount: Amount, from: &str, to: &str, secs: i64) -> Option<Amount>;
    fn wf_sum(
        &self,
        period: &str,
        flow: &str,
        category: Option<&str>,
        now: i64,
        zone: &str,
    ) -> Amount;
    fn wf_recent_average(&self, workflow_id: &str, count: usize) -> Option<(Amount, String)>;
    fn wf_accounts(&self, parent: Option<&str>) -> Vec<(String, String, Amount)>;
    fn wf_outflow_base(&self, tx: &Transaction) -> Amount;
    fn wf_outflow_in(&self, tx: &Transaction, currency: &str) -> Option<Amount>;
    fn wf_mark(&self, workflow_id: &str, key: &str) -> Option<String>;
    fn wf_var(&self, scope: &VarScope, name: &str) -> Option<WfValue>;
    fn store_name(&self, store_id: &str) -> Option<String>;
    fn contact_name(&self, contact_id: &str) -> Option<String>;
    /// The workflow a "Call scheme" node names.
    fn callee(&self, workflow_id: &str) -> Option<Workflow>;
    fn funds_shortfall(&self, tx: &Transaction, secs: i64) -> Option<RejectReason>;
    fn resolve_scope(&self, scope: &StoreScope) -> std::collections::HashSet<String>;
    fn key_rate_on(&self, day: NaiveDate) -> Option<Amount>;
    fn inflation_series(&self, currency: &str) -> Vec<InflationReading>;
    fn inflation_on(&self, currency: &str, month: &str) -> Option<Amount>;
    fn recurring_check(&self, tx: &Transaction) -> RecurringReading;
    fn topup_advice(&self, amount: Option<Amount>, now: i64, zone: &str) -> TopupReading;
    fn emergency_fund_status(&self, now: i64, zone: &str) -> EmergencyReading;
    fn budget_status(&self, now: i64, zone: &str) -> Vec<BudgetReading>;
    fn data_source(&self, id: &str) -> Option<DataSource>;
    fn goal_progress(&self, now: i64, zone: &str) -> Vec<GoalReading>;

    // -- what an `Effect` does to the host (see `effect.rs`) -----------------------

    /// Check money and rules, then add the operations; all of them or none.
    fn insert_transactions(
        &mut self,
        txs: Vec<Transaction>,
        now: i64,
    ) -> Result<Vec<Transaction>, String>;
    fn set_mark(&mut self, workflow_id: &str, key: &str, value: &str);
    fn set_var(&mut self, scope: VarScope, name: &str, value: &WfValue);
    fn add_reminder(&mut self, reminder: Reminder);

    /// An exported graph of an older format brought to the current one; `None` when it needs no change.
    fn migrate_export_graph(&self, json: &str) -> Option<String>;

    // -- undo ---------------------------------------------------------------------

    fn checkpoint(&self) -> Checkpoint;
    fn restore(&mut self, checkpoint: Checkpoint);
}
