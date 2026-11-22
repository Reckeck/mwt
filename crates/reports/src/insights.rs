//! What a report says in words, and the analytics overview.
//!
//! A report gets a few [`Insight`]s: a code and the numbers behind it. The
//! sentence itself is built by the UI from its locale, so one set of facts
//! reads in every language (and can later be handed to a model as is).

use chrono::{Datelike, Duration, NaiveDate};

use mwt_base::Amount;
use mwt_base::calendar::{self, first_of_month};
use mwt_base::error::Result;
use mwt_model::CategoryGroup;

use crate::health::lifestyle_creep;
use mwt_money::books::Books;
use mwt_money::portfolio::realized_pnl_filtered;
use mwt_planning::budget::budget_currency;

use super::overview::{
    CashflowReport, CategoriesReport, InflationReport, LossReport, NetworthReport, PnlReport,
    PortfolioReport, Report, ReportFilter, StoresReport, YearReport, cashflow_report,
    categories_report, loss_report, networth_report, pnl_report, report_ctx, year_report,
};

/// A category must change at least this much (percent) to be named as grown.
const NOTABLE_GROWTH_PCT: i64 = 10;
/// Net worth moving less than this (percent) reads as «about the same».
const FLAT_PCT: i64 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum InsightCode {
    /// Spent more than earned: `pct` over the income, `amount` the gap.
    Overspent,
    /// Kept part of the income: `pct` of it, `amount` kept.
    Saved,
    /// Spent with no income at all: `amount` spent.
    NoIncome,
    /// The month that cost most: `subject` = `YYYY-MM`, `amount` spent.
    CostliestMonth,
    /// Spending against the month before: `pct` up.
    SpentUp,
    /// Spending against the month before: `pct` down.
    SpentDown,
    /// The biggest expense: `subject` = category key, `amount`, `pct` share.
    TopCategory,
    /// A category grew most against the previous period: `subject`, `pct`.
    CategoryGrew,
    /// Kept less than the savings plan: `amount` short.
    BelowPlan,
    /// The month that kept most: `subject` = `YYYY-MM`, `amount` kept.
    BestMonth,
    /// Net worth grew: `amount`, `pct`.
    WealthUp,
    /// Net worth fell: `amount` (positive), `pct` (positive).
    WealthDown,
    /// Net worth about the same: `amount` = now.
    WealthFlat,
    /// Debts and loans owed: `amount`.
    Owes,
    /// The wallet that grew most: `subject` = wallet name, `amount`.
    StoreGrewMost,
    /// Realized profit: `amount`.
    Profit,
    /// Realized loss: `amount` (positive).
    Loss,
    /// Fees paid: `amount`.
    FeesPaid,
    /// Lost on conversions and fees: `amount`, `pct` of it in fees.
    ConversionLosses,
    /// The biggest holding: `subject` = asset, `pct` share.
    TopAsset,
    /// Net in today's money changed: `pct` real, `amount` = nominal percent.
    RealChange,
    /// Today's money asked for but no inflation set for the currency.
    NoInflationRate,
    /// Spending grew as fast as income or faster (plan C4): `amount` the
    /// monthly growth of spending, `pct` of the income growth, `subject` =
    /// `year` (against the same months a year back) or `previous`.
    LifestyleCreep,
}

/// One fact a report states; which fields are set depends on the code.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Insight {
    pub code: InsightCode,
    #[serde(default)]
    pub amount: Option<Amount>,
    #[serde(default)]
    pub pct: Option<Amount>,
    /// What the sentence names: a category key, a month, a wallet, an asset.
    #[serde(default)]
    pub subject: Option<String>,
}

impl Insight {
    fn new(code: InsightCode) -> Self {
        Self {
            code,
            amount: None,
            pct: None,
            subject: None,
        }
    }

    fn amount(mut self, value: Amount) -> Self {
        self.amount = Some(value.round(2));
        self
    }

    fn pct(mut self, value: Amount) -> Self {
        self.pct = Some(value.round(1));
        self
    }

    fn subject(mut self, value: impl Into<String>) -> Self {
        self.subject = Some(value.into());
        self
    }
}

fn hundred() -> Amount {
    Amount::new(rust_decimal::Decimal::from(100))
}

fn whole(value: i64) -> Amount {
    Amount::new(rust_decimal::Decimal::from(value))
}

/// `part` as a percent of `of`; `None` when `of` is zero.
fn share(part: Amount, of: Amount) -> Option<Amount> {
    (!of.is_zero()).then(|| (part / of.abs() * hundred()).round(1))
}

