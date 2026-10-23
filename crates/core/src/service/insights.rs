//! What a report says in words, and the analytics overview.
//!
//! A report gets a few [`Insight`]s: a code and the numbers behind it. The
//! sentence itself is built by the UI from its locale, so one set of facts
//! reads in every language (and can later be handed to a model as is).

use std::collections::BTreeMap;

use chrono::{Datelike, Duration, NaiveDate};

use crate::Amount;
use crate::calendar::{self, first_of_month};
use crate::error::Result;
use crate::model::category::CategoryGroup;

use super::UnlockedVault;
use super::losses::LossReport;
use super::reports::{
    CashflowReport, CategoriesReport, InflationReport, NetworthReport, PnlReport, PortfolioReport,
    Report, ReportFilter, StoresReport, YearReport,
};

/// A category must change at least this much (percent) to be named as grown.
const NOTABLE_GROWTH_PCT: i64 = 10;
/// Net worth moving less than this (percent) reads as «about the same».
const FLAT_PCT: i64 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
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
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
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
fn balance_insight(income: Amount, spent: Amount) -> Option<Insight> {
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
fn wealth_change(first: Amount, last: Amount) -> Insight {
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
    let total = report.total.total;
    if !total.is_positive() {
        return Vec::new();
    }
    let mut fact = Insight::new(InsightCode::ConversionLosses).amount(total);
    if let Some(pct) = share(report.total.fee, total) {
        fact = fact.pct(pct);
    }
    vec![fact]
}

// -- overview ------------------------------------------------------------------

/// «Income and spending» card: the month against the one before.
#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
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
#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct OverviewWealth {
    pub net: Amount,
    pub change: Amount,
    pub has_data: bool,
    pub insights: Vec<Insight>,
}

/// «Investments» card: since the start of the year — a month is too short.
#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct OverviewInvestments {
    pub realized: Amount,
    pub fees: Amount,
    pub losses: Amount,
    pub has_data: bool,
    pub insights: Vec<Insight>,
}

/// «Year» card: what the year kept so far.
#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct OverviewYear {
    pub saved: Amount,
    pub saved_pct: Amount,
    pub best_month: Option<String>,
    pub worst_month: Option<String>,
    pub has_data: bool,
    pub insights: Vec<Insight>,
}

/// The analytics start screen: one card per question.
#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
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

