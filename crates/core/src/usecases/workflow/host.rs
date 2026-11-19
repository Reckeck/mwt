//! The vault as the host of a workflow run: every lookup and every `Effect`
//! the executor asks for, answered from `UnlockedVault` (spec core-strangler,
//! F6). The executor itself does not know the vault.

use std::collections::HashSet;

use chrono::NaiveDate;

use crate::Amount;
use crate::model::datasource::DataSource;
use crate::model::inflation::InflationReading;
use crate::model::planning::{RejectReason, Reminder};
use crate::model::store::{AssetRef, StoreScope};
use crate::model::transaction::Transaction;
use crate::service::UnlockedVault;
use crate::workflow::host::{
    BudgetReading, Checkpoint, EmergencyReading, GoalReading, RecurringReading, TopupBasket,
    TopupReading, WorkflowHost,
};
use crate::workflow::model::{VarScope, Workflow};
use crate::workflow::types::WfValue;

impl WorkflowHost for UnlockedVault {
    fn base_currency(&self) -> String {
        UnlockedVault::base_currency(self).to_string()
    }

    fn wf_asset(&self, symbol: &str) -> AssetRef {
        UnlockedVault::wf_asset(self, symbol)
    }

    fn wf_balance(&self, store_id: &str, asset: Option<&AssetRef>) -> Result<WfValue, String> {
        UnlockedVault::wf_balance(self, store_id, asset)
    }

    fn wf_price(&self, asset: &AssetRef, currency: &str, secs: i64) -> Option<Amount> {
        UnlockedVault::wf_price(self, asset, currency, secs)
    }

    fn wf_convert(&self, amount: Amount, from: &str, to: &str, secs: i64) -> Option<Amount> {
        UnlockedVault::wf_convert(self, amount, from, to, secs)
    }

    fn wf_sum(
        &self,
        period: &str,
        flow: &str,
        category: Option<&str>,
        now: i64,
        zone: &str,
    ) -> Amount {
        UnlockedVault::wf_sum(self, period, flow, category, now, zone)
    }

    fn wf_recent_average(&self, workflow_id: &str, count: usize) -> Option<(Amount, String)> {
        UnlockedVault::wf_recent_average(self, workflow_id, count)
    }

    fn wf_accounts(&self, parent: Option<&str>) -> Vec<(String, String, Amount)> {
        UnlockedVault::wf_accounts(self, parent)
    }

    fn wf_outflow_base(&self, tx: &Transaction) -> Amount {
        UnlockedVault::wf_outflow_base(self, tx)
    }

    fn wf_outflow_in(&self, tx: &Transaction, currency: &str) -> Option<Amount> {
        UnlockedVault::wf_outflow_in(self, tx, currency)
    }

    fn wf_mark(&self, workflow_id: &str, key: &str) -> Option<String> {
        UnlockedVault::wf_mark(self, workflow_id, key)
    }

    fn wf_var(&self, scope: &VarScope, name: &str) -> Option<WfValue> {
        UnlockedVault::wf_var(self, scope, name)
    }

    fn store_name(&self, store_id: &str) -> Option<String> {
        self.find_store(store_id).map(|store| store.name.clone())
    }

    fn contact_name(&self, contact_id: &str) -> Option<String> {
        self.doc
            .contacts
            .iter()
            .find(|contact| contact.id == contact_id)
            .map(|contact| contact.name.clone())
    }

    fn callee(&self, workflow_id: &str) -> Option<Workflow> {
        self.doc
            .workflows
            .iter()
            .find(|workflow| workflow.id == workflow_id)
            .cloned()
    }

    fn funds_shortfall(&self, tx: &Transaction, secs: i64) -> Option<RejectReason> {
        UnlockedVault::funds_shortfall(self, tx, secs)
    }

    fn resolve_scope(&self, scope: &StoreScope) -> HashSet<String> {
        UnlockedVault::resolve_scope(self, scope)
    }

    fn key_rate_on(&self, day: NaiveDate) -> Option<Amount> {
        UnlockedVault::key_rate_on(self, day)
    }

    fn inflation_series(&self, currency: &str) -> Vec<InflationReading> {
        UnlockedVault::inflation_series(self, currency)
    }

    fn inflation_on(&self, currency: &str, month: &str) -> Option<Amount> {
        UnlockedVault::inflation_on(self, currency, month)
    }

    fn recurring_check(&self, tx: &Transaction) -> RecurringReading {
        let check = UnlockedVault::recurring_check(self, tx);
        RecurringReading {
            new: check.new,
            grew: check.grew,
            previous: check.previous,
            change_pct: check.change_pct,
        }
    }

    fn topup_advice(&self, amount: Option<Amount>, now: i64, zone: &str) -> TopupReading {
        let advice = UnlockedVault::topup_advice(self, amount, now, zone);
        TopupReading {
            currency: advice.currency,
            amount: advice.amount,
            baskets: advice
                .rows
                .into_iter()
                .map(|row| TopupBasket {
                    name: row.class.name,
                    add: row.add,
                    capped: row.capped,
                })
                .collect(),
        }
    }

    fn emergency_fund_status(&self, now: i64, zone: &str) -> EmergencyReading {
        let status = UnlockedVault::emergency_fund_status(self, now, zone);
        EmergencyReading {
            currency: status.currency,
            target_amount: status.target_amount,
            total: status.total,
            first_store_id: status.stores.first().map(|store| store.store_id.clone()),
        }
    }

    fn budget_status(&self, now: i64, zone: &str) -> Vec<BudgetReading> {
        UnlockedVault::budget_status(self, now, zone)
            .into_iter()
            .map(|status| BudgetReading {
                id: status.budget.id,
                currency: status.budget.currency,
                remaining: status.remaining,
                used: status.used,
                period_start: status.period_start,
                period_end: status.period_end,
            })
            .collect()
    }

    fn data_source(&self, id: &str) -> Option<DataSource> {
        UnlockedVault::data_source(self, id).cloned()
    }

    fn goal_progress(&self, now: i64, zone: &str) -> Vec<GoalReading> {
        UnlockedVault::goal_progress(self, now, zone)
            .into_iter()
            .map(|progress| GoalReading {
                id: progress.goal.id,
                name: progress.goal.name,
                currency: progress.goal.currency,
                target_amount: progress.goal.target_amount,
                saved: progress.saved,
                pct: progress.pct,
            })
            .collect()
    }

    fn insert_transactions(
        &mut self,
        txs: Vec<Transaction>,
        now: i64,
    ) -> Result<Vec<Transaction>, String> {
        self.wf_insert(txs, now)
    }

    fn set_mark(&mut self, workflow_id: &str, key: &str, value: &str) {
        self.wf_set_mark(workflow_id, key, value);
    }

    fn set_var(&mut self, scope: VarScope, name: &str, value: &WfValue) {
        self.wf_set_var(scope, name, value);
    }

    fn add_reminder(&mut self, reminder: Reminder) {
        self.doc.reminders.push(reminder);
    }

    fn checkpoint(&self) -> Checkpoint {
        Checkpoint(self.doc.clone())
    }

    fn restore(&mut self, checkpoint: Checkpoint) {
        self.doc = checkpoint.0;
    }
}