/// Facts of any report.
pub fn report_insights(report: &Report) -> Vec<Insight> {
    match report {
        Report::Cashflow(report) => cashflow_insights(report),
        Report::Year(report) => year_insights(report),
        Report::Categories(report) => categories_insights(report),
        Report::Networth(report) => networth_insights(report),
        Report::Stores(report) => stores_insights(report),
        Report::Pnl(report) => pnl_insights(report),
        Report::Portfolio(report) => portfolio_insights(report),
        Report::Inflation(report) => inflation_insights(report),
        Report::Losses(report) => loss_insights(report),
        Report::Where(_) => Vec::new(),
    }
}

/// Earned against spent over a period.
pub fn balance_insight(income: Amount, spent: Amount) -> Option<Insight> {
    if income.is_zero() && spent.is_zero() {
        return None;
    }
    if income.is_zero() {
        return Some(Insight::new(InsightCode::NoIncome).amount(spent));
    }
    let kept = income - spent;
    let pct = share(kept.abs(), income).unwrap_or(Amount::ZERO);
    Some(if kept.is_negative() {
        Insight::new(InsightCode::Overspent).amount(-kept).pct(pct)
    } else {
        Insight::new(InsightCode::Saved).amount(kept).pct(pct)
    })
}

pub(crate) fn cashflow_insights(report: &CashflowReport) -> Vec<Insight> {
    let total = &report.total;
    let mut out: Vec<Insight> = balance_insight(total.income, total.fixed + total.daily)
        .into_iter()
        .collect();
    if report.rows.len() > 1 {
        // Ties go to the earlier month.
        let costliest = report
            .rows
            .iter()
            .rev()
            .max_by(|left, right| (left.fixed + left.daily).cmp(&(right.fixed + right.daily)))
            .filter(|row| !(row.fixed + row.daily).is_zero());
        if let Some(row) = costliest {
            out.push(
                Insight::new(InsightCode::CostliestMonth)
                    .subject(row.month.clone())
                    .amount(row.fixed + row.daily),
            );
        }
    }
    out
}

pub(crate) fn year_insights(report: &YearReport) -> Vec<Insight> {
    let total = &report.total;
    let mut out: Vec<Insight> = balance_insight(total.income, total.fixed + total.daily)
        .into_iter()
        .collect();
    if total.savings_plan.is_positive() && total.deviation.is_negative() {
        out.push(Insight::new(InsightCode::BelowPlan).amount(-total.deviation));
    }
    let best = report
        .best_month
        .as_ref()
        .and_then(|month| report.rows.iter().find(|row| &row.month == month))
        .filter(|row| row.savings_fact.is_positive());
    if let Some(row) = best {
        out.push(
            Insight::new(InsightCode::BestMonth)
                .subject(row.month.clone())
                .amount(row.savings_fact),
        );
    }
    out
}

pub(crate) fn categories_insights(report: &CategoriesReport) -> Vec<Insight> {
    let mut out = Vec::new();
    let spending: Amount = report
        .groups
        .iter()
        .filter(|block| {
            matches!(
                block.group,
                Some(CategoryGroup::Fixed | CategoryGroup::Daily)
            )
        })
        .fold(Amount::ZERO, |sum, block| sum + block.total);
    if let Some(top) = report.top.first() {
        let mut fact = Insight::new(InsightCode::TopCategory)
            .subject(top.key.clone())
            .amount(top.total);
        if let Some(pct) = share(top.total, spending) {
            fact = fact.pct(pct);
        }
        out.push(fact);
    }
    let grown = report
        .top
        .iter()
        .filter_map(|row| row.delta_pct.map(|delta| (row, delta)))
        .filter(|(_, delta)| *delta >= whole(NOTABLE_GROWTH_PCT))
        .max_by(|left, right| left.1.cmp(&right.1));
    if let Some((row, delta)) = grown {
        out.push(
            Insight::new(InsightCode::CategoryGrew)
                .subject(row.key.clone())
                .pct(delta),
        );
    }
    out
}

/// Net worth from its first point to its last.
pub fn wealth_change(first: Amount, last: Amount) -> Insight {
    let change = last - first;
    let pct = share(change, first);
    let flat = pct.map_or(change.is_zero(), |pct| pct.abs() < whole(FLAT_PCT));
    if flat {
        return Insight::new(InsightCode::WealthFlat).amount(last);
    }
    let code = if change.is_negative() {
        InsightCode::WealthDown
    } else {
        InsightCode::WealthUp
    };
    let fact = Insight::new(code).amount(change.abs());
    match pct {
        Some(pct) => fact.pct(pct.abs()),
        None => fact,
    }
}

