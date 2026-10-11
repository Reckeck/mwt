//! Analytics (plan Ф6): reports over months, the spending calendar and CSV.
//!
//! Every report reads the same classified flows as the budget
//! ([`UnlockedVault::classify_flow`]) or replays legs to value what was held
//! at a past date. Amounts are in the report currency (the budget currency
//! unless the filter names one) at the rate of the day they happened; with
//! `real_terms` each month is restated in today's money by the user's
//! reference inflation (plan §6.5).

use std::collections::{BTreeMap, HashMap, HashSet};

use chrono::{Datelike, Duration, NaiveDate};

use crate::Amount;
use crate::calendar::{self, Calendar, first_of_month, next_month};
use crate::error::{Error, Result};
use crate::model::category::CategoryGroup;
use crate::model::store::{AssetRef, StoreKind, StoreScope};
use crate::model::transaction::{Leg, LegRole};
use crate::pricing::history::secs_or_now;

use super::budget::Flow;
use super::rates::RateBook;
use super::{FlowClass, UnlockedVault, UpcomingItem};

/// Longest range a report covers.
pub const MAX_REPORT_MONTHS: usize = 120;
/// Assets shown by name in the portfolio report; the rest is «other».
pub const PORTFOLIO_TOP: usize = 8;
/// Above this many days the portfolio report samples monthly, else weekly.
pub const PORTFOLIO_MONTHLY_AFTER_DAYS: i64 = 180;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum ReportKind {
    Cashflow,
    Year,
    Categories,
    Networth,
    Stores,
    Pnl,
    Portfolio,
    Inflation,
    Losses,
}

/// What every report is asked for.
#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct ReportFilter {
    /// Wallets to cover; empty = everything.
    #[serde(default)]
    pub scope: StoreScope,
    /// First month, `YYYY-MM`; default: eleven months before `to`.
    #[serde(default)]
    pub from: Option<String>,
    /// Last month, `YYYY-MM`; default: this month.
    #[serde(default)]
    pub to: Option<String>,
    /// The year of the year report; default: this year.
    #[serde(default)]
    pub year: Option<i32>,
    /// Report currency; default: the budget currency.
    #[serde(default)]
    pub currency: Option<String>,
    /// Restate each month in today's money by the reference inflation.
    #[serde(default)]
    pub real_terms: bool,
}

/// What a report covers, for its header.
#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct ReportMeta {
    pub currency: String,
    /// `YYYY-MM`, oldest first.
    pub months: Vec<String>,
    pub real_terms: bool,
    /// Annual inflation used for `real_terms`, percent.
    pub inflation_rate: Option<Amount>,
}

#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct CashflowRow {
    pub month: String,
    pub income: Amount,
    pub fixed: Amount,
    pub daily: Amount,
    pub savings_moved: Amount,
    pub budget_in: Amount,
    pub budget_out: Amount,
    /// `income − fixed − daily`.
    pub net: Amount,
}

#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct CashflowReport {
    pub meta: ReportMeta,
    pub rows: Vec<CashflowRow>,
    pub total: CashflowRow,
}

#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct YearRow {
    pub month: String,
    pub income: Amount,
    pub fixed: Amount,
    pub daily: Amount,
    pub savings_plan: Amount,
    pub savings_fact: Amount,
    pub deviation: Amount,
    pub saved_pct: Amount,
    /// No income and no spending: left out of the average.
    pub empty: bool,
}

#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct YearReport {
    pub meta: ReportMeta,
    pub year: i32,
    pub rows: Vec<YearRow>,
    pub total: YearRow,
    /// Average of the non-empty months.
    pub average: YearRow,
    /// Month with the most and the least left over.
    pub best_month: Option<String>,
    pub worst_month: Option<String>,
}

#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct CategoryRow {
    /// Category key; empty = without a category.
    pub key: String,
    pub group: Option<CategoryGroup>,
    /// One per report month.
    pub cells: Vec<Amount>,
    pub total: Amount,
    pub average: Amount,
    /// Share of its group's total, percent.
    pub share: Amount,
    /// Change against the previous period of the same length, percent;
    /// `None` when that period had nothing.
    pub delta_pct: Option<Amount>,
}

#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct CategoryGroupBlock {
    pub group: Option<CategoryGroup>,
    pub rows: Vec<CategoryRow>,
    /// Group total per month.
    pub totals: Vec<Amount>,
    pub total: Amount,
}

#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct CategoriesReport {
    pub meta: ReportMeta,
    pub groups: Vec<CategoryGroupBlock>,
    /// The five biggest spending categories of the period.
    pub top: Vec<CategoryRow>,
}

#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct NetworthPoint {
    pub month: String,
    pub financial: Amount,
    pub non_financial: Amount,
    pub receivables: Amount,
    pub liabilities: Amount,
    pub payables: Amount,
    pub net: Amount,
}

#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct NetworthReport {
    pub meta: ReportMeta,
    /// Value at the end of each month (today for this month).
    pub points: Vec<NetworthPoint>,
}

#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct StoreReportRow {
    pub store_id: String,
    pub name: String,
    /// 0 for a top-level wallet.
    pub depth: u32,
    pub kind: Option<StoreKind>,
    /// Value just before the first month and at the end of the last.
    pub value_start: Amount,
    pub value_end: Amount,
    pub change: Amount,
    /// Money in and out over the period, on the day it moved.
    pub inflow: Amount,
    pub outflow: Amount,
    pub tx_count: u32,
}

#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct StoresReport {
    pub meta: ReportMeta,
    pub rows: Vec<StoreReportRow>,
}

#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct PnlRow {
    pub month: String,
    pub realized: Amount,
    pub fees: Amount,
}

#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct PnlReport {
    pub meta: ReportMeta,
    pub rows: Vec<PnlRow>,
    pub total_realized: Amount,
    pub total_fees: Amount,
}

#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct PortfolioPoint {
    /// `YYYY-MM-DD`.
    pub date: String,
    pub total: Amount,
    /// In the order of [`PortfolioReport::assets`].
    pub values: Vec<Amount>,
    /// Percent; sums to 100 when `total` is positive.
    pub shares: Vec<Amount>,
}

#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct PortfolioReport {
    pub meta: ReportMeta,
    /// Asset symbols, the biggest first; the last may be `""` for «other».
    pub assets: Vec<String>,
    pub points: Vec<PortfolioPoint>,
}

#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct InflationPoint {
    pub month: String,
    /// What one unit of that month is worth in today's money.
    pub coefficient: Amount,
    pub nominal_net: Amount,
    pub real_net: Amount,
}

#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct InflationReport {
    pub meta: ReportMeta,
    pub points: Vec<InflationPoint>,
    /// Change of net worth over the period, nominal and real, percent.
    pub nominal_change_pct: Option<Amount>,
    pub real_change_pct: Option<Amount>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
// `Report` is a DOM type in TypeScript; a different name keeps them apart.
#[ts(rename = "AnalyticsReport")]
pub enum Report {
    Cashflow(CashflowReport),
    Year(Box<YearReport>),
    Categories(CategoriesReport),
    Networth(NetworthReport),
    Stores(StoresReport),
    Pnl(PnlReport),
    Portfolio(PortfolioReport),
    Inflation(InflationReport),
    Losses(super::losses::LossReport),
}

