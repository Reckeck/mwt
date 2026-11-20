//! Shim of the `budget` area: the rules moved to `crate::planning::budget`, the scenarios and the
//! flow queries to `crate::planning::flows` (spec core-strangler, D1).

pub use crate::planning::budget::{
    BudgetDraft, BudgetStatus, DailyBudget, MIN_HISTORY_MONTHS, MonthAvg, MonthSummary,
};
pub(crate) use crate::planning::flows::Flow;

use std::collections::HashSet;

use crate::Amount;
#[cfg(test)]
use crate::calendar::Calendar;
use crate::error::Result;
use crate::kernel::books::Books;
use crate::model::planning::Budget;
use crate::model::store::StoreScope;
use crate::model::transaction::Leg;

use super::UnlockedVault;
use super::forward::forward;
use super::rates::RateBook;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// The budget currency, never empty (D20).
        pub fn budget_currency(&self) -> String
            => crate::planning::budget::budget_currency @books;
        pub fn budgets(&self) -> Vec<Budget>
            => crate::planning::budget::budgets @doc;
        /// Whether a scope belongs to the budget: the form's over-limit hint.
        pub fn scope_in_budget(&self, scope: &StoreScope) -> bool
            => crate::planning::budget::scope_in_budget @books;
        /// The user's own month from the settings (plan B): income and spending,
        /// `months = 0`. `None` without a hand-entered income.
        pub fn manual_month(&self) -> Option<MonthAvg>
            => crate::planning::budget::manual_month @books;
        pub fn create_budget(&mut self, draft: BudgetDraft) -> Result<Budget>
            => crate::usecases::planning::budget::create_budget;
        pub fn update_budget(&mut self, id: &str, draft: BudgetDraft) -> Result<Budget>
            => crate::usecases::planning::budget::update_budget;
        /// Delete a limit; its notices go with it, each with a tombstone, or another
        /// device would send them back.
        pub fn delete_budget(&mut self, id: &str) -> Result<()>
            => crate::usecases::planning::budget::delete_budget;
        /// Every active limit with what its current period used (plan §7.3):
        /// mandatory and everyday spending of the budget stores, by category, by
        /// wallet or in total, in the limit's own currency at each day's rate.
        pub fn budget_status(&self, now: i64, zone: &str) -> Vec<BudgetStatus>
            => crate::usecases::planning::budget::budget_status @books;
        /// Income, mandatory spending, everyday spending and savings of a calendar
        /// month (`YYYY-MM`) in the budget currency (plan §7.3).
        pub fn month_summary(&self, month: &str, zone: &str) -> Result<MonthSummary>
            => crate::usecases::planning::budget::month_summary @books;
        /// Averages of the last `months` finished calendar months that had any
        /// flow, before the month of `now`; `None` without such months.
        pub fn avg_month(&self, months: usize, now: i64, zone: &str) -> Option<MonthAvg>
            => crate::planning::budget::avg_month @books;
        /// [`avg_month`], or the hand-entered month while fewer than
        /// `MIN_HISTORY_MONTHS` finished months have flows.
        pub fn income_basis(&self, months: usize, now: i64, zone: &str) -> Option<MonthAvg>
            => crate::planning::budget::income_basis @books;
        /// How much can be spent today (plan §7.3), in the budget currency.
        pub fn daily_budget(&self, now: i64, zone: &str) -> DailyBudget
            => crate::planning::daily_budget::daily_budget @books;
    }
    // forward:end

    // Read by other areas until they move; not in the baseline (crate-private), so outside the
    // regenerated block.
    forward! {
        /// Classified flows in `[from, to)`, valued in `currency`.
        #[cfg(test)]
        pub(crate) fn flows_between(&self, cal: &Calendar, currency: &str, from: i64, to: i64) -> Vec<Flow>
            => crate::planning::flows::flows_between @books;
        /// Operations the budget could not value in its currency: a side had no
        /// rate (Currency-1 · 7, "Show").
        pub(crate) fn unpriced_tx_ids(&self) -> HashSet<String>
            => crate::usecases::planning::budget::unpriced_tx_ids @books;
        /// The currency a store mostly holds, for terms entered without one.
        pub(crate) fn store_currency(&self, store_id: &str) -> String
            => crate::planning::budget::store_currency @books;
    }

    /// [`Self::month_summary`] over the books of one request.
    pub(crate) fn month_summary_in(
        &self,
        books: &Books,
        month: &str,
        zone: &str,
    ) -> Result<MonthSummary> {
        crate::usecases::planning::budget::month_summary(books, month, zone)
    }

    /// Value of one leg in `currency` at `secs`; `None` without a rate.
    pub(crate) fn leg_value_in(
        book: &RateBook,
        leg: &Leg,
        currency: &str,
        secs: i64,
    ) -> Option<Amount> {
        crate::planning::flows::leg_value_in(book, leg, currency, secs)
    }
}
