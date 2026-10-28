//! Compound-interest engine behind the investment calculator (plan, block A).
//!
//! Pure and vault-independent: the UI sends an [`InvestInput`] and gets a
//! month-by-month schedule back. The schedule is a plain monthly loop rather
//! than closed formulas, so contribution stages, indexation, tax and
//! withdrawals combine without special cases. All arithmetic is decimal;
//! powers come from [`Amount::pow_int`] / [`Amount::root_n`] because the
//! `maths` feature of `rust_decimal` is off.
//!
//! Order of events inside month `m` (1-based):
//! 1. contributions with [`Timing::Start`];
//! 2. interest for the month (daily compounding uses the real calendar from
//!    [`InvestInput::start`]);
//! 3. capitalization, when the frequency falls on `m` (always in the last month);
//! 4. tax on what was capitalized ([`TaxMode::Yearly`] every 12th and the
//!    last month, [`TaxMode::AtEnd`] in the last month only);
//! 5. contributions with [`Timing::End`];
//! 6. the withdrawal; one larger than the balance empties it and stops
//!    further withdrawals ([`InvestResult::depleted_at`]).

use chrono::{Months, NaiveDate};
use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};

use crate::amount::Amount;
use crate::calendar::days_in_month;
use crate::error::{Error, Result};

/// The longest term the calculator plans for: 100 years.
pub const MAX_TERM_MONTHS: u32 = 1200;
/// The bounds of [`SolveUnknown::Rate`], in percent per year.
const RATE_SOLVE_MAX_PCT: Decimal = Decimal::from_parts(200, 0, 0, false, 0);
/// «Что выгоднее?»: the rate bump (percentage points) and the contribution factor.
const COMPARE_RATE_BUMP: Decimal = Decimal::TWO;
const COMPARE_CONTRIBUTION_FACTOR: Decimal = Decimal::from_parts(12, 0, 0, false, 1);
const HUNDRED: Decimal = Decimal::ONE_HUNDRED;
const TWELVE: Decimal = Decimal::from_parts(12, 0, 0, false, 0);
const DAYS_IN_YEAR: Decimal = Decimal::from_parts(365, 0, 0, false, 0);
const BISECTION_STEPS: u32 = 80;
/// How far past the estimated month [`SolveUnknown::Term`] checks exactly.
const TERM_VERIFY_STEPS: u32 = 36;

/// How often earned interest joins the balance.
#[derive(
    Default,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    ts_rs::TS,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum CapFreq {
    Daily,
    #[default]
    Monthly,
    Quarterly,
    Yearly,
    /// Simple interest, paid in the last month.
    AtEnd,
}

/// How often a contribution or a withdrawal repeats, counted from its start month.
#[derive(
    Default,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    ts_rs::TS,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Freq {
    #[default]
    Monthly,
    Quarterly,
    Yearly,
}

/// Whether a contribution arrives before the month's interest or after it.
#[derive(
    Default,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    ts_rs::TS,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Timing {
    Start,
    #[default]
    End,
}

#[derive(
    Default,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    ts_rs::TS,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum TaxMode {
    /// Every year on the interest capitalized during it.
    #[default]
    Yearly,
    /// Once, on all interest, in the last month.
    AtEnd,
}

/// Tax on interest; `pct = 0` means no tax.
#[derive(
    Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, schemars::JsonSchema,
)]
#[serde(default)]
pub struct TaxRule {
    pub pct: Amount,
    pub mode: TaxMode,
}

/// A contribution plan from `from_month` until the next stage starts.
#[derive(
    Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, schemars::JsonSchema,
)]
#[serde(default)]
pub struct ContributionStage {
    /// 1-based month the stage starts in.
    pub from_month: u32,
    pub amount: Amount,
    pub freq: Freq,
    pub timing: Timing,
}

/// Regular withdrawals; they stop once the balance runs out.
#[derive(
    Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, schemars::JsonSchema,
)]
#[serde(default)]
pub struct Withdrawal {
    pub amount: Amount,
    pub freq: Freq,
    /// 1-based month of the first withdrawal.
    pub from_month: u32,
    /// Grow the amount with [`InvestInput::inflation_pct`] every full year.
    pub index_to_inflation: bool,
}