pub(crate) fn networth_insights(report: &NetworthReport) -> Vec<Insight> {
    let (Some(first), Some(last)) = (report.points.first(), report.points.last()) else {
        return Vec::new();
    };
    let mut out = vec![wealth_change(first.net, last.net)];
    let owed = last.liabilities + last.payables;
    if owed.is_positive() {
        out.push(Insight::new(InsightCode::Owes).amount(owed));
    }
    if report.meta.real_terms && report.meta.inflation_rate.is_none() {
        out.push(Insight::new(InsightCode::NoInflationRate));
    }
    out
}

pub(crate) fn stores_insights(report: &StoresReport) -> Vec<Insight> {
    report
        .rows
        .iter()
        .filter(|row| row.change.is_positive())
        .max_by(|left, right| left.change.cmp(&right.change))
        .map(|row| {
            Insight::new(InsightCode::StoreGrewMost)
                .subject(row.name.clone())
                .amount(row.change)
        })
        .into_iter()
        .collect()
}

pub(crate) fn pnl_insights(report: &PnlReport) -> Vec<Insight> {
    let mut out = Vec::new();
    let realized = report.total_realized;
    if realized.is_positive() {
        out.push(Insight::new(InsightCode::Profit).amount(realized));
    } else if realized.is_negative() {
        out.push(Insight::new(InsightCode::Loss).amount(-realized));
    }
    if report.total_fees.is_positive() {
        out.push(Insight::new(InsightCode::FeesPaid).amount(report.total_fees));
    }
    out
}

pub(crate) fn portfolio_insights(report: &PortfolioReport) -> Vec<Insight> {
    let Some(last) = report.points.last() else {
        return Vec::new();
    };
    last.shares
        .iter()
        .zip(&report.assets)
        .filter(|(share, _)| share.is_positive())
        .max_by(|left, right| left.0.cmp(right.0))
        .map(|(share, asset)| {
            Insight::new(InsightCode::TopAsset)
                .subject(asset.clone())
                .pct(*share)
        })
        .into_iter()
        .collect()
}

pub(crate) fn inflation_insights(report: &InflationReport) -> Vec<Insight> {
    if report.meta.inflation_rate.is_none() {
        return vec![Insight::new(InsightCode::NoInflationRate)];
    }
    match (report.real_change_pct, report.nominal_change_pct) {
        (Some(real), Some(nominal)) => {
            vec![
                Insight::new(InsightCode::RealChange)
                    .pct(real)
                    .amount(nominal),
            ]
        }
        _ => Vec::new(),
    }
}

pub(crate) fn loss_insights(report: &LossReport) -> Vec<Insight> {
    let total = report.losses.total.total;
    if !total.is_positive() {
        return Vec::new();
    }
    let mut fact = Insight::new(InsightCode::ConversionLosses).amount(total);
    if let Some(pct) = share(report.losses.total.fee, total) {
        fact = fact.pct(pct);
    }
    vec![fact]
}

// -- overview ------------------------------------------------------------------

/// «Income and spending» card: the month against the one before.
#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct OverviewFlow {
    pub income: Amount,
    pub spent: Amount,
    pub net: Amount,
    pub previous_spent: Amount,
    /// The biggest expense of the month, a category key.
    pub top_category: Option<String>,
    pub has_data: bool,
    pub insights: Vec<Insight>,
}

/// «How much I have» card: net worth now and over the month.
#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct OverviewWealth {
    pub net: Amount,
    pub change: Amount,
    pub has_data: bool,
    pub insights: Vec<Insight>,
}

/// «Investments» card: since the start of the year — a month is too short.
#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct OverviewInvestments {
    pub realized: Amount,
    pub fees: Amount,
    pub losses: Amount,
    pub has_data: bool,
    pub insights: Vec<Insight>,
}

/// «Year» card: what the year kept so far.
#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct OverviewYear {
    pub saved: Amount,
    pub saved_pct: Amount,
    pub best_month: Option<String>,
    pub worst_month: Option<String>,
    pub has_data: bool,
    pub insights: Vec<Insight>,
}

/// The analytics start screen: one card per question.
#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ReportOverview {
    pub currency: String,
    /// The month the cards are about, `YYYY-MM`.
    pub month: String,
    pub previous_month: String,
    pub year: i32,
    pub flow: OverviewFlow,
    pub wealth: OverviewWealth,
    pub investments: OverviewInvestments,
    pub savings: OverviewYear,
}

fn month_before(month: NaiveDate) -> NaiveDate {
    first_of_month(month - Duration::days(1))
}

