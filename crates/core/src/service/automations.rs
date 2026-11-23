//! Shim of the `automations` area: the rules moved to `crate::planning::automations`, the
//! scenarios to `crate::usecases::planning::automations` (spec core-strangler, D1).

pub use crate::planning::automations::{
    AutomationDraft, AutomationSimulation, MAX_CATCH_UP, SimulatedBalance, SimulatedRun,
    describe_reject,
};

use crate::calendar::Calendar;
use crate::model::planning::Automation;
use crate::model::transaction::Transaction;

use super::UnlockedVault;

impl UnlockedVault {
    /// The template the next run books: with an amount that differs, at the
    /// current estimate. Only the automation tests read it; the budget calls
    /// the rule directly.
    #[cfg(test)]
    pub(crate) fn automation_draft(
        &self,
        automation: &Automation,
    ) -> crate::model::planning::TransactionDraft {
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
        crate::planning::automation_plan::automation_occurrences(automation, cal, after, until)
    }
}