#[derive(
    Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, schemars::JsonSchema,
)]
#[serde(default)]
pub struct InvestInput {
    /// Display only: the currency every amount is in.
    pub currency: String,
    /// `YYYY-MM` of month 1: daily compounding counts its real days.
    pub start: String,
    pub initial: Amount,
    /// Interest per year, percent.
    pub rate_pct: Amount,
    pub term_months: u32,
    pub capitalization: CapFreq,
    /// Sorted by the engine; empty means no contributions.
    pub stages: Vec<ContributionStage>,
    /// Growth of a stage's contribution per full year inside the stage, percent.
    pub indexation_pct: Amount,
    /// Inflation per year, percent: the real result and indexed withdrawals.
    pub inflation_pct: Amount,
    pub tax: TaxRule,
    pub withdrawal: Option<Withdrawal>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct InvestMonth {
    /// 1-based.
    pub month: u32,
    /// `YYYY-MM`.
    pub date: String,
    pub contributed: Amount,
    /// Interest capitalized this month.
    pub interest: Amount,
    pub tax: Amount,
    pub withdrawn: Amount,
    /// After every event of the month; interest not yet capitalized is not in it.
    pub balance: Amount,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct InvestYear {
    /// 1-based; the last year may be partial.
    pub year: u32,
    pub contributed: Amount,
    pub interest: Amount,
    pub tax: Amount,
    pub withdrawn: Amount,
    /// `initial` plus every contribution so far.
    pub invested: Amount,
    pub balance: Amount,
    /// `balance` in money of month 1.
    pub real_balance: Amount,
    pub months: Vec<InvestMonth>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct InvestResult {
    pub final_nominal: Amount,
    /// `final_nominal` in money of month 1.
    pub final_real: Amount,
    /// Contributions only; `initial` is not in it.
    pub contributed: Amount,
    /// `initial + contributed`.
    pub invested: Amount,
    pub interest: Amount,
    pub tax: Amount,
    pub withdrawn: Amount,
    /// The yearly rate the capitalization turns `rate_pct` into, percent.
    pub effective_rate_pct: Amount,
    /// The first year whose interest exceeds its contributions.
    pub crossover_year: Option<u32>,
    /// The month a withdrawal emptied the balance.
    pub depleted_at: Option<u32>,
    pub years: Vec<InvestYear>,
}

/// What [`solve`] looks for.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum SolveUnknown {
    /// The first stage's amount; every stage scales by the same factor.
    #[default]
    Contribution,
    /// Months.
    Term,
    /// Percent per year.
    Rate,
}

/// The answer of [`solve`]. When the target cannot be reached, `value` and
/// `result` describe the best the unknown can do, and the UI says so in text.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct SolveResult {
    pub reachable: bool,
    pub value: Amount,
    /// The input with the unknown filled in.
    pub input: InvestInput,
    pub result: InvestResult,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum CompareKind {
    /// The input as it is.
    #[default]
    Base,
    /// Two percentage points more: more risk or more time spent choosing.
    MoreRate,
    /// Every contribution 20% larger.
    MoreContribution,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct CompareRow {
    pub kind: CompareKind,
    pub final_nominal: Amount,
    /// Against [`CompareKind::Base`].
    pub gain: Amount,
}

/// «Что выгоднее?»: a higher rate against higher contributions.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct InvestCompare {
    pub rows: Vec<CompareRow>,
    /// Larger contributions beat the higher rate: the calmer way wins.
    pub contribution_wins: bool,
}

/// The schedule of `input`.
pub fn simulate(input: &InvestInput) -> Result<InvestResult> {
    let start = validate(input)?;
    let run = Run::new(input, start, input.term_months).ok_or_else(too_large)?;
    Ok(run.result(input))
}

/// The value of `unknown` that brings `input` to `target` nominal.
pub fn solve(input: &InvestInput, target: Amount, unknown: SolveUnknown) -> Result<SolveResult> {
    let start = validate(input)?;
    if !target.is_positive() {
        return Err(Error::InvalidInput("target must be positive".into()));
    }
    let target = target.into_inner();
    match unknown {
        SolveUnknown::Contribution => solve_contribution(input, start, target),
        SolveUnknown::Term => solve_term(input, start, target),
        SolveUnknown::Rate => solve_rate(input, start, target),
    }
}

/// The input against `rate + 2` and against contributions × 1.2.
pub fn compare(input: &InvestInput) -> Result<InvestCompare> {
    let base = simulate(input)?.final_nominal;
    let mut more_rate = input.clone();
    more_rate.rate_pct = Amount(input.rate_pct.0 + COMPARE_RATE_BUMP);
    let more_rate = simulate(&more_rate)?.final_nominal;
    let more_contribution =
        simulate(&scaled(input, COMPARE_CONTRIBUTION_FACTOR, false))?.final_nominal;
    let row = |kind, final_nominal: Amount| CompareRow {
        kind,
        final_nominal,
        gain: final_nominal - base,
    };
    Ok(InvestCompare {
        contribution_wins: more_contribution > more_rate,
        rows: vec![
            row(CompareKind::Base, base),
            row(CompareKind::MoreRate, more_rate),
            row(CompareKind::MoreContribution, more_contribution),
        ],
    })
}

/// The largest withdrawal, in whole units, that the plan pays to the end of the
/// term; `None` when the input withdraws nothing or never runs out.
pub fn safe_withdrawal(input: &InvestInput) -> Result<Option<Amount>> {
    let start = validate(input)?;
    let Some(withdrawal) = &input.withdrawal else {
        return Ok(None);
    };
    let with_amount = |amount: Decimal| {
        let mut out = input.clone();
        if let Some(rule) = &mut out.withdrawal {
            rule.amount = Amount(amount);
        }
        out
    };
    let lasts = |amount: Decimal| {
        Run::new(&with_amount(amount), start, input.term_months)
            .is_none_or(|run| run.depleted_at.is_none())
    };
    if lasts(withdrawal.amount.0) {
        return Ok(None);
    }
    let (mut low, mut high) = (Decimal::ZERO, withdrawal.amount.0);
    for _ in 0..BISECTION_STEPS {
        if high - low < Decimal::ONE {
            break;
        }
        let middle = ((low + high) / Decimal::TWO).floor();
        if lasts(middle) {
            low = middle;
        } else {
            high = middle;
        }
    }
    Ok(Some(Amount(low)))
}

fn too_large() -> Error {
    Error::InvalidInput("the result is too large to compute".into())
}

fn validate(input: &InvestInput) -> Result<NaiveDate> {
    let start = NaiveDate::parse_from_str(&format!("{}-01", input.start.trim()), "%Y-%m-%d")
        .map_err(|_| Error::InvalidInput(format!("bad start month: {}", input.start)))?;
    let bad = |what: &str| Err(Error::InvalidInput(format!("bad {what}")));
    let above_minus_hundred = |value: Amount| value.0 > -HUNDRED;
    if input.term_months > MAX_TERM_MONTHS {
        return bad("term");
    }
    if input.initial.is_negative() {
        return bad("initial amount");
    }
    if !above_minus_hundred(input.rate_pct) || input.rate_pct.0 > HUNDRED * Decimal::TEN {
        return bad("rate");
    }
    if !above_minus_hundred(input.indexation_pct) || !above_minus_hundred(input.inflation_pct) {
        return bad("indexation or inflation");
    }
    if input.tax.pct.is_negative() || input.tax.pct.0 > HUNDRED {
        return bad("tax");
    }
    if input
        .stages
        .iter()
        .any(|stage| stage.from_month == 0 || stage.amount.is_negative())
    {
        return bad("contribution stage");
    }
    if let Some(withdrawal) = &input.withdrawal
        && (withdrawal.from_month == 0 || withdrawal.amount.is_negative())
    {
        return bad("withdrawal");
    }
    Ok(start)
}

/// Whether a schedule that starts in `from` falls on month `month`.
fn due(freq: Freq, from: u32, month: u32) -> bool {
    let offset = month - from;
    match freq {
        Freq::Monthly => true,
        Freq::Quarterly => offset.is_multiple_of(3),
        Freq::Yearly => offset.is_multiple_of(12),
    }
}

/// One month of the schedule, before rounding.
#[derive(Default, Clone, Copy)]
struct Step {
    contributed: Decimal,
    interest: Decimal,
    tax: Decimal,
    withdrawn: Decimal,
    balance: Decimal,
}

/// A computed schedule over `horizon` months.
struct Run {
    start: NaiveDate,
    initial: Decimal,
    steps: Vec<Step>,
    /// What closing the plan right after each month would pay out: the
    /// balance plus pending interest, less the tax still due on it.
    closeouts: Vec<Decimal>,
    /// The monthly growth factor of prices.
    inflation_month: Decimal,
    depleted_at: Option<u32>,
}

impl Run {
    /// `None` when a number outgrows a decimal.
    fn new(input: &InvestInput, start: NaiveDate, horizon: u32) -> Option<Run> {
        let rate = input.rate_pct.0 / HUNDRED;
        let tax_pct = input.tax.pct.0 / HUNDRED;
        let indexation = Amount(Decimal::ONE + input.indexation_pct.0 / HUNDRED);
        let inflation = Amount(Decimal::ONE + input.inflation_pct.0 / HUNDRED);
        let mut stages = input.stages.clone();
        stages.sort_by_key(|stage| stage.from_month);
        let contribution = |month: u32, timing: Timing| -> Option<Decimal> {
            let stage = stages
                .iter()
                .rev()
                .find(|stage| stage.from_month <= month)?;
            if stage.timing != timing || !due(stage.freq, stage.from_month, month) {
                return Some(Decimal::ZERO);
            }
            stage.amount.0.checked_mul(
                indexation
                    .checked_pow_int((month - stage.from_month) / 12)?
                    .0,
            )
        };

        let mut run = Run {
            start,
            initial: input.initial.0,
            steps: Vec::with_capacity(horizon as usize),
            closeouts: Vec::with_capacity(horizon as usize),
            inflation_month: inflation.root_n(12).0,
            depleted_at: None,
        };
        let mut balance = input.initial.0;
        let mut accrued = Decimal::ZERO;
        let mut taxable = Decimal::ZERO;
        let mut interest_total = Decimal::ZERO;
        for month in 1..=horizon {
            let last = month == horizon;
            let mut step = Step::default();
            let early = contribution(month, Timing::Start).unwrap_or(Decimal::ZERO);
            balance = balance.checked_add(early)?;
            step.contributed += early;

            if input.capitalization == CapFreq::Daily {
                let date = start.checked_add_months(Months::new(month - 1))?;
                let daily = Amount(Decimal::ONE + rate / DAYS_IN_YEAR);
                let growth = daily.checked_pow_int(days_in_month(date))?.0 - Decimal::ONE;
                accrued = accrued.checked_add(balance.checked_mul(growth)?)?;
            } else {
                accrued = accrued.checked_add(balance.checked_mul(rate)? / TWELVE)?;
            }
            let capitalize = last
                || match input.capitalization {
                    CapFreq::Daily | CapFreq::Monthly => true,
                    CapFreq::Quarterly => month.is_multiple_of(3),
                    CapFreq::Yearly => month.is_multiple_of(12),
                    CapFreq::AtEnd => false,
                };
            if capitalize {
                balance = balance.checked_add(accrued)?;
                step.interest = accrued;
                taxable += accrued;
                interest_total += accrued;
                accrued = Decimal::ZERO;
            }

            let tax_now = match input.tax.mode {
                TaxMode::Yearly => last || month.is_multiple_of(12),
                TaxMode::AtEnd => last,
            };
            if tax_now && !tax_pct.is_zero() {
                let base = match input.tax.mode {
                    TaxMode::Yearly => taxable,
                    TaxMode::AtEnd => interest_total,
                };
                step.tax = (base.max(Decimal::ZERO) * tax_pct).min(balance.max(Decimal::ZERO));
                balance -= step.tax;
            }
            if tax_now {
                taxable = Decimal::ZERO;
            }

            let late = contribution(month, Timing::End).unwrap_or(Decimal::ZERO);
            balance = balance.checked_add(late)?;
            step.contributed += late;

            if let Some(withdrawal) = &input.withdrawal
                && run.depleted_at.is_none()
                && month >= withdrawal.from_month
                && due(withdrawal.freq, withdrawal.from_month, month)
            {
                let mut amount = withdrawal.amount.0;
                if withdrawal.index_to_inflation {
                    amount = amount.checked_mul(
                        inflation
                            .checked_pow_int((month - withdrawal.from_month) / 12)?
                            .0,
                    )?;
                }
                if amount >= balance && amount.is_sign_positive() && !amount.is_zero() {
                    step.withdrawn = balance.max(Decimal::ZERO);
                    run.depleted_at = Some(month);
                } else {
                    step.withdrawn = amount;
                }
                balance -= step.withdrawn;
            }

            step.balance = balance;
            let pending_base = match input.tax.mode {
                TaxMode::Yearly => taxable + accrued,
                TaxMode::AtEnd => interest_total + accrued,
            };
            let pending_tax = if last {
                Decimal::ZERO
            } else {
                pending_base.max(Decimal::ZERO) * tax_pct
            };
            run.closeouts.push(balance + accrued - pending_tax);
            run.steps.push(step);
        }
        Some(run)
    }

    fn final_balance(&self) -> Decimal {
        self.steps.last().map_or(self.initial, |step| step.balance)
    }

    fn result(&self, input: &InvestInput) -> InvestResult {
        let money = |value: Decimal| Amount(value).round(2);
        let mut years: Vec<InvestYear> = Vec::new();
        let mut invested = self.initial;
        let mut deflator = Decimal::ONE;
        for (index, step) in self.steps.iter().enumerate() {
            let month = index as u32 + 1;
            invested += step.contributed;
            deflator = deflator
                .checked_mul(self.inflation_month)
                .unwrap_or(Decimal::MAX);
            if (month - 1).is_multiple_of(12) {
                years.push(InvestYear {
                    year: (month - 1) / 12 + 1,
                    ..InvestYear::default()
                });
            }
            let Some(year) = years.last_mut() else {
                continue;
            };
            year.contributed += money(step.contributed);
            year.interest += money(step.interest);
            year.tax += money(step.tax);
            year.withdrawn += money(step.withdrawn);
            year.invested = money(invested);
            year.balance = money(step.balance);
            year.real_balance = money(step.balance / deflator);
            let date = self
                .start
                .checked_add_months(Months::new(month - 1))
                .unwrap_or(self.start);
            year.months.push(InvestMonth {
                month,
                date: date.format("%Y-%m").to_string(),
                contributed: money(step.contributed),
                interest: money(step.interest),
                tax: money(step.tax),
                withdrawn: money(step.withdrawn),
                balance: money(step.balance),
            });
        }
        let sum = |pick: fn(&Step) -> Decimal| money(self.steps.iter().map(pick).sum());
        let contributed = sum(|step| step.contributed);
        InvestResult {
            final_nominal: money(self.final_balance()),
            final_real: money(self.final_balance() / deflator),
            contributed,
            invested: money(self.initial) + contributed,
            interest: sum(|step| step.interest),
            tax: sum(|step| step.tax),
            withdrawn: sum(|step| step.withdrawn),
            effective_rate_pct: effective_rate_pct(input),
            crossover_year: years
                .iter()
                .find(|year| year.contributed.is_positive() && year.interest > year.contributed)
                .map(|year| year.year),
            depleted_at: self.depleted_at,
            years,
        }
    }
}

/// `(1 + r/n)^n − 1` for the capitalization frequency; simple interest over
/// the whole term for [`CapFreq::AtEnd`].
fn effective_rate_pct(input: &InvestInput) -> Amount {
    let rate = input.rate_pct.0 / HUNDRED;
    let periodic = |periods: u32| {
        Amount(Decimal::ONE + rate / Decimal::from(periods))
            .pow_int(periods)
            .0
    };
    let growth = match input.capitalization {
        CapFreq::Daily => periodic(365),
        CapFreq::Monthly => periodic(12),
        CapFreq::Quarterly => periodic(4),
        CapFreq::Yearly => Decimal::ONE + rate,
        CapFreq::AtEnd if input.term_months == 0 => Decimal::ONE + rate,
        CapFreq::AtEnd => {
            let term = Decimal::from(input.term_months);
            Amount(Decimal::ONE + rate * term / TWELVE)
                .root_n(input.term_months)
                .pow_int(12)
                .0
        }
    };
    Amount((growth - Decimal::ONE) * HUNDRED).round(2)
}

/// The final nominal, or `None` when it outgrows a decimal (larger than any target).
fn final_value(input: &InvestInput, start: NaiveDate) -> Option<Decimal> {
    Run::new(input, start, input.term_months).map(|run| run.final_balance())
}

fn reaches(input: &InvestInput, start: NaiveDate, target: Decimal) -> bool {
    final_value(input, start).is_none_or(|value| value >= target)
}

fn solved(input: InvestInput, value: Amount, reachable: bool) -> Result<SolveResult> {
    let result = simulate(&input)?;
    Ok(SolveResult {
        reachable,
        value,
        input,
        result,
    })
}

/// Every stage's amount × `factor`, rounded up to cents when `ceil`. With no
/// stages (or only empty ones) the stages become «1 a month from month 1»,
/// so a factor has something to scale.
fn scaled(input: &InvestInput, factor: Decimal, ceil: bool) -> InvestInput {
    let mut out = input.clone();
    if out.stages.iter().all(|stage| stage.amount.is_zero()) {
        if out.stages.is_empty() {
            out.stages.push(ContributionStage {
                from_month: 1,
                ..ContributionStage::default()
            });
        }
        if !ceil && factor != Decimal::ZERO {
            // Comparing (not solving) an input without contributions: keep it so.
            return input.clone();
        }
        for stage in &mut out.stages {
            stage.amount = Amount(Decimal::ONE);
        }
    }
    for stage in &mut out.stages {
        let value = stage.amount.0 * factor;
        stage.amount = Amount(if ceil {
            value.round_dp_with_strategy(2, RoundingStrategy::ToPositiveInfinity)
        } else {
            value
        });
    }
    out
}

fn solve_contribution(
    input: &InvestInput,
    start: NaiveDate,
    target: Decimal,
) -> Result<SolveResult> {
    let at = |factor: Decimal| scaled(input, factor, true);
    let first_amount = |solved: &InvestInput| {
        let mut stages = solved.stages.clone();
        stages.sort_by_key(|stage| stage.from_month);
        stages.first().map_or(Amount::ZERO, |stage| stage.amount)
    };
    if reaches(&at(Decimal::ZERO), start, target) {
        let zero = at(Decimal::ZERO);
        return solved(zero, Amount::ZERO, true);
    }
    let mut high = Decimal::ONE;
    let mut doublings = 0;
    while !reaches(&at(high), start, target) {
        if doublings == 40 {
            return solved(input.clone(), first_amount(input), false);
        }
        high *= Decimal::TWO;
        doublings += 1;
    }
    let mut low = Decimal::ZERO;
    for _ in 0..BISECTION_STEPS {
        let middle = (low + high) / Decimal::TWO;
        if reaches(&at(middle), start, target) {
            high = middle;
        } else {
            low = middle;
        }
    }
    let answer = at(high);
    let value = first_amount(&answer);
    solved(answer, value, true)
}

fn solve_term(input: &InvestInput, start: NaiveDate, target: Decimal) -> Result<SolveResult> {
    let with_term = |months: u32| InvestInput {
        term_months: months,
        ..input.clone()
    };
    if input.initial.0 >= target {
        return solved(with_term(0), Amount::ZERO, true);
    }
    // One run over the longest horizon estimates every term at once; the
    // estimate is then checked with exact runs (the last month differs only
    // by the capitalization and tax a real end brings).
    let horizon = Run::new(input, start, MAX_TERM_MONTHS);
    let closeouts = horizon
        .as_ref()
        .map(|run| run.closeouts.as_slice())
        .unwrap_or_default();
    let estimate = closeouts
        .iter()
        .position(|value| *value >= target)
        .map_or(closeouts.len() as u32 + 1, |index| index as u32 + 1)
        .min(MAX_TERM_MONTHS);
    for months in estimate..=(estimate + TERM_VERIFY_STEPS).min(MAX_TERM_MONTHS) {
        if reaches(&with_term(months), start, target) {
            return solved(with_term(months), Amount::from(months), true);
        }
    }
    let best = closeouts
        .iter()
        .enumerate()
        .max_by(|left, right| left.1.cmp(right.1))
        .map_or(MAX_TERM_MONTHS, |(index, _)| index as u32 + 1);
    solved(with_term(best), Amount::from(best), false)
}

fn solve_rate(input: &InvestInput, start: NaiveDate, target: Decimal) -> Result<SolveResult> {
    let with_rate = |rate: Decimal| InvestInput {
        rate_pct: Amount(rate),
        ..input.clone()
    };
    if reaches(&with_rate(Decimal::ZERO), start, target) {
        return solved(with_rate(Decimal::ZERO), Amount::ZERO, true);
    }
    if !reaches(&with_rate(RATE_SOLVE_MAX_PCT), start, target) {
        return solved(
            with_rate(RATE_SOLVE_MAX_PCT),
            Amount(RATE_SOLVE_MAX_PCT),
            false,
        );
    }
    let mut low = Decimal::ZERO;
    let mut high = RATE_SOLVE_MAX_PCT;
    for _ in 0..BISECTION_STEPS {
        let middle = (low + high) / Decimal::TWO;
        if reaches(&with_rate(middle), start, target) {
            high = middle;
        } else {
            low = middle;
        }
    }
    let rate = high.round_dp_with_strategy(4, RoundingStrategy::ToPositiveInfinity);
    solved(with_rate(rate), Amount(rate), true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn amt(raw: &str) -> Amount {
        Amount::parse(raw).unwrap()
    }

    fn assert_amt(actual: Amount, expected: &str) {
        assert_eq!(actual, amt(expected), "expected {expected}, got {actual}");
    }

    fn monthly(amount: &str) -> ContributionStage {
        ContributionStage {
            from_month: 1,
            amount: amt(amount),
            ..ContributionStage::default()
        }
    }

    fn base(initial: &str, rate: &str, months: u32) -> InvestInput {
        InvestInput {
            currency: "RUB".into(),
            start: "2025-01".into(),
            initial: amt(initial),
            rate_pct: amt(rate),
            term_months: months,
            ..InvestInput::default()
        }
    }

    // Reference values are the closed forms of the same plans (`FV = PV·(1+i)^n`,
    // the annuity `P·((1+i)^n − 1)/i`), computed to 40 digits with Python's
    // `decimal` (the calculator on calcus.ru cannot be queried without a browser).

    #[test]
    fn lump_sum_monthly_capitalization() {
        let res = simulate(&base("1000000", "10", 12)).unwrap();
        assert_amt(res.final_nominal, "1104713.07");
        assert_amt(res.interest, "104713.07");
        assert_amt(res.effective_rate_pct, "10.47");
        assert_eq!(res.years.len(), 1);
        assert_eq!(res.years[0].months.len(), 12);
        assert_eq!(res.years[0].months[11].date, "2025-12");
    }

    #[test]
    fn monthly_contributions_annuity() {
        let input = InvestInput {
            stages: vec![monthly("10000")],
            ..base("0", "15", 120)
        };
        let res = simulate(&input).unwrap();
        assert_amt(res.final_nominal, "2752170.58");
        assert_amt(res.contributed, "1200000");
        assert_amt(res.invested, "1200000");
        let early = InvestInput {
            stages: vec![ContributionStage {
                timing: Timing::Start,
                ..monthly("10000")
            }],
            ..input
        };
        assert_amt(simulate(&early).unwrap().final_nominal, "2786572.72");
    }

    #[test]
    fn quarterly_capitalization() {
        let input = InvestInput {
            capitalization: CapFreq::Quarterly,
            ..base("100000", "8", 24)
        };
        let res = simulate(&input).unwrap();
        assert_amt(res.final_nominal, "117165.94");
        assert!(res.years[0].months[1].interest.is_zero());
        assert_amt(res.effective_rate_pct, "8.24");
    }

    #[test]
    fn daily_capitalization_counts_real_days() {
        let input = InvestInput {
            capitalization: CapFreq::Daily,
            ..base("100000", "12", 12)
        };
        assert_amt(simulate(&input).unwrap().final_nominal, "112747.46");
        // a leap year has one more day of interest
        let leap = InvestInput {
            start: "2024-01".into(),
            ..input
        };
        assert!(simulate(&leap).unwrap().final_nominal > amt("112747.46"));
    }

    #[test]
    fn at_end_is_simple_interest() {
        let input = InvestInput {
            capitalization: CapFreq::AtEnd,
            ..base("100000", "10", 24)
        };
        let res = simulate(&input).unwrap();
        assert_amt(res.final_nominal, "120000");
        assert!(res.years[0].interest.is_zero());
        // 20% over two years is 9.54% a year
        assert_amt(res.effective_rate_pct, "9.54");
    }

    #[test]
    fn zero_rate_and_short_terms() {
        let input = InvestInput {
            stages: vec![monthly("100")],
            ..base("1000", "0", 12)
        };
        let res = simulate(&input).unwrap();
        assert_amt(res.final_nominal, "2200");
        assert!(res.interest.is_zero());
        assert_eq!(res.crossover_year, None);

        let none = simulate(&base("1000", "10", 0)).unwrap();
        assert_amt(none.final_nominal, "1000");
        assert!(none.years.is_empty());

        let one = simulate(&base("1200", "10", 1)).unwrap();
        assert_amt(one.final_nominal, "1210");
    }

    #[test]
    fn tax_yearly_costs_more_than_at_end() {
        let tax = |mode| InvestInput {
            tax: TaxRule {
                pct: amt("13"),
                mode,
            },
            ..base("100000", "10", 36)
        };
        let yearly = simulate(&tax(TaxMode::Yearly)).unwrap();
        let at_end = simulate(&tax(TaxMode::AtEnd)).unwrap();
        let untaxed = simulate(&base("100000", "10", 36)).unwrap();
        assert!(yearly.final_nominal < at_end.final_nominal);
        assert!(at_end.final_nominal < untaxed.final_nominal);
        assert_amt(
            at_end.tax,
            &(untaxed.interest * amt("0.13")).round(2).to_string(),
        );
        assert!(yearly.years.iter().all(|year| year.tax.is_positive()));
    }

    #[test]
    fn stages_and_indexation() {
        let input = InvestInput {
            stages: vec![
                ContributionStage {
                    from_month: 13,
                    amount: amt("200"),
                    ..ContributionStage::default()
                },
                monthly("100"),
            ],
            ..base("0", "0", 24)
        };
        let res = simulate(&input).unwrap();
        assert_amt(res.years[0].contributed, "1200");
        assert_amt(res.years[1].contributed, "2400");

        let indexed = InvestInput {
            stages: vec![monthly("100")],
            indexation_pct: amt("10"),
            ..base("0", "0", 24)
        };
        let res = simulate(&indexed).unwrap();
        assert_amt(res.years[1].months[0].contributed, "110");
        assert_amt(res.contributed, "2520");

        let quarterly = InvestInput {
            stages: vec![ContributionStage {
                freq: Freq::Quarterly,
                ..monthly("300")
            }],
            ..base("0", "0", 12)
        };
        assert_amt(simulate(&quarterly).unwrap().contributed, "1200");
    }

    #[test]
    fn withdrawals_until_depleted() {
        let input = InvestInput {
            withdrawal: Some(Withdrawal {
                amount: amt("300"),
                from_month: 1,
                ..Withdrawal::default()
            }),
            ..base("1000", "0", 12)
        };
        let res = simulate(&input).unwrap();
        assert_eq!(res.depleted_at, Some(4));
        assert_amt(res.withdrawn, "1000");
        assert!(res.final_nominal.is_zero());

        let indexed = InvestInput {
            inflation_pct: amt("10"),
            withdrawal: Some(Withdrawal {
                amount: amt("100"),
                freq: Freq::Yearly,
                from_month: 1,
                index_to_inflation: true,
            }),
            ..base("10000", "0", 24)
        };
        let res = simulate(&indexed).unwrap();
        assert_amt(res.withdrawn, "210");
        assert_eq!(res.depleted_at, None);
    }

    #[test]
    fn contributions_go_on_after_the_withdrawals_ran_out() {
        // 100 + 10 a month against 30 a month: the pot is empty in month 4, the
        // withdrawals stop, and the contributions rebuild the balance to the term's end.
        let input = InvestInput {
            stages: vec![monthly("10")],
            withdrawal: Some(Withdrawal {
                amount: amt("30"),
                from_month: 1,
                ..Withdrawal::default()
            }),
            ..base("100", "0", 12)
        };
        let res = simulate(&input).unwrap();
        assert_eq!(res.depleted_at, Some(5));
        // Everything that was there is withdrawn once, nothing after.
        assert_amt(res.withdrawn, "150");
        let last = res
            .years
            .last()
            .and_then(|year| year.months.last())
            .unwrap();
        assert_eq!(last.month, 12);
        assert_amt(last.withdrawn, "0");
        // Seven more contributions after the empty month.
        assert_amt(res.final_nominal, "70");
    }

    #[test]
    fn safe_withdrawal_lasts_the_term() {
        let rule = |amount: &str| Withdrawal {
            amount: amt(amount),
            from_month: 1,
            ..Withdrawal::default()
        };
        let input = InvestInput {
            withdrawal: Some(rule("300")),
            ..base("1200", "0", 12)
        };
        let safe = safe_withdrawal(&input).unwrap().unwrap();
        // 100 × 12 empties the plan in its last month, which counts as running out.
        assert_amt(safe, "99");
        let fits = InvestInput {
            withdrawal: Some(rule("99")),
            ..input.clone()
        };
        assert_eq!(safe_withdrawal(&fits).unwrap(), None);
        let none = InvestInput {
            withdrawal: None,
            ..input
        };
        assert_eq!(safe_withdrawal(&none).unwrap(), None);
    }

    #[test]
    fn real_result_and_crossover() {
        let input = InvestInput {
            inflation_pct: amt("10"),
            ..base("110000", "0", 12)
        };
        assert_amt(simulate(&input).unwrap().final_real, "100000");

        let input = InvestInput {
            stages: vec![monthly("10000")],
            ..base("0", "15", 120)
        };
        let res = simulate(&input).unwrap();
        // year 6 earns ~123k on 120k contributed
        assert_eq!(res.crossover_year, Some(6));
        assert!(res.years[4].interest < res.years[4].contributed);
    }

    #[test]
    fn solve_round_trips() {
        let input = InvestInput {
            stages: vec![monthly("10000")],
            ..base("0", "15", 120)
        };
        let target = amt("3000000");

        let found = solve(&input, target, SolveUnknown::Contribution).unwrap();
        assert!(found.reachable);
        assert!(found.result.final_nominal >= target);
        assert!(found.value > amt("10000") && found.value < amt("11000"));
        let less = InvestInput {
            stages: vec![monthly(&(found.value - amt("0.05")).to_string())],
            ..input.clone()
        };
        assert!(simulate(&less).unwrap().final_nominal < target);

        let found = solve(&input, target, SolveUnknown::Term).unwrap();
        assert!(found.reachable);
        let months = found.input.term_months;
        assert!(found.result.final_nominal >= target);
        let shorter = InvestInput {
            term_months: months - 1,
            ..input.clone()
        };
        assert!(simulate(&shorter).unwrap().final_nominal < target);

        let found = solve(&input, target, SolveUnknown::Rate).unwrap();
        assert!(found.reachable);
        assert!(found.result.final_nominal >= target);
        assert!(found.value > amt("15") && found.value < amt("17"));
    }

    #[test]
    fn solve_with_empty_stages_and_at_end() {
        let input = base("1000", "10", 12);
        let found = solve(&input, amt("5000"), SolveUnknown::Contribution).unwrap();
        assert!(found.reachable);
        assert_eq!(found.input.stages.len(), 1);
        assert!(found.result.final_nominal >= amt("5000"));

        let at_end = InvestInput {
            capitalization: CapFreq::AtEnd,
            tax: TaxRule {
                pct: amt("13"),
                mode: TaxMode::AtEnd,
            },
            ..base("100000", "10", 12)
        };
        let found = solve(&at_end, amt("150000"), SolveUnknown::Term).unwrap();
        assert!(found.reachable);
        let shorter = InvestInput {
            term_months: found.input.term_months - 1,
            ..at_end
        };
        assert!(simulate(&shorter).unwrap().final_nominal < amt("150000"));
    }

    #[test]
    fn solve_unreachable() {
        let draining = InvestInput {
            withdrawal: Some(Withdrawal {
                amount: amt("500"),
                from_month: 1,
                ..Withdrawal::default()
            }),
            ..base("1000", "1", 12)
        };
        let term = solve(&draining, amt("2000"), SolveUnknown::Term).unwrap();
        assert!(!term.reachable);
        let rate = solve(&base("1000", "1", 1), amt("1000000"), SolveUnknown::Rate).unwrap();
        assert!(!rate.reachable);
        assert_amt(rate.value, "200");
        let already = solve(
            &base("1000", "1", 12),
            amt("500"),
            SolveUnknown::Contribution,
        )
        .unwrap();
        assert!(already.reachable);
        assert!(already.value.is_zero());
    }

    #[test]
    fn compare_rate_against_contribution() {
        let input = InvestInput {
            stages: vec![monthly("10000")],
            ..base("0", "15", 120)
        };
        let res = compare(&input).unwrap();
        assert_eq!(res.rows.len(), 3);
        assert!(res.rows[0].gain.is_zero());
        assert!(res.rows[1].gain.is_positive() && res.rows[2].gain.is_positive());
        assert_amt(
            res.rows[2].final_nominal,
            &(amt("2752170.58") * amt("1.2")).round(2).to_string(),
        );

        let lump = compare(&base("100000", "10", 120)).unwrap();
        assert!(lump.rows[2].gain.is_zero());
        assert!(!lump.contribution_wins);
    }

    #[test]
    fn zeros_and_bad_input_do_not_panic() {
        let zero = InvestInput {
            stages: vec![monthly("0")],
            ..base("0", "0", 1200)
        };
        assert!(simulate(&zero).unwrap().final_nominal.is_zero());
        assert!(solve(&zero, amt("100"), SolveUnknown::Term).is_ok());
        assert!(solve(&zero, amt("100"), SolveUnknown::Rate).is_ok());
        assert!(solve(&zero, amt("0"), SolveUnknown::Rate).is_err());

        let huge = base("1000000000", "1000", 1200);
        assert!(simulate(&huge).is_err());
        assert!(
            solve(
                &base("1", "0", 1200),
                amt("1000000000000"),
                SolveUnknown::Rate
            )
            .unwrap()
            .reachable
        );

        assert!(
            simulate(&InvestInput {
                start: "2025".into(),
                ..base("1", "1", 1)
            })
            .is_err()
        );
        assert!(simulate(&base("1", "1", MAX_TERM_MONTHS + 1)).is_err());
        assert!(simulate(&base("-1", "1", 1)).is_err());
        let stage_zero = InvestInput {
            stages: vec![ContributionStage::default()],
            ..base("1", "1", 1)
        };
        assert!(simulate(&stage_zero).is_err());
    }
}
