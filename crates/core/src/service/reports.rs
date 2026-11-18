//! Shim of the `reports` area: moved to `crate::reports::overview` (spec core-strangler, D1).
//! The reports take `&Books`; the delegates below keep the old `UnlockedVault` methods that
//! `insights`, `where_went`, `losses` and the household scenarios still call.

pub use crate::reports::overview::{
    CalendarDay, CalendarMonth, CashflowReport, CashflowRow, CategoriesReport, CategoryGroupBlock,
    CategoryRow, DayStatus, InflationPoint, InflationReport, NetworthPoint, NetworthReport,
    PnlReport, PnlRow, PortfolioPoint, PortfolioReport, Report, ReportFilter, ReportKind,
    ReportMeta, StoreReportRow, StoresReport, UnpricedSpending, YearReport, YearRow, report_csv,
};
pub(crate) use crate::reports::overview::{Ctx, csv_text};

use crate::Amount;
use crate::error::Result;
use crate::model::store::AssetRef;

use super::RateBook;
use super::UnlockedVault;
use super::budget::Flow;
use super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// One report of a kind for a filter. `inflation` is the user's
        /// reference table (`AppSettings.inflation`).
        pub fn report(&self, kind: ReportKind, filter: &ReportFilter, now: i64, zone: &str) -> Result<Report>
            => crate::reports::overview::report @books;
        /// Spending by day of a month (`YYYY-MM`) against the day's allowance,
        /// with what is still to come (plan F6).
        pub fn calendar_month(&self, month: &str, now: i64, zone: &str) -> Result<CalendarMonth>
            => crate::reports::overview::calendar_month @books;
        pub(crate) fn report_ctx(&self, filter: &ReportFilter, now: i64, zone: &str) -> Result<Ctx>
            => crate::reports::overview::report_ctx @books;
        /// Classified flows of the report range (or `[from, to)`), in scope.
        pub(crate) fn report_flows(&self, ctx: &Ctx, from: i64, to: i64) -> Vec<Flow>
            => crate::reports::overview::report_flows @books;
        pub(crate) fn cashflow_report(&self, ctx: &Ctx) -> CashflowReport
            => crate::reports::overview::cashflow_report @books;
        pub(crate) fn year_report(&self, ctx: &Ctx) -> YearReport
            => crate::reports::overview::year_report @books;
        pub(crate) fn categories_report(&self, ctx: &Ctx) -> CategoriesReport
            => crate::reports::overview::categories_report @books;
        pub(crate) fn networth_report(&self, ctx: &Ctx) -> NetworthReport
            => crate::reports::overview::networth_report @books;
        pub(crate) fn pnl_report(&self, ctx: &Ctx) -> PnlReport
            => crate::reports::overview::pnl_report @books;
    }
    // forward:end

    /// Value of a quantity in `currency` at `secs`; zero without a rate. An associated function
    /// (no receiver), so it sits outside `forward!`.
    pub(crate) fn value_at(
        book: &RateBook,
        asset: &AssetRef,
        qty: Amount,
        currency: &str,
        secs: i64,
    ) -> Amount {
        crate::reports::overview::value_at(book, asset, qty, currency, secs)
    }
}

/// What the crypto module was worth at each cutoff in USD (`None`: no record that month) and
/// whether it already counts a replayed wallet's coins. `CryptoHistory` and its readers are
/// visible only inside `service`, so this adapter stays here until stream E opens them.
pub(crate) fn crypto_module(
    vault: &UnlockedVault,
    cutoffs: &[i64],
    in_scope: impl Fn(&str) -> bool,
) -> (Vec<Option<Amount>>, impl Fn(&str, &AssetRef) -> bool) {
    let history = vault.crypto_history(cutoffs, in_scope);
    let values = (0..cutoffs.len())
        .map(|index| history.value_usd(index))
        .collect();
    (values, move |store_id, asset| {
        history.supersedes(store_id, asset)
    })
}