/// The overview for the filter's scope and currency. Its month is
/// `filter.to` (this month by default), compared with the one before.
pub fn report_overview(
    books: &Books,
    filter: &ReportFilter,
    now: i64,
    zone: &str,
) -> Result<ReportOverview> {
    let anchor = report_ctx(books, filter, now, zone)?;
    let month = *anchor
        .months
        .last()
        .unwrap_or(&first_of_month(anchor.today));
    let previous = month_before(month);
    let key = calendar::month_key;
    let span = |from: NaiveDate, to: NaiveDate| ReportFilter {
        from: Some(key(from)),
        to: Some(key(to)),
        ..filter.clone()
    };

    let pair = report_ctx(books, &span(previous, month), now, zone)?;
    let cash = cashflow_report(books, &pair);
    let (before, current) = (&cash.rows[0], &cash.rows[1]);
    let single = report_ctx(books, &span(month, month), now, zone)?;
    let categories = categories_report(books, &single);
    let spent = current.fixed + current.daily;
    let previous_spent = before.fixed + before.daily;
    let mut flow_insights: Vec<Insight> =
        balance_insight(current.income, spent).into_iter().collect();
    if let Some(pct) = share(spent - previous_spent, previous_spent)
        && !pct.is_zero()
    {
        let code = if pct.is_negative() {
            InsightCode::SpentDown
        } else {
            InsightCode::SpentUp
        };
        flow_insights.push(Insight::new(code).pct(pct.abs()));
    }
    flow_insights.extend(
        categories_insights(&categories)
            .into_iter()
            .filter(|fact| fact.code == InsightCode::TopCategory),
    );
    // Creep is measured over the whole budget in its currency.
    let whole_budget = filter.scope.store_ids.is_empty()
        && filter
            .currency
            .as_ref()
            .is_none_or(|code| code.eq_ignore_ascii_case(&budget_currency(books)));
    if whole_budget {
        flow_insights.extend(lifestyle_creep(books, now, zone));
    }
    let flow = OverviewFlow {
        income: current.income,
        spent,
        net: current.net,
        previous_spent,
        top_category: categories.top.first().map(|row| row.key.clone()),
        has_data: !(current.income.is_zero()
            && spent.is_zero()
            && before.income.is_zero()
            && previous_spent.is_zero()),
        insights: flow_insights,
    };

    let worth = networth_report(books, &pair);
    let wealth = match (worth.points.first(), worth.points.last()) {
        (Some(first), Some(last)) => OverviewWealth {
            net: last.net,
            change: last.net - first.net,
            has_data: !(last.financial.is_zero()
                && last.non_financial.is_zero()
                && last.receivables.is_zero()
                && last.liabilities.is_zero()
                && last.payables.is_zero()),
            insights: networth_insights(&worth),
        },
        _ => OverviewWealth::default(),
    };

    let year = month.year();
    let january = NaiveDate::from_ymd_opt(year, 1, 1).unwrap_or(month);
    let since_january = report_ctx(books, &span(january, month), now, zone)?;
    let pnl = pnl_report(books, &since_january);
    let losses = loss_report(books, &since_january);
    // Spending a currency also closes «positions»; only a result counts.
    let traded = realized_pnl_filtered(books, since_january.scope.as_ref())
        .legs
        .iter()
        .any(|leg| !leg.pnl_base.is_zero());
    let mut invest_insights = pnl_insights(&pnl);
    invest_insights.extend(loss_insights(&losses));
    let investments = OverviewInvestments {
        realized: pnl.total_realized,
        fees: pnl.total_fees,
        losses: losses.losses.total.total,
        has_data: traded || !pnl.total_fees.is_zero() || !losses.losses.total.total.is_zero(),
        insights: invest_insights,
    };

    let whole_year = report_ctx(
        books,
        &span(
            january,
            NaiveDate::from_ymd_opt(year, 12, 1).unwrap_or(month),
        ),
        now,
        zone,
    )?;
    let year_report = year_report(books, &whole_year);
    let savings = OverviewYear {
        saved: year_report.total.savings_fact,
        saved_pct: year_report.total.saved_pct,
        best_month: year_report.best_month.clone(),
        worst_month: year_report.worst_month.clone(),
        has_data: year_report.rows.iter().any(|row| !row.empty),
        insights: year_insights(&year_report),
    };

    Ok(ReportOverview {
        currency: anchor.currency.clone(),
        month: key(month),
        previous_month: key(previous),
        year,
        flow,
        wealth,
        investments,
        savings,
    })
}