impl UnlockedVault {
    /// The overview for the filter's scope and currency. Its month is
    /// `filter.to` (this month by default), compared with the one before.
    pub fn report_overview(
        &self,
        filter: &ReportFilter,
        now: i64,
        zone: &str,
        inflation: &BTreeMap<String, Amount>,
    ) -> Result<ReportOverview> {
        let anchor = self.report_ctx(filter, now, zone, inflation)?;
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

        let pair = self.report_ctx(&span(previous, month), now, zone, inflation)?;
        let cash = self.cashflow_report(&pair);
        let (before, current) = (&cash.rows[0], &cash.rows[1]);
        let single = self.report_ctx(&span(month, month), now, zone, inflation)?;
        let categories = self.categories_report(&single);
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
                .is_none_or(|code| code.eq_ignore_ascii_case(&self.budget_currency()));
        if whole_budget {
            flow_insights.extend(self.lifestyle_creep(now, zone));
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

        let worth = self.networth_report(&pair);
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
        let since_january = self.report_ctx(&span(january, month), now, zone, inflation)?;
        let pnl = self.pnl_report(&since_january);
        let losses = self.loss_report(&since_january);
        // Spending a currency also closes «positions»; only a result counts.
        let traded = self
            .realized_pnl_filtered(since_january.scope.as_ref())
            .legs
            .iter()
            .any(|leg| !leg.pnl_base.is_zero());
        let mut invest_insights = pnl_insights(&pnl);
        invest_insights.extend(loss_insights(&losses));
        let investments = OverviewInvestments {
            realized: pnl.total_realized,
            fees: pnl.total_fees,
            losses: losses.total.total,
            has_data: traded || !pnl.total_fees.is_zero() || !losses.total.total.is_zero(),
            insights: invest_insights,
        };

        let whole_year = self.report_ctx(
            &span(
                january,
                NaiveDate::from_ymd_opt(year, 12, 1).unwrap_or(month),
            ),
            now,
            zone,
            inflation,
        )?;
        let year_report = self.year_report(&whole_year);
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::store::StoreKind;
    use crate::model::transaction::Transaction;
    use crate::pricing::history::unix_secs;
    use crate::service::ReportKind;
    use crate::service::test_support::*;

    const NOW: &str = "2026-06-15T12:00:00Z";

    fn dated(mut tx: Transaction, ts: &str) -> Transaction {
        at(&mut tx, ts);
        tx
    }

    fn spend(store: &str, sum: &str, category: &str, ts: &str) -> Transaction {
        let mut tx = payment(store, sum, "spend");
        tx.meta_mut().categories = vec![category.into()];
        dated(tx, ts)
    }

    fn earn(store: &str, sum: &str, ts: &str) -> Transaction {
        dated(income(store, sum, "salary"), ts)
    }

    fn vault_with_card() -> (tempfile::TempDir, UnlockedVault, String) {
        let (dir, mut vault) = empty_vault();
        vault.set_base_currency("usd");
        quote_usd(&mut vault);
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        (dir, vault, card)
    }

    fn overview(vault: &UnlockedVault) -> ReportOverview {
        vault
            .report_overview(
                &ReportFilter::default(),
                unix_secs(NOW).unwrap(),
                "UTC",
                &BTreeMap::new(),
            )
            .unwrap()
    }

    fn codes(facts: &[Insight]) -> Vec<InsightCode> {
        facts.iter().map(|fact| fact.code).collect()
    }

    #[test]
    fn overspending_names_the_gap_and_the_share_of_income() {
        let fact = balance_insight(amount("1000"), amount("1120")).unwrap();
        assert_eq!(fact.code, InsightCode::Overspent);
        assert_eq!(fact.amount, Some(amount("120")));
        assert_eq!(fact.pct, Some(amount("12")));
        let kept = balance_insight(amount("1000"), amount("750")).unwrap();
        assert_eq!(kept.code, InsightCode::Saved);
        assert_eq!(kept.pct, Some(amount("25")));
        assert_eq!(
            balance_insight(Amount::ZERO, amount("10")).unwrap().code,
            InsightCode::NoIncome
        );
        assert!(balance_insight(Amount::ZERO, Amount::ZERO).is_none());
    }

    #[test]
    fn wealth_within_a_percent_reads_as_flat() {
        assert_eq!(
            wealth_change(amount("1000"), amount("1005")).code,
            InsightCode::WealthFlat
        );
        let down = wealth_change(amount("1000"), amount("800"));
        assert_eq!(down.code, InsightCode::WealthDown);
        assert_eq!(down.amount, Some(amount("200")));
        assert_eq!(down.pct, Some(amount("20")));
        assert_eq!(
            wealth_change(Amount::ZERO, amount("50")).code,
            InsightCode::WealthUp
        );
    }

    #[test]
    fn every_report_carries_its_insights_in_the_meta() {
        let (_dir, mut vault, card) = vault_with_card();
        vault
            .add_transaction(earn(&card, "1000", "2026-05-05T10:00:00Z"))
            .unwrap();
        vault
            .add_transaction(spend(&card, "1200", "groceries", "2026-05-06T10:00:00Z"))
            .unwrap();
        let report = vault
            .report(
                ReportKind::Cashflow,
                &ReportFilter::default(),
                unix_secs(NOW).unwrap(),
                "UTC",
                &BTreeMap::new(),
            )
            .unwrap();
        let Report::Cashflow(cash) = report else {
            panic!()
        };
        assert_eq!(
            codes(&cash.meta.insights),
            vec![InsightCode::Overspent, InsightCode::CostliestMonth]
        );
        assert_eq!(cash.meta.insights[1].subject.as_deref(), Some("2026-05"));
    }

    #[test]
    fn the_overview_compares_the_month_with_the_one_before() {
        let (_dir, mut vault, card) = vault_with_card();
        vault
            .add_transaction(earn(&card, "1000", "2026-05-05T10:00:00Z"))
            .unwrap();
        vault
            .add_transaction(spend(&card, "400", "groceries", "2026-05-06T10:00:00Z"))
            .unwrap();
        vault
            .add_transaction(earn(&card, "1000", "2026-06-05T10:00:00Z"))
            .unwrap();
        vault
            .add_transaction(spend(&card, "500", "groceries", "2026-06-06T10:00:00Z"))
            .unwrap();
        vault
            .add_transaction(spend(&card, "100", "taxi", "2026-06-07T10:00:00Z"))
            .unwrap();
        let view = overview(&vault);
        assert_eq!(view.month, "2026-06");
        assert_eq!(view.previous_month, "2026-05");
        assert_eq!(view.flow.spent, amount("600"));
        assert_eq!(view.flow.previous_spent, amount("400"));
        assert_eq!(view.flow.top_category.as_deref(), Some("groceries"));
        assert_eq!(
            codes(&view.flow.insights),
            vec![
                InsightCode::Saved,
                InsightCode::SpentUp,
                InsightCode::TopCategory
            ]
        );
        assert_eq!(view.flow.insights[1].pct, Some(amount("50")));
        assert!(view.flow.has_data);
        assert!(view.wealth.has_data);
        assert_eq!(view.wealth.net, amount("1000"));
        assert!(view.savings.has_data);
        assert_eq!(view.savings.saved, amount("1000"));
        assert!(!view.investments.has_data);
    }

    #[test]
    fn an_empty_vault_has_no_data_on_any_card() {
        let (_dir, vault, _card) = vault_with_card();
        let view = overview(&vault);
        assert!(!view.flow.has_data);
        assert!(!view.wealth.has_data);
        assert!(!view.investments.has_data);
        assert!(!view.savings.has_data);
        assert!(view.flow.insights.is_empty());
    }
}
