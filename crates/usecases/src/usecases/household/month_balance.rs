//! Scenarios of the month_balance area: the balance is a read, the carry switch
//! decides in `money::month_balance` and ends in one `vault.transact`. The
//! reports area has not moved: the vault serves it through `ReportSource` (OD6).

use chrono::NaiveDate;

use crate::Amount;
use crate::error::Result;
use crate::money::month_balance::{
    CashMonth, DayFlow, FlowKind, MonthBalance, ReportFlows, ReportSource,
    month_balance as balance, parse_carry_since,
};
use crate::money::worth::FlowClass;
use crate::service::Session;
use mwt_planning::flows::Basis;
use mwt_reports::overview::ReportFilter;

/// The vault as the month balance's source for the reports area: it holds the
/// filter, the moment and the zone the balance was asked for.
struct VaultReports<'ask> {
    vault: &'ask Session,
    filter: &'ask ReportFilter,
    now: i64,
    zone: &'ask str,
}

impl ReportSource for VaultReports<'_> {
    fn shown_months(&self) -> Result<Vec<NaiveDate>> {
        Ok(mwt_reports::overview::report_ctx(
            &self.vault.books(),
            self.filter,
            self.now,
            self.zone,
        )?
        .months)
    }

    fn report(&self, from: &str, to: &str) -> Result<ReportFlows> {
        let mut wide = self.filter.clone();
        wide.from = Some(from.to_string());
        wide.to = Some(to.to_string());
        let ctx =
            mwt_reports::overview::report_ctx(&self.vault.books(), &wide, self.now, self.zone)?;
        let cash = mwt_reports::overview::cashflow_report(&self.vault.books(), &ctx);
        let last_month = match ctx.months.last() {
            Some(last) => {
                let (start, end) = ctx.cal.month_bounds(*last);
                mwt_reports::overview::report_flows(&self.vault.books(), &ctx, start, end)
                    .into_iter()
                    .filter_map(|flow| {
                        let kind = match flow.class {
                            FlowClass::Income => FlowKind::Income,
                            FlowClass::Fixed | FlowClass::Daily => FlowKind::Spending,
                            _ => return None,
                        };
                        Some(DayFlow {
                            date: flow.date,
                            value: ctx.shown(flow.value, *last),
                            kind,
                            category: flow.categories.first().cloned().unwrap_or_default(),
                        })
                    })
                    .collect()
            }
            None => Vec::new(),
        };
        Ok(ReportFlows {
            currency: cash.meta.currency,
            today: ctx.today,
            months: ctx.months,
            cash: cash
                .rows
                .into_iter()
                .map(|row| CashMonth {
                    month: row.month,
                    income: row.income,
                    fixed: row.fixed,
                    daily: row.daily,
                })
                .collect(),
            last_month,
        })
    }

    fn allowance(&self, month: &str, currency: &str) -> Result<Option<Amount>> {
        let calendar_month =
            mwt_reports::overview::calendar_month(&self.vault.books(), month, self.now, self.zone)?;
        Ok(calendar_month
            .allowance
            .filter(|_| calendar_month.currency == currency))
    }
}

/// The balance of the filter's months (the last twelve by default). The
/// carry starts at `settings.carry_since`, even before the period does. The
/// balance is a time-bound reader: it counts by the money date whatever the
/// filter's basis (R3).
pub fn month_balance(
    vault: &Session,
    filter: &ReportFilter,
    now: i64,
    zone: &str,
) -> Result<MonthBalance> {
    let money_date = ReportFilter {
        basis: Some(Basis::MoneyDate),
        ..filter.clone()
    };
    let source = VaultReports {
        vault,
        filter: &money_date,
        now,
        zone,
    };
    balance(&vault.books(), &source)
}

/// Carry the unspent over from `month` (`YYYY-MM`), or stop carrying (`None`).
pub fn set_carry_over(vault: &mut Session, month: Option<&str>) -> Result<()> {
    let since = parse_carry_since(month)?;
    if vault.doc.settings.carry_since == since {
        return Ok(());
    }
    vault.transact(|tx| {
        tx.doc_mut().settings.carry_since = since;
        Ok(())
    })
}
