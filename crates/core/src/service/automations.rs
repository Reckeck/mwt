//! Shim of the `automations` area: the rules moved to `crate::planning::automations`, the
//! scenarios to `crate::usecases::planning::automations` (spec core-strangler, D1).

pub(super) use crate::planning::automations::outflow;
pub use crate::planning::automations::{
    AutomationDraft, AutomationSimulation, MAX_CATCH_UP, SimulatedBalance, SimulatedRun,
    describe_reject,
};
pub(crate) use crate::planning::automations::{draft_to_transaction, main_amount};

use crate::Amount;
use crate::calendar::Calendar;
use crate::error::Result;
use crate::model::planning::{Automation, AutomationRun, RejectReason, TransactionDraft};
use crate::model::transaction::Transaction;
use crate::workflow::model::Workflow;

use super::UnlockedVault;
use super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        pub fn automations(&self) -> Vec<Automation>
            => crate::usecases::planning::automations::automations;
        pub fn automation(&self, id: &str) -> Result<Automation>
            => crate::usecases::planning::automations::automation;
        pub fn create_automation(&mut self, draft: AutomationDraft) -> Result<Automation>
            => crate::usecases::planning::automations::create_automation;
        /// Change what an automation does: its graph is built again, the old one
        /// kept as a version. Dates already processed stay processed.
        pub fn update_automation(&mut self, id: &str, draft: AutomationDraft) -> Result<Automation>
            => crate::usecases::planning::automations::update_automation;
        /// [`update_automation`] with the edit moment given by the caller.
        pub fn update_automation_at(&mut self, id: &str, draft: AutomationDraft, edited_at: i64) -> Result<Automation>
            => crate::usecases::planning::automations::update_automation_at;
        /// Remove an automation and its run history. Transactions it created stay.
        pub fn delete_automation(&mut self, id: &str) -> Result<()>
            => crate::usecases::planning::automations::delete_automation;
        /// Make an automation an ordinary workflow: the graph stays and is edited
        /// in the editor from now on; the form no longer shows it.
        pub fn detach_automation(&mut self, id: &str) -> Result<Workflow>
            => crate::usecases::planning::automations::detach_automation;
        /// Emulate the automations over the next `days` (one of them, or all):
        /// day by day on a sandbox copy, so each run sees the balance the earlier
        /// ones left, exactly as the scheduler would. The vault is not touched.
        pub fn simulate_automations(&self, automation_id: Option<&str>, days: u32, now: i64, zone: &str) -> Result<AutomationSimulation>
            => crate::usecases::planning::automations::simulate_automations;
        /// Create the transaction of an automation now ("Create now"), with
        /// the same funds check as the schedule.
        pub fn run_automation(&mut self, id: &str, now: i64) -> Result<Transaction>
            => crate::usecases::planning::automations::run_automation;
        /// Retry a refused run: funds are checked at the current moment and the
        /// transaction is dated now.
        pub fn retry_automation_run(&mut self, run_id: &str, now: i64, zone: &str) -> Result<Transaction>
            => crate::usecases::planning::automations::retry_automation_run;
        /// Dismiss a refused run.
        pub fn skip_automation_run(&mut self, run_id: &str) -> Result<AutomationRun>
            => crate::usecases::planning::automations::skip_automation_run;
        /// Put in the real amount of an operation booked approximately: it is
        /// corrected with the usual checks and counts toward the next estimate.
        pub fn confirm_estimated_amount(&mut self, tx_id: &str, amount: Amount) -> Result<Transaction>
            => crate::usecases::planning::automations::confirm_estimated_amount;
        /// Run history, newest planned date first.
        pub fn automation_runs(&self, automation_id: Option<&str>) -> Vec<AutomationRun>
            => crate::usecases::planning::automation_rules::automation_runs @doc;
    }
    // forward:end

    /// The template the next run books: with an amount that differs, at the
    /// current estimate.
    pub(crate) fn automation_draft(&self, automation: &Automation) -> TransactionDraft {
        crate::planning::automations::automation_draft(automation)
    }

    /// The planned moments of an automation in `(after, until]`, with the
    /// operation each would make.
    pub(crate) fn automation_occurrences(
        &self,
        automation: &Automation,
        cal: &Calendar,
        after: i64,
        until: i64,
    ) -> Vec<(i64, Transaction)> {
        crate::usecases::planning::automation_rules::automation_occurrences(
            automation, cal, after, until,
        )
    }

    // Not in the baseline (crate-private), so outside the regenerated block.
    forward! {
        /// Quantity of an asset on one store from transactions up to `secs`.
        pub(crate) fn balance_at(&self, store_id: &str, asset_key: &str, secs: i64) -> Amount
            => crate::planning::automations::balance_at @doc;
        /// Whether the source of a transaction can pay for it at `secs`; `None` when it can.
        pub(crate) fn funds_shortfall(&self, tx: &Transaction, secs: i64) -> Option<RejectReason>
            => crate::planning::automations::funds_shortfall @doc;
        /// «Card: 500 USD of 800 USD» — with the store's name, for the error text.
        pub(crate) fn describe_shortfall(&self, reason: &RejectReason) -> String
            => crate::planning::automations::describe_shortfall @doc;
    }
}