/// How one day of the calendar went against its allowance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum DayStatus {
    /// No allowance to compare with.
    None,
    Under,
    /// Above 80 % of the allowance.
    Near,
    Over,
    Future,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct CalendarDay {
    /// `YYYY-MM-DD`.
    pub date: String,
    pub income: Amount,
    /// Mandatory and everyday spending.
    pub spent: Amount,
    /// Everyday spending — what the allowance is about.
    pub daily: Amount,
    /// Every transaction of the day, budget or not.
    pub tx_count: u32,
    pub status: DayStatus,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct CalendarMonth {
    /// `YYYY-MM`.
    pub month: String,
    pub currency: String,
    /// The day's allowance: the month mode's, or the hand-set day limit when smaller.
    pub allowance: Option<Amount>,
    pub days: Vec<CalendarDay>,
    /// What is still to come this month.
    pub upcoming: Vec<UpcomingItem>,
}

/// Everything a report needs to know about its range.
pub(crate) struct Ctx {
    pub(crate) cal: Calendar,
    pub(crate) currency: String,
    /// First days of the report months.
    pub(crate) months: Vec<NaiveDate>,
    pub(crate) today: NaiveDate,
    pub(crate) now: i64,
    pub(crate) rate: Option<Amount>,
    pub(crate) real: bool,
    pub(crate) scope: Option<HashSet<String>>,
}

impl Ctx {
    pub(crate) fn meta(&self) -> ReportMeta {
        ReportMeta {
            currency: self.currency.clone(),
            months: self
                .months
                .iter()
                .map(|month| calendar::month_key(*month))
                .collect(),
            real_terms: self.real,
            inflation_rate: self.rate,
        }
    }

    pub(crate) fn start(&self) -> i64 {
        self.cal.start_of(self.months[0])
    }

    pub(crate) fn end(&self) -> i64 {
        self.cal
            .start_of(next_month(*self.months.last().unwrap_or(&self.today)))
    }

    /// End of a month as a valuation moment: its last second, or now.
    fn month_end(&self, month: NaiveDate) -> i64 {
        (self.cal.start_of(next_month(month)) - 1).min(self.now)
    }

    /// What one unit of `month` is worth in today's money.
    fn coefficient(&self, month: NaiveDate) -> Amount {
        let Some(rate) = self.rate else {
            return one();
        };
        let elapsed = months_between(month, first_of_month(self.today)).saturating_sub(1);
        let years = f64::from(elapsed) / 12.0;
        Amount::from_f64((1.0 + rate.to_f64() / 100.0).powf(years))
    }

    /// `amount` of `month` as the report shows it.
    pub(crate) fn shown(&self, amount: Amount, month: NaiveDate) -> Amount {
        if self.real {
            (amount * self.coefficient(month)).round(2)
        } else {
            amount
        }
    }

    pub(crate) fn in_scope(&self, store_id: &str) -> bool {
        self.scope.as_ref().is_none_or(|ids| ids.contains(store_id))
    }

    pub(crate) fn index_of(&self, date: NaiveDate) -> Option<usize> {
        let first = first_of_month(date);
        self.months.iter().position(|month| *month == first)
    }
}

/// Months from `from` to `to`, both included; zero when `to` is earlier.
fn months_between(from: NaiveDate, to: NaiveDate) -> u32 {
    let span = (to.year() - from.year()) * 12 + to.month() as i32 - from.month() as i32 + 1;
    span.max(0) as u32
}

fn one() -> Amount {
    Amount::new(rust_decimal::Decimal::ONE)
}

fn hundred() -> Amount {
    Amount::new(rust_decimal::Decimal::from(100))
}

fn count(value: usize) -> Amount {
    Amount::new(rust_decimal::Decimal::from(value as u64))
}

fn pct(part: Amount, whole: Amount) -> Amount {
    if whole.is_zero() {
        Amount::ZERO
    } else {
        (part / whole * hundred()).round(1)
    }
}

/// A category's cells per report month, its previous-period total and group.
type CategoryCells = (Vec<Amount>, Amount, Option<CategoryGroup>);

/// Positions per (store, asset key) at each cutoff, cutoffs ascending.
type Positions = HashMap<(String, String), (AssetRef, Amount)>;

impl UnlockedVault {
    pub(crate) fn report_ctx(
        &self,
        filter: &ReportFilter,
        now: i64,
        zone: &str,
        inflation: &BTreeMap<String, Amount>,
    ) -> Result<Ctx> {
        let cal = Calendar::new(zone);
        let today = cal.date_of(now);
        let parse = |raw: &Option<String>| -> Result<Option<NaiveDate>> {
            match raw
                .as_deref()
                .map(str::trim)
                .filter(|text| !text.is_empty())
            {
                None => Ok(None),
                Some(text) => calendar::parse_month(text)
                    .map(Some)
                    .ok_or_else(|| Error::InvalidInput(format!("month {text}: expected YYYY-MM"))),
            }
        };
        let to = parse(&filter.to)?.unwrap_or_else(|| first_of_month(today));
        let mut from = match parse(&filter.from)? {
            Some(month) => month,
            None => {
                let mut month = to;
                for _ in 0..11 {
                    month = first_of_month(month - Duration::days(1));
                }
                month
            }
        };
        if from > to {
            return Err(Error::InvalidInput(
                "the period starts after it ends".into(),
            ));
        }
        let mut months = Vec::new();
        while from <= to && months.len() < MAX_REPORT_MONTHS {
            months.push(from);
            from = next_month(from);
        }
        let currency = filter
            .currency
            .as_deref()
            .map(|code| code.trim().to_lowercase())
            .filter(|code| !code.is_empty())
            .unwrap_or_else(|| self.budget_currency());
        let scope = (!filter.scope.store_ids.is_empty()).then(|| self.resolve_scope(&filter.scope));
        Ok(Ctx {
            cal,
            rate: inflation.get(&currency).copied(),
            currency,
            months,
            today,
            now,
            real: filter.real_terms,
            scope,
        })
    }

    /// One report of a kind for a filter. `inflation` is the user's
    /// reference table (`AppSettings.inflation`).
    pub fn report(
        &self,
        kind: ReportKind,
        filter: &ReportFilter,
        now: i64,
        zone: &str,
        inflation: &BTreeMap<String, Amount>,
    ) -> Result<Report> {
        let mut filter = filter.clone();
        if kind == ReportKind::Year {
            let cal = Calendar::new(zone);
            let year = filter.year.unwrap_or_else(|| cal.date_of(now).year());
            filter.from = Some(format!("{year:04}-01"));
            filter.to = Some(format!("{year:04}-12"));
        }
        let ctx = self.report_ctx(&filter, now, zone, inflation)?;
        Ok(match kind {
            ReportKind::Cashflow => Report::Cashflow(self.cashflow_report(&ctx)),
            ReportKind::Year => Report::Year(Box::new(self.year_report(&ctx))),
            ReportKind::Categories => Report::Categories(self.categories_report(&ctx)),
            ReportKind::Networth => Report::Networth(self.networth_report(&ctx)),
            ReportKind::Stores => Report::Stores(self.stores_report(&ctx)),
            ReportKind::Pnl => Report::Pnl(self.pnl_report(&ctx)),
            ReportKind::Portfolio => Report::Portfolio(self.portfolio_report(&ctx)),
            ReportKind::Inflation => Report::Inflation(self.inflation_report(&ctx)),
            ReportKind::Losses => Report::Losses(self.loss_report(&ctx)),
        })
    }

    /// Classified flows of the report range (or `[from, to)`), in scope.
    fn report_flows(&self, ctx: &Ctx, from: i64, to: i64) -> Vec<Flow> {
        self.flows_between(&ctx.cal, &ctx.currency, from, to)
            .into_iter()
            .filter(|flow| {
                ctx.scope
                    .as_ref()
                    .is_none_or(|ids| flow.stores.iter().any(|id| ids.contains(id)))
            })
            .collect()
    }

    // -- cash flow --------------------------------------------------------------

    fn cashflow_report(&self, ctx: &Ctx) -> CashflowReport {
        let mut rows: Vec<CashflowRow> = ctx
            .months
            .iter()
            .map(|month| CashflowRow {
                month: calendar::month_key(*month),
                ..CashflowRow::default()
            })
            .collect();
        for flow in self.report_flows(ctx, ctx.start(), ctx.end()) {
            let Some(index) = ctx.index_of(flow.date) else {
                continue;
            };
            let value = ctx.shown(flow.value, ctx.months[index]);
            let row = &mut rows[index];
            match flow.class {
                FlowClass::Income => row.income += value,
                FlowClass::Fixed => row.fixed += value,
                FlowClass::Daily => row.daily += value,
                FlowClass::Savings => row.savings_moved += value,
                FlowClass::BudgetIn => row.budget_in += value,
                FlowClass::BudgetOut => row.budget_out += value,
                FlowClass::Ignored => {}
            }
        }
        let mut total = CashflowRow {
            month: String::new(),
            ..CashflowRow::default()
        };
        for row in &mut rows {
            row.net = row.income - row.fixed - row.daily;
            total.income += row.income;
            total.fixed += row.fixed;
            total.daily += row.daily;
            total.savings_moved += row.savings_moved;
            total.budget_in += row.budget_in;
            total.budget_out += row.budget_out;
            total.net += row.net;
        }
        CashflowReport {
            meta: ctx.meta(),
            rows,
            total,
        }
    }

    // -- the year -------------------------------------------------------------------

    fn year_report(&self, ctx: &Ctx) -> YearReport {
        let rate = self.doc.settings.savings_rate;
        let cash = self.cashflow_report(ctx);
        let finish = |row: &mut YearRow| {
            row.savings_plan = (row.income * rate).round(2);
            row.savings_fact = row.income - row.fixed - row.daily;
            row.deviation = row.savings_fact - row.savings_plan;
            row.saved_pct = pct(row.savings_fact, row.income);
        };
        let rows: Vec<YearRow> = cash
            .rows
            .iter()
            .map(|cash_row| {
                let mut row = YearRow {
                    month: cash_row.month.clone(),
                    income: cash_row.income,
                    fixed: cash_row.fixed,
                    daily: cash_row.daily,
                    empty: cash_row.income.is_zero()
                        && cash_row.fixed.is_zero()
                        && cash_row.daily.is_zero(),
                    ..YearRow::default()
                };
                finish(&mut row);
                row
            })
            .collect();
        let mut total = YearRow::default();
        for row in &rows {
            total.income += row.income;
            total.fixed += row.fixed;
            total.daily += row.daily;
        }
        finish(&mut total);
        let filled: Vec<&YearRow> = rows.iter().filter(|row| !row.empty).collect();
        let mut average = YearRow::default();
        if !filled.is_empty() {
            let size = count(filled.len());
            average.income = (total.income / size).round(2);
            average.fixed = (total.fixed / size).round(2);
            average.daily = (total.daily / size).round(2);
            finish(&mut average);
        }
        // Ties go to the earlier month: `max_by` keeps the last of equals, so
        // walk the months in reverse.
        let best_month = filled
            .iter()
            .rev()
            .max_by(|left, right| left.savings_fact.cmp(&right.savings_fact))
            .map(|row| row.month.clone());
        let worst_month = filled
            .iter()
            .rev()
            .min_by(|left, right| left.savings_fact.cmp(&right.savings_fact))
            .map(|row| row.month.clone());
        YearReport {
            meta: ctx.meta(),
            year: ctx.months.first().map_or(ctx.today.year(), NaiveDate::year),
            rows,
            total,
            average,
            best_month,
            worst_month,
        }
    }

    // -- categories ---------------------------------------------------------------

    fn categories_report(&self, ctx: &Ctx) -> CategoriesReport {
        let span = ctx.months.len();
        let mut previous_start = ctx.months[0];
        for _ in 0..span {
            previous_start = first_of_month(previous_start - Duration::days(1));
        }
        let flows = self.report_flows(ctx, ctx.cal.start_of(previous_start), ctx.end());
        let group_of = |class: FlowClass| match class {
            FlowClass::Income => Some(CategoryGroup::Income),
            FlowClass::Fixed => Some(CategoryGroup::Fixed),
            FlowClass::Daily => Some(CategoryGroup::Daily),
            FlowClass::Savings => Some(CategoryGroup::Savings),
            _ => None,
        };
        // (group order, key) → (cells, previous total)
        let mut table: BTreeMap<(u8, String), CategoryCells> = BTreeMap::new();
        let order = |group: CategoryGroup| match group {
            CategoryGroup::Income => 0u8,
            CategoryGroup::Fixed => 1,
            CategoryGroup::Daily => 2,
            CategoryGroup::Savings => 3,
            CategoryGroup::Service => 4,
        };
        for flow in &flows {
            let Some(group) = group_of(flow.class) else {
                continue;
            };
            let key = flow.categories.first().cloned().unwrap_or_default();
            let entry = table
                .entry((order(group), key))
                .or_insert_with(|| (vec![Amount::ZERO; span], Amount::ZERO, Some(group)));
            match ctx.index_of(flow.date) {
                Some(index) => entry.0[index] += ctx.shown(flow.value, ctx.months[index]),
                None => entry.1 += flow.value,
            }
        }
        let mut blocks: Vec<CategoryGroupBlock> = Vec::new();
        for ((_, key), (cells, previous, group)) in table {
            let total = cells.iter().fold(Amount::ZERO, |sum, cell| sum + *cell);
            if total.is_zero() && previous.is_zero() {
                continue;
            }
            let row = CategoryRow {
                key,
                group,
                average: (total / count(span)).round(2),
                total,
                share: Amount::ZERO,
                delta_pct: (!previous.is_zero())
                    .then(|| ((total - previous) / previous * hundred()).round(1)),
                cells,
            };
            match blocks.iter_mut().find(|block| block.group == group) {
                Some(block) => block.rows.push(row),
                None => blocks.push(CategoryGroupBlock {
                    group,
                    rows: vec![row],
                    totals: vec![Amount::ZERO; span],
                    total: Amount::ZERO,
                }),
            }
        }
        for block in &mut blocks {
            for row in &block.rows {
                for (index, cell) in row.cells.iter().enumerate() {
                    block.totals[index] += *cell;
                }
                block.total += row.total;
            }
            let block_total = block.total;
            for row in &mut block.rows {
                row.share = pct(row.total, block_total);
            }
            block
                .rows
                .sort_by(|left, right| right.total.cmp(&left.total).then(left.key.cmp(&right.key)));
        }
        let mut top: Vec<CategoryRow> = blocks
            .iter()
            .filter(|block| {
                matches!(
                    block.group,
                    Some(CategoryGroup::Fixed | CategoryGroup::Daily)
                )
            })
            .flat_map(|block| block.rows.iter().cloned())
            .collect();
        top.sort_by(|left, right| right.total.cmp(&left.total).then(left.key.cmp(&right.key)));
        top.truncate(5);
        CategoriesReport {
            meta: ctx.meta(),
            groups: blocks,
            top,
        }
    }

    // -- valuing the past -----------------------------------------------------------

    /// What every store held at each cutoff (ascending), from the legs.
    fn positions_at(&self, cutoffs: &[i64]) -> Vec<Positions> {
        let mut legs: Vec<(i64, Leg)> = self
            .doc
            .transactions
            .iter()
            .flat_map(|tx| {
                let secs = secs_or_now(&tx.meta().timestamp);
                tx.legs().into_iter().map(move |leg| (secs, leg))
            })
            .filter(|(_, leg)| !leg.asset.is_derivative())
            .collect();
        legs.sort_by_key(|(secs, _)| *secs);
        let mut out = Vec::with_capacity(cutoffs.len());
        let mut current: Positions = HashMap::new();
        let mut next = 0;
        for cutoff in cutoffs {
            while next < legs.len() && legs[next].0 <= *cutoff {
                let (_, leg) = &legs[next];
                let entry = current
                    .entry((leg.store_id.clone(), leg.asset.key()))
                    .or_insert_with(|| (leg.asset.clone(), Amount::ZERO));
                entry.1 += leg.amount;
                next += 1;
            }
            out.push(current.clone());
        }
        out
    }

    /// Value of a quantity in `currency` at `secs`; zero without a rate.
    pub(crate) fn value_at(
        book: &RateBook,
        asset: &AssetRef,
        qty: Amount,
        currency: &str,
        secs: i64,
    ) -> Amount {
        if qty.is_zero() {
            return Amount::ZERO;
        }
        if asset.is_fiat() && asset.symbol.eq_ignore_ascii_case(currency) {
            return qty;
        }
        book.history_price(&asset.key(), secs)
            .and_then(|usd| book.convert(qty * usd, "usd", currency, secs))
            .unwrap_or(Amount::ZERO)
    }

    fn networth_points(&self, ctx: &Ctx) -> Vec<NetworthPoint> {
        let book = self.rate_book();
        let kinds: HashMap<&str, StoreKind> = self
            .doc
            .stores
            .iter()
            .map(|store| (store.id.as_str(), store.kind))
            .collect();
        let cutoffs: Vec<i64> = ctx
            .months
            .iter()
            .map(|month| ctx.month_end(*month))
            .collect();
        let snapshots = self.positions_at(&cutoffs);
        ctx.months
            .iter()
            .zip(cutoffs.iter().zip(snapshots))
            .map(|(month, (cutoff, positions))| {
                let mut point = NetworthPoint {
                    month: calendar::month_key(*month),
                    ..NetworthPoint::default()
                };
                let mut loans: HashMap<&str, Amount> = HashMap::new();
                for ((store_id, _), (asset, qty)) in &positions {
                    if !ctx.in_scope(store_id) {
                        continue;
                    }
                    let value = Self::value_at(&book, asset, *qty, &ctx.currency, *cutoff);
                    match kinds.get(store_id.as_str()) {
                        Some(StoreKind::Property | StoreKind::Loyalty) => {
                            point.non_financial += value
                        }
                        Some(StoreKind::Liability) => {
                            *loans.entry(store_id.as_str()).or_default() += value;
                        }
                        _ => point.financial += value,
                    }
                }
                for value in loans.into_values() {
                    if value.is_negative() {
                        point.liabilities += value.abs();
                    } else {
                        point.financial += value;
                    }
                }
                if ctx.scope.is_none() {
                    for balance in self.contact_balances_until(*cutoff) {
                        let unit =
                            Self::value_at(&book, &balance.asset, one(), &ctx.currency, *cutoff);
                        point.receivables += balance.receivable.max(Amount::ZERO) * unit;
                        point.payables += balance.payable.max(Amount::ZERO) * unit;
                    }
                }
                point.net = point.financial + point.non_financial + point.receivables
                    - point.liabilities
                    - point.payables;
                for value in [
                    &mut point.financial,
                    &mut point.non_financial,
                    &mut point.receivables,
                    &mut point.liabilities,
                    &mut point.payables,
                    &mut point.net,
                ] {
                    *value = ctx.shown(value.round(2), *month);
                }
                point
            })
            .collect()
    }

    fn networth_report(&self, ctx: &Ctx) -> NetworthReport {
        NetworthReport {
            meta: ctx.meta(),
            points: self.networth_points(ctx),
        }
    }

    // -- wallets ----------------------------------------------------------------------

    fn stores_report(&self, ctx: &Ctx) -> StoresReport {
        let book = self.rate_book();
        let (start, end) = (ctx.start(), ctx.end().min(ctx.now));
        let snapshots = self.positions_at(&[start - 1, end]);
        let value_of = |positions: &Positions, store_id: &str, secs: i64| {
            positions
                .iter()
                .filter(|((store, _), _)| store == store_id)
                .fold(Amount::ZERO, |sum, (_, (asset, qty))| {
                    sum + Self::value_at(&book, asset, *qty, &ctx.currency, secs)
                })
        };
        let mut rows: Vec<(String, StoreReportRow)> = self
            .doc
            .stores
            .iter()
            .filter(|store| ctx.in_scope(&store.id))
            .map(|store| {
                let path: Vec<String> = self
                    .ancestors_of(&store.id)
                    .iter()
                    .map(|parent| parent.name.clone())
                    .chain(std::iter::once(store.name.clone()))
                    .collect();
                let value_start = value_of(&snapshots[0], &store.id, start - 1).round(2);
                let value_end = value_of(&snapshots[1], &store.id, end).round(2);
                let row = StoreReportRow {
                    store_id: store.id.clone(),
                    name: store.name.clone(),
                    depth: path.len().saturating_sub(1) as u32,
                    kind: Some(store.kind),
                    value_start,
                    value_end,
                    change: value_end - value_start,
                    ..StoreReportRow::default()
                };
                (path.join("\u{0}").to_lowercase(), row)
            })
            .collect();
        for tx in &self.doc.transactions {
            let secs = secs_or_now(&tx.meta().timestamp);
            if secs < start || secs >= end {
                continue;
            }
            let mut touched: HashSet<String> = HashSet::new();
            for leg in tx.legs() {
                if leg.role != LegRole::Principal {
                    continue;
                }
                let Some((_, row)) = rows
                    .iter_mut()
                    .find(|(_, row)| row.store_id == leg.store_id)
                else {
                    continue;
                };
                let value = UnlockedVault::leg_value_in(&book, &leg, &ctx.currency, secs)
                    .unwrap_or(Amount::ZERO);
                if leg.amount.is_negative() {
                    row.outflow += value;
                } else {
                    row.inflow += value;
                }
                touched.insert(leg.store_id.clone());
            }
            for (_, row) in rows.iter_mut() {
                if touched.contains(&row.store_id) {
                    row.tx_count += 1;
                }
            }
        }
        rows.sort_by(|left, right| left.0.cmp(&right.0));
        StoresReport {
            meta: ctx.meta(),
            rows: rows
                .into_iter()
                .map(|(_, mut row)| {
                    row.inflow = row.inflow.round(2);
                    row.outflow = row.outflow.round(2);
                    row
                })
                .collect(),
        }
    }

    // -- PnL --------------------------------------------------------------------------

    fn pnl_report(&self, ctx: &Ctx) -> PnlReport {
        let book = self.rate_book();
        let base = self.base_and_fx().0;
        let mut rows: Vec<PnlRow> = ctx
            .months
            .iter()
            .map(|month| PnlRow {
                month: calendar::month_key(*month),
                ..PnlRow::default()
            })
            .collect();
        for leg in self.realized_pnl_filtered(ctx.scope.as_ref()).legs {
            let secs = secs_or_now(&leg.ts);
            let Some(index) = ctx.index_of(ctx.cal.date_of(secs)) else {
                continue;
            };
            let value = book
                .convert(leg.pnl_base, &base, &ctx.currency, secs)
                .unwrap_or(leg.pnl_base);
            rows[index].realized += ctx.shown(value, ctx.months[index]);
        }
        for tx in &self.doc.transactions {
            let secs = secs_or_now(&tx.meta().timestamp);
            let Some(index) = ctx.index_of(ctx.cal.date_of(secs)) else {
                continue;
            };
            for leg in tx.legs() {
                if leg.role != LegRole::Fee || !ctx.in_scope(&leg.store_id) {
                    continue;
                }
                let value = UnlockedVault::leg_value_in(&book, &leg, &ctx.currency, secs)
                    .unwrap_or(Amount::ZERO);
                rows[index].fees += ctx.shown(value, ctx.months[index]);
            }
        }
        let mut out = PnlReport {
            meta: ctx.meta(),
            ..PnlReport::default()
        };
        for row in &mut rows {
            row.realized = row.realized.round(2);
            row.fees = row.fees.round(2);
            out.total_realized += row.realized;
            out.total_fees += row.fees;
        }
        out.rows = rows;
        out
    }

    // -- portfolio ----------------------------------------------------------------------

    fn portfolio_report(&self, ctx: &Ctx) -> PortfolioReport {
        let book = self.rate_book();
        let first = ctx.months[0];
        let last = (ctx
            .cal
            .start_of(next_month(*ctx.months.last().unwrap_or(&first)))
            - 1)
        .min(ctx.now);
        let last_date = ctx.cal.date_of(last);
        let monthly = (last_date - first).num_days() > PORTFOLIO_MONTHLY_AFTER_DAYS;
        let mut dates: Vec<NaiveDate> = Vec::new();
        if monthly {
            dates.extend(
                ctx.months
                    .iter()
                    .map(|month| (next_month(*month) - Duration::days(1)).min(last_date)),
            );
        } else {
            let mut date = first + Duration::days(6);
            while date < last_date {
                dates.push(date);
                date += Duration::days(7);
            }
            dates.push(last_date);
        }
        dates.dedup();
        let cutoffs: Vec<i64> = dates
            .iter()
            .map(|date| (ctx.cal.start_of(*date + Duration::days(1)) - 1).min(ctx.now))
            .collect();
        let snapshots = self.positions_at(&cutoffs);
        // Value per symbol per point; liabilities (negative) are not holdings.
        let by_symbol: Vec<BTreeMap<String, Amount>> = snapshots
            .iter()
            .zip(&cutoffs)
            .map(|(positions, cutoff)| {
                let mut values: BTreeMap<String, Amount> = BTreeMap::new();
                for ((store_id, _), (asset, qty)) in positions {
                    if !ctx.in_scope(store_id) || !qty.is_positive() {
                        continue;
                    }
                    *values.entry(asset.symbol.clone()).or_default() +=
                        Self::value_at(&book, asset, *qty, &ctx.currency, *cutoff);
                }
                values
            })
            .collect();
        let mut ranking: Vec<(String, Amount)> = by_symbol
            .last()
            .map(|values| {
                values
                    .iter()
                    .map(|(symbol, value)| (symbol.clone(), *value))
                    .collect()
            })
            .unwrap_or_default();
        for values in &by_symbol {
            for symbol in values.keys() {
                if !ranking.iter().any(|(known, _)| known == symbol) {
                    ranking.push((symbol.clone(), Amount::ZERO));
                }
            }
        }
        ranking.sort_by(|left, right| right.1.cmp(&left.1).then(left.0.cmp(&right.0)));
        let mut assets: Vec<String> = ranking
            .iter()
            .take(PORTFOLIO_TOP)
            .map(|(symbol, _)| symbol.clone())
            .collect();
        let has_other = ranking.len() > PORTFOLIO_TOP;
        if has_other {
            assets.push(String::new());
        }
        let points = dates
            .iter()
            .zip(by_symbol)
            .map(|(date, values)| {
                let mut row: Vec<Amount> = assets
                    .iter()
                    .filter(|symbol| !symbol.is_empty())
                    .map(|symbol| values.get(symbol).copied().unwrap_or(Amount::ZERO))
                    .collect();
                if has_other {
                    let named: HashSet<&String> = assets.iter().collect();
                    row.push(
                        values
                            .iter()
                            .filter(|(symbol, _)| !named.contains(symbol))
                            .fold(Amount::ZERO, |sum, (_, value)| sum + *value),
                    );
                }
                let month = first_of_month(*date);
                let row: Vec<Amount> = row
                    .into_iter()
                    .map(|value| ctx.shown(value.round(2), month))
                    .collect();
                let total = row.iter().fold(Amount::ZERO, |sum, value| sum + *value);
                let mut shares: Vec<Amount> = row.iter().map(|value| pct(*value, total)).collect();
                // Rounding leaves the shares a hair off 100: give the rest to the biggest.
                if total.is_positive()
                    && let Some(biggest) = (0..shares.len()).max_by_key(|index| row[*index])
                {
                    let sum = shares.iter().fold(Amount::ZERO, |acc, share| acc + *share);
                    shares[biggest] += hundred() - sum;
                }
                PortfolioPoint {
                    date: date.format("%Y-%m-%d").to_string(),
                    total,
                    values: row,
                    shares,
                }
            })
            .collect();
        PortfolioReport {
            meta: ctx.meta(),
            assets,
            points,
        }
    }

    // -- inflation ------------------------------------------------------------------------

    fn inflation_report(&self, ctx: &Ctx) -> InflationReport {
        let nominal_ctx = Ctx {
            real: false,
            scope: ctx.scope.clone(),
            months: ctx.months.clone(),
            currency: ctx.currency.clone(),
            ..*ctx
        };
        let points: Vec<InflationPoint> = self
            .networth_points(&nominal_ctx)
            .into_iter()
            .zip(&ctx.months)
            .map(|(point, month)| {
                let coefficient = ctx.coefficient(*month).round(4);
                InflationPoint {
                    month: point.month,
                    coefficient,
                    nominal_net: point.net,
                    real_net: (point.net * coefficient).round(2),
                }
            })
            .collect();
        let change = |first: Amount, last: Amount| {
            (!first.is_zero() && first.is_positive()).then(|| pct(last - first, first))
        };
        let (nominal_change_pct, real_change_pct) = match (points.first(), points.last()) {
            (Some(first), Some(last)) => (
                change(first.nominal_net, last.nominal_net),
                change(first.real_net, last.real_net),
            ),
            _ => (None, None),
        };
        InflationReport {
            meta: ctx.meta(),
            points,
            nominal_change_pct,
            real_change_pct,
        }
    }

    // -- the calendar -----------------------------------------------------------------------

    /// Spending by day of a month (`YYYY-MM`) against the day's allowance,
    /// with what is still to come (plan Ф6).
    pub fn calendar_month(&self, month: &str, now: i64, zone: &str) -> Result<CalendarMonth> {
        let first = calendar::parse_month(month)
            .ok_or_else(|| Error::InvalidInput(format!("month {month}: expected YYYY-MM")))?;
        let cal = Calendar::new(zone);
        let today = cal.date_of(now);
        let currency = self.budget_currency();
        let (from, to) = cal.month_bounds(first);
        let flows = self.flows_between(&cal, &currency, from, to);
        let allowance = {
            let per_day = self.month_allowance(&cal, first, &currency, &flows).per_day;
            let manual = self.manual_day_limit(&currency, now);
            match (per_day.is_positive().then_some(per_day), manual) {
                (Some(plan), Some(limit)) => Some(plan.min(limit)),
                (plan, limit) => plan.or(limit),
            }
        };
        let mut tx_counts: HashMap<NaiveDate, u32> = HashMap::new();
        for tx in &self.doc.transactions {
            let secs = secs_or_now(&tx.meta().timestamp);
            if secs >= from && secs < to {
                *tx_counts.entry(cal.date_of(secs)).or_default() += 1;
            }
        }
        let mut days = Vec::new();
        let mut date = first;
        let end = next_month(first);
        while date < end {
            let of_day = |class: FlowClass| {
                flows
                    .iter()
                    .filter(|flow| flow.date == date && flow.class == class)
                    .fold(Amount::ZERO, |sum, flow| sum + flow.value)
            };
            let daily = of_day(FlowClass::Daily);
            let status = if date > today {
                DayStatus::Future
            } else {
                match allowance {
                    None => DayStatus::None,
                    Some(limit) if daily > limit => DayStatus::Over,
                    Some(limit) if daily * Amount::parse("1.25").unwrap_or(one()) > limit => {
                        DayStatus::Near
                    }
                    Some(_) => DayStatus::Under,
                }
            };
            days.push(CalendarDay {
                date: date.format("%Y-%m-%d").to_string(),
                income: of_day(FlowClass::Income),
                spent: of_day(FlowClass::Fixed) + daily,
                daily,
                tx_count: tx_counts.get(&date).copied().unwrap_or(0),
                status,
            });
            date += Duration::days(1);
        }
        let upcoming = if end > today {
            let days_left = (end - today).num_days().max(0) as u32;
            self.upcoming(days_left, now, zone)
                .into_iter()
                .filter(|item| {
                    calendar::parse_month(item.date.get(..7).unwrap_or_default()) == Some(first)
                })
                .collect()
        } else {
            Vec::new()
        };
        Ok(CalendarMonth {
            month: calendar::month_key(first),
            currency,
            allowance,
            days,
            upcoming,
        })
    }
}

// -- CSV ------------------------------------------------------------------------------------

/// One CSV field: quoted when it holds the separator, a quote or a line break.
fn csv_field(raw: &str) -> String {
    if raw.contains([';', '"', '\n', '\r']) {
        format!("\"{}\"", raw.replace('"', "\"\""))
    } else {
        raw.to_string()
    }
}

fn csv_line(fields: &[String]) -> String {
    fields
        .iter()
        .map(|field| csv_field(field))
        .collect::<Vec<_>>()
        .join(";")
}

/// A report as CSV for spreadsheets: UTF-8 with a BOM (so Excel picks the
/// encoding), `;` between fields, CRLF lines, a currency column on every row.
pub fn report_csv(report: &Report) -> String {
    let text = |value: &Amount| value.to_string();
    let mut lines: Vec<Vec<String>> = Vec::new();
    match report {
        Report::Cashflow(data) => {
            lines.push(
                [
                    "month",
                    "income",
                    "fixed",
                    "daily",
                    "savings_moved",
                    "budget_in",
                    "budget_out",
                    "net",
                    "currency",
                ]
                .map(String::from)
                .to_vec(),
            );
            for row in data.rows.iter().chain(std::iter::once(&data.total)) {
                lines.push(vec![
                    if row.month.is_empty() {
                        "total".into()
                    } else {
                        row.month.clone()
                    },
                    text(&row.income),
                    text(&row.fixed),
                    text(&row.daily),
                    text(&row.savings_moved),
                    text(&row.budget_in),
                    text(&row.budget_out),
                    text(&row.net),
                    data.meta.currency.clone(),
                ]);
            }
        }
        Report::Year(data) => {
            lines.push(
                [
                    "month",
                    "income",
                    "fixed",
                    "daily",
                    "savings_plan",
                    "savings_fact",
                    "deviation",
                    "saved_pct",
                    "currency",
                ]
                .map(String::from)
                .to_vec(),
            );
            let labelled = data.rows.iter().map(|row| (row.month.clone(), row)).chain([
                ("total".to_string(), &data.total),
                ("average".to_string(), &data.average),
            ]);
            for (label, row) in labelled {
                lines.push(vec![
                    label,
                    text(&row.income),
                    text(&row.fixed),
                    text(&row.daily),
                    text(&row.savings_plan),
                    text(&row.savings_fact),
                    text(&row.deviation),
                    text(&row.saved_pct),
                    data.meta.currency.clone(),
                ]);
            }
        }
        Report::Categories(data) => {
            let mut header = vec!["group".to_string(), "category".to_string()];
            header.extend(data.meta.months.iter().cloned());
            header.extend(
                ["total", "average", "share_pct", "delta_pct", "currency"].map(String::from),
            );
            lines.push(header);
            for block in &data.groups {
                let group = block
                    .group
                    .map(|group| format!("{group:?}").to_lowercase())
                    .unwrap_or_default();
                for row in &block.rows {
                    let mut line = vec![group.clone(), row.key.clone()];
                    line.extend(row.cells.iter().map(text));
                    line.extend([
                        text(&row.total),
                        text(&row.average),
                        text(&row.share),
                        row.delta_pct.as_ref().map(text).unwrap_or_default(),
                        data.meta.currency.clone(),
                    ]);
                    lines.push(line);
                }
            }
        }
        Report::Networth(data) => {
            lines.push(
                [
                    "month",
                    "financial",
                    "non_financial",
                    "receivables",
                    "liabilities",
                    "payables",
                    "net",
                    "currency",
                ]
                .map(String::from)
                .to_vec(),
            );
            for point in &data.points {
                lines.push(vec![
                    point.month.clone(),
                    text(&point.financial),
                    text(&point.non_financial),
                    text(&point.receivables),
                    text(&point.liabilities),
                    text(&point.payables),
                    text(&point.net),
                    data.meta.currency.clone(),
                ]);
            }
        }
        Report::Stores(data) => {
            lines.push(
                [
                    "wallet",
                    "depth",
                    "value_start",
                    "value_end",
                    "change",
                    "inflow",
                    "outflow",
                    "transactions",
                    "currency",
                ]
                .map(String::from)
                .to_vec(),
            );
            for row in &data.rows {
                lines.push(vec![
                    row.name.clone(),
                    row.depth.to_string(),
                    text(&row.value_start),
                    text(&row.value_end),
                    text(&row.change),
                    text(&row.inflow),
                    text(&row.outflow),
                    row.tx_count.to_string(),
                    data.meta.currency.clone(),
                ]);
            }
        }
        Report::Pnl(data) => {
            lines.push(
                ["month", "realized", "fees", "currency"]
                    .map(String::from)
                    .to_vec(),
            );
            for row in &data.rows {
                lines.push(vec![
                    row.month.clone(),
                    text(&row.realized),
                    text(&row.fees),
                    data.meta.currency.clone(),
                ]);
            }
        }
        Report::Portfolio(data) => {
            let mut header = vec!["date".to_string(), "total".to_string()];
            header.extend(data.assets.iter().map(|symbol| {
                if symbol.is_empty() {
                    "other".to_string()
                } else {
                    symbol.clone()
                }
            }));
            header.push("currency".into());
            lines.push(header);
            for point in &data.points {
                let mut line = vec![point.date.clone(), text(&point.total)];
                line.extend(point.values.iter().map(text));
                line.push(data.meta.currency.clone());
                lines.push(line);
            }
        }
        Report::Losses(data) => {
            lines.push(
                ["month", "fee", "slippage", "total", "currency"]
                    .map(String::from)
                    .to_vec(),
            );
            for row in data.rows.iter().chain(std::iter::once(&data.total)) {
                lines.push(vec![
                    if row.month.is_empty() {
                        "total".into()
                    } else {
                        row.month.clone()
                    },
                    text(&row.fee),
                    text(&row.slippage),
                    text(&row.total),
                    data.meta.currency.clone(),
                ]);
            }
        }
        Report::Inflation(data) => {
            lines.push(
                [
                    "month",
                    "coefficient",
                    "nominal_net",
                    "real_net",
                    "currency",
                ]
                .map(String::from)
                .to_vec(),
            );
            for point in &data.points {
                lines.push(vec![
                    point.month.clone(),
                    text(&point.coefficient),
                    text(&point.nominal_net),
                    text(&point.real_net),
                    data.meta.currency.clone(),
                ]);
            }
        }
    }
    let mut out = String::from("\u{feff}");
    for line in lines {
        out.push_str(&csv_line(&line));
        out.push_str("\r\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::store::{LiabilityKind, LiabilityTerms};
    use crate::model::transaction::{AccountRef, DebtEffect, Transaction};
    use crate::pricing::history::unix_secs;
    use crate::service::test_support::*;
    use crate::service::{ContactDraft, StoreExtras};

    fn secs(iso: &str) -> i64 {
        unix_secs(iso).unwrap()
    }

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

    fn usd_move(from: &str, to: &str, sum: &str, ts: &str) -> Transaction {
        dated(
            transfer_between(
                AccountRef::Store(from.into()),
                AccountRef::Store(to.into()),
                usd(),
                sum,
            ),
            ts,
        )
    }

    fn vault_with_card() -> (tempfile::TempDir, UnlockedVault, String) {
        let (dir, mut vault) = empty_vault();
        vault.set_base_currency("usd");
        quote_usd(&mut vault);
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        (dir, vault, card)
    }

    fn range(from: &str, to: &str) -> ReportFilter {
        ReportFilter {
            from: Some(from.into()),
            to: Some(to.into()),
            ..ReportFilter::default()
        }
    }

    const NOW: &str = "2026-06-15T12:00:00Z";

    fn run(vault: &UnlockedVault, kind: ReportKind, filter: &ReportFilter) -> Report {
        vault
            .report(kind, filter, secs(NOW), "UTC", &BTreeMap::new())
            .unwrap()
    }

    #[test]
    fn cashflow_leaves_internal_moves_out() {
        let (_dir, mut vault, card) = vault_with_card();
        let pocket = store(&mut vault, "Pocket", StoreKind::Bank, Some(&card));
        vault
            .add_transaction(earn(&card, "1000", "2026-03-01T10:00:00Z"))
            .unwrap();
        vault
            .add_transaction(usd_move(&card, &pocket, "300", "2026-03-02T10:00:00Z"))
            .unwrap();
        vault
            .add_transaction(spend(&card, "100", "groceries", "2026-04-02T10:00:00Z"))
            .unwrap();
        let Report::Cashflow(report) =
            run(&vault, ReportKind::Cashflow, &range("2026-03", "2026-04"))
        else {
            panic!()
        };
        assert_eq!(report.meta.months, vec!["2026-03", "2026-04"]);
        assert_eq!(report.rows[0].income, amount("1000"));
        assert_eq!(report.rows[0].daily, Amount::ZERO);
        assert_eq!(report.rows[1].daily, amount("100"));
        assert_eq!(report.total.net, amount("900"));
    }

    #[test]
    fn categories_compare_with_the_previous_period_and_keep_ties_stable() {
        let (_dir, mut vault, card) = vault_with_card();
        vault
            .add_transaction(spend(&card, "100", "groceries", "2026-02-10T10:00:00Z"))
            .unwrap();
        vault
            .add_transaction(spend(&card, "150", "groceries", "2026-03-10T10:00:00Z"))
            .unwrap();
        for key in ["taxi", "fuel", "sport", "pets", "gifts", "beauty"] {
            vault
                .add_transaction(spend(&card, "50", key, "2026-03-11T10:00:00Z"))
                .unwrap();
        }
        let Report::Categories(report) =
            run(&vault, ReportKind::Categories, &range("2026-03", "2026-03"))
        else {
            panic!()
        };
        let daily = report
            .groups
            .iter()
            .find(|block| block.group == Some(CategoryGroup::Daily))
            .unwrap();
        let groceries = daily
            .rows
            .iter()
            .find(|row| row.key == "groceries")
            .unwrap();
        assert_eq!(groceries.delta_pct, Some(amount("50")));
        assert_eq!(groceries.share, amount("33.3"));
        assert_eq!(
            daily
                .rows
                .iter()
                .find(|row| row.key == "taxi")
                .unwrap()
                .delta_pct,
            None
        );
        let top: Vec<&str> = report.top.iter().map(|row| row.key.as_str()).collect();
        assert_eq!(top, vec!["groceries", "beauty", "fuel", "gifts", "pets"]);
    }

    #[test]
    fn year_average_skips_empty_months_and_names_the_best_and_worst() {
        let (_dir, mut vault, card) = vault_with_card();
        vault.doc.settings.savings_rate = Amount::ZERO;
        vault
            .add_transaction(earn(&card, "1000", "2026-01-05T10:00:00Z"))
            .unwrap();
        vault
            .add_transaction(spend(&card, "900", "groceries", "2026-01-06T10:00:00Z"))
            .unwrap();
        vault
            .add_transaction(earn(&card, "1000", "2026-03-05T10:00:00Z"))
            .unwrap();
        vault
            .add_transaction(spend(&card, "200", "groceries", "2026-03-06T10:00:00Z"))
            .unwrap();
        let filter = ReportFilter {
            year: Some(2026),
            ..ReportFilter::default()
        };
        let Report::Year(report) = run(&vault, ReportKind::Year, &filter) else {
            panic!()
        };
        assert_eq!(report.rows.len(), 12);
        assert!(report.rows[1].empty);
        assert_eq!(report.average.income, amount("1000"));
        assert_eq!(report.average.daily, amount("550"));
        assert_eq!(report.best_month.as_deref(), Some("2026-03"));
        assert_eq!(report.worst_month.as_deref(), Some("2026-01"));
        assert_eq!(report.total.savings_fact, amount("900"));
    }

    #[test]
    fn net_worth_counts_loans_and_debts() {
        let (_dir, mut vault, card) = vault_with_card();
        let loan = vault
            .create_store_with(
                "Loan",
                StoreKind::Liability,
                "",
                "",
                None,
                StoreExtras {
                    liability: Some(LiabilityTerms {
                        kind: LiabilityKind::Loan,
                        rate: None,
                        limit: None,
                        due_day: None,
                        monthly_payment: None,
                    }),
                    ..StoreExtras::default()
                },
            )
            .unwrap()
            .id;
        vault
            .add_transaction(earn(&card, "1000", "2026-03-01T10:00:00Z"))
            .unwrap();
        vault
            .add_transaction(usd_move(&loan, &card, "400", "2026-04-01T10:00:00Z"))
            .unwrap();
        let friend = vault
            .create_contact(ContactDraft {
                name: "Friend".into(),
                kind: crate::model::contact::ContactKind::Person,
                note: String::new(),
                accounts: Vec::new(),
            })
            .unwrap()
            .id;
        let mut lend = transfer_between(
            AccountRef::Store(card.clone()),
            AccountRef::Contact {
                contact_id: friend,
                account_id: None,
            },
            usd(),
            "100",
        );
        if let Transaction::Transfer(tx) = &mut lend {
            tx.debt = Some(DebtEffect::Lend);
        }
        vault
            .add_transaction(dated(lend, "2026-05-01T10:00:00Z"))
            .unwrap();

        let Report::Networth(report) =
            run(&vault, ReportKind::Networth, &range("2026-03", "2026-05"))
        else {
            panic!()
        };
        let [march, april, may] = &report.points[..] else {
            panic!()
        };
        assert_eq!(march.net, amount("1000"));
        assert_eq!(
            (april.financial, april.liabilities, april.net),
            (amount("1400"), amount("400"), amount("1000"))
        );
        assert_eq!(
            (may.financial, may.receivables, may.net),
            (amount("1300"), amount("100"), amount("1000"))
        );
    }

    #[test]
    fn portfolio_shares_add_up_to_a_hundred() {
        let (_dir, mut vault, card) = vault_with_card();
        let wallet = store(&mut vault, "Cold", StoreKind::Crypto, None);
        vault
            .add_transaction(earn(&card, "1000", "2026-03-01T10:00:00Z"))
            .unwrap();
        vault
            .add_transaction(dated(
                income_of(&wallet, btc(), "0.02", "60000", "bought"),
                "2026-03-02T10:00:00Z",
            ))
            .unwrap();
        let Report::Portfolio(report) =
            run(&vault, ReportKind::Portfolio, &range("2026-05", "2026-06"))
        else {
            panic!()
        };
        assert_eq!(report.assets, vec!["BTC", "USD"]);
        for point in &report.points {
            let sum = point
                .shares
                .iter()
                .fold(Amount::ZERO, |acc, share| acc + *share);
            assert_eq!(sum, amount("100"), "{point:?}");
        }
    }

    #[test]
    fn real_terms_restate_old_months_in_todays_money() {
        let (_dir, mut vault, card) = vault_with_card();
        vault
            .add_transaction(earn(&card, "1000", "2025-06-05T10:00:00Z"))
            .unwrap();
        let inflation = BTreeMap::from([("usd".to_string(), amount("10"))]);
        let filter = ReportFilter {
            real_terms: true,
            ..range("2025-06", "2026-06")
        };
        let Report::Cashflow(report) = vault
            .report(ReportKind::Cashflow, &filter, secs(NOW), "UTC", &inflation)
            .unwrap()
        else {
            panic!()
        };
        // A year ago: 1000 × 1.1.
        assert_eq!(report.rows[0].income, amount("1100"));
        assert_eq!(report.meta.inflation_rate, Some(amount("10")));
        let Report::Inflation(inflation_report) = vault
            .report(ReportKind::Inflation, &filter, secs(NOW), "UTC", &inflation)
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(inflation_report.points[0].coefficient, amount("1.1"));
        assert_eq!(
            inflation_report.points.last().unwrap().coefficient,
            amount("1")
        );
        assert_eq!(inflation_report.nominal_change_pct, Some(Amount::ZERO));
        assert!(inflation_report.real_change_pct.unwrap().is_negative());
    }

    #[test]
    fn csv_has_a_bom_semicolons_and_quotes_what_needs_it() {
        let report = Report::Stores(StoresReport {
            meta: ReportMeta {
                currency: "usd".into(),
                ..ReportMeta::default()
            },
            rows: vec![StoreReportRow {
                name: "Card; \"main\"".into(),
                ..StoreReportRow::default()
            }],
        });
        let csv = report_csv(&report);
        assert!(csv.starts_with('\u{feff}'));
        let lines: Vec<&str> = csv.trim_start_matches('\u{feff}').split("\r\n").collect();
        assert!(lines[0].starts_with("wallet;depth;"));
        assert!(lines[1].starts_with("\"Card; \"\"main\"\"\";0;"));
        assert!(lines[1].ends_with(";usd"));
    }

    #[test]
    fn the_calendar_cuts_days_in_the_local_zone() {
        let (_dir, mut vault, card) = vault_with_card();
        vault
            .add_transaction(earn(&card, "3100", "2026-03-01T08:00:00Z"))
            .unwrap();
        // 20:00 UTC on the 1st is the 2nd in Bangkok.
        vault
            .add_transaction(spend(&card, "150", "groceries", "2026-03-01T20:00:00Z"))
            .unwrap();
        let now = secs("2026-03-20T12:00:00Z");
        let utc = vault.calendar_month("2026-03", now, "UTC").unwrap();
        let bangkok = vault
            .calendar_month("2026-03", now, "Asia/Bangkok")
            .unwrap();
        assert_eq!(utc.days[0].daily, amount("150"));
        assert_eq!(bangkok.days[0].daily, Amount::ZERO);
        assert_eq!(bangkok.days[1].daily, amount("150"));
        // 3100 a month = 100 a day (savings 10 %: 90); 150 is over.
        assert_eq!(utc.allowance, Some(amount("90")));
        assert_eq!(utc.days[0].status, DayStatus::Over);
        assert_eq!(utc.days[2].status, DayStatus::Under);
        assert_eq!(utc.days[25].status, DayStatus::Future);
        assert_eq!(utc.days[0].tx_count, 2);
    }
}
