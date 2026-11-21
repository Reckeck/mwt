//! The capital checkup (plan C4): the light judged from the facts the other
//! areas give, and the report. The figures and texts are in `planning::health`;
//! the writes (the freedom rate, the monthly notice) in
//! `usecases::planning::health`.
//!
//! The light and the report sit in `reports` because they name the insight
//! types, which live there.

use chrono::NaiveDate;

use crate::Amount;
use crate::calendar::Calendar;
use crate::error::Result;
use crate::household::debts::{DebtLoad, LoanClass};
use crate::model::planning::{HealthColor, HealthReasonCode};
use crate::money::books::Books;
use crate::planning::budget::budget_currency;
use crate::planning::debt_load::debt_load;
use crate::planning::emergency::{EmergencyStatus, emergency_fund_status};
use crate::planning::health::{
    DTI_ELEVATED_PCT, DTI_HIGH_PCT, FUND_MIN_MONTHS, FUND_SAFE_MONTHS, HealthReason, RECENT_MONTHS,
    SavingsMonth, finished_months, months_average, percent, savings_figures, savings_series,
    years_back,
};
use crate::planning::passive::{PassiveIncome, passive_income};
use crate::reports::insights::{Insight, InsightCode};

/// The whole checkup (plan C4).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct HealthReport {
    pub color: HealthColor,
    /// Worst first.
    pub reasons: Vec<HealthReason>,
    /// The budget currency every amount here is in.
    pub currency: String,
    pub emergency: EmergencyStatus,
    pub debts: DebtLoad,
    pub passive: PassiveIncome,
    /// The last finished months, oldest first.
    pub savings: Vec<SavingsMonth>,
    /// `savings_rate` as a percent.
    pub savings_target_pct: Amount,
    /// Saved against income over the last months with income, percent.
    pub savings_avg_pct: Option<Amount>,
    pub creep: Option<Insight>,
    /// The yearly return the freedom potential assumes when set by hand.
    pub freedom_rate: Option<Amount>,
}

/// What the light is judged from; built by [`capital_health`].
#[derive(Debug, Clone, Default)]
pub(crate) struct HealthFacts {
    /// A finished month of history or a hand-entered month exists.
    pub has_history: bool,
    /// `(store id, class, owed, rate)` of each loan.
    pub loans: Vec<(String, LoanClass, Amount, Option<Amount>)>,
    pub dti_pct: Option<Amount>,
    pub fund_set: bool,
    pub fund_total: Amount,
    pub e_mand: Option<Amount>,
    pub months_covered: Option<Amount>,
    pub savings_avg_pct: Option<Amount>,
    pub savings_target_pct: Amount,
    /// Average monthly shortfall against the plan over the same months.
    pub savings_gap: Option<Amount>,
    pub creep: Option<Insight>,
}

fn reason(code: HealthReasonCode, level: HealthColor) -> HealthReason {
    HealthReason {
        code,
        level,
        amount: None,
        pct: None,
        subject: None,
    }
}

/// The light and its reasons, worst first.
pub(crate) fn judge(facts: &HealthFacts) -> (HealthColor, Vec<HealthReason>) {
    use HealthColor::{Green, NoData, Red, Yellow};
    use HealthReasonCode as Code;
    let mut out = Vec::new();
    for (store_id, class, owed, rate) in &facts.loans {
        let (code, level) = match class {
            LoanClass::Costly => (Code::CostlyLoan, Red),
            LoanClass::Unknown => (Code::LoanWithoutRate, Green),
            LoanClass::Normal | LoanClass::Cheap => continue,
        };
        out.push(HealthReason {
            amount: Some(*owed),
            pct: *rate,
            subject: Some(store_id.clone()),
            ..reason(code, level)
        });
    }
    if !facts.has_history {
        out.push(reason(Code::NoHistory, NoData));
    } else {
        if let Some(dti) = facts.dti_pct {
            let level = if dti > Amount::from(DTI_HIGH_PCT) {
                Some((Code::DtiHigh, Red))
            } else if dti > Amount::from(DTI_ELEVATED_PCT) {
                Some((Code::DtiElevated, Yellow))
            } else {
                None
            };
            if let Some((code, level)) = level {
                out.push(HealthReason {
                    pct: Some(dti),
                    ..reason(code, level)
                });
            }
        }
        if !facts.fund_set {
            out.push(reason(Code::FundNotSet, Yellow));
        } else if let (Some(months), Some(e_mand)) = (facts.months_covered, facts.e_mand) {
            let level = if months < Amount::from(FUND_MIN_MONTHS) {
                Some((Code::FundBelowOne, Red, FUND_MIN_MONTHS))
            } else if months < Amount::from(FUND_SAFE_MONTHS) {
                Some((Code::FundBelowThree, Yellow, FUND_SAFE_MONTHS))
            } else {
                None
            };
            if let Some((code, level, goal)) = level {
                let gap = (e_mand * Amount::from(goal) - facts.fund_total).max(Amount::ZERO);
                out.push(HealthReason {
                    amount: Some(gap.round(2)),
                    pct: Some(months),
                    ..reason(code, level)
                });
            }
        }
        if let Some(avg) = facts.savings_avg_pct
            && avg < facts.savings_target_pct
        {
            out.push(HealthReason {
                amount: facts.savings_gap,
                pct: Some(avg),
                ..reason(Code::SavingsBelowPlan, Yellow)
            });
        }
        if let Some(creep) = &facts.creep {
            out.push(HealthReason {
                amount: creep.amount,
                pct: creep.pct,
                subject: creep.subject.clone(),
                ..reason(Code::LifestyleCreep, Green)
            });
        }
    }
    out.sort_by_key(|item| std::cmp::Reverse(item.level));
    let worst = out.first().map_or(Green, |first| first.level);
    let no_data = out.iter().any(|item| item.code == Code::NoHistory);
    let color = if worst <= Green && no_data {
        NoData
    } else {
        worst.max(Green)
    };
    (color, out)
}

/// Spending growing as fast as income or faster (decision 26): the last
/// finished months against the same months a year back, or against the
/// months just before when the year has no data.
pub fn lifestyle_creep(books: &Books<'_>, now: i64, zone: &str) -> Option<Insight> {
    let today = Calendar::new(zone).date_of(now);
    let recent = finished_months(today, RECENT_MONTHS);
    let (income, spent) = months_average(books, &recent, zone)?;
    let year_ago: Vec<NaiveDate> = recent.iter().map(|month| years_back(*month, 1)).collect();
    let (subject, (base_income, base_spent)) = match months_average(books, &year_ago, zone) {
        Some(base) => ("year", base),
        None => {
            let before = finished_months(recent[0], RECENT_MONTHS);
            ("previous", months_average(books, &before, zone)?)
        }
    };
    let income_up = income - base_income;
    let spent_up = spent - base_spent;
    if !income_up.is_positive() || spent_up < income_up {
        return None;
    }
    Some(Insight {
        code: InsightCode::LifestyleCreep,
        amount: Some(spent_up.round(2)),
        pct: percent(spent_up, income_up),
        subject: Some(subject.into()),
    })
}

/// The checkup. `manual` is the hand-entered inflation table, as for the
/// other inflation readers.
pub fn capital_health(books: &Books<'_>, now: i64, zone: &str) -> Result<HealthReport> {
    let doc = books.doc();
    let emergency = emergency_fund_status(books, now, zone);
    let debts = debt_load(books, now, zone)?;
    let freedom_rate = doc.settings.freedom_rate;
    let passive = passive_income(books, now, zone, freedom_rate)?;
    let savings = savings_series(books, now, zone)?;
    let creep = lifestyle_creep(books, now, zone);

    let savings_target_pct = (doc.settings.savings_rate * Amount::from(100u32)).round(1);
    let (savings_avg_pct, savings_gap) = savings_figures(&savings, doc.settings.savings_rate);

    let facts = HealthFacts {
        has_history: emergency.e_mand.is_some() || debts.income.is_some(),
        loans: debts
            .loans
            .iter()
            .map(|loan| (loan.store_id.clone(), loan.class, loan.owed, loan.rate))
            .collect(),
        dti_pct: debts.dti_pct,
        fund_set: !emergency.fund.scope.store_ids.is_empty(),
        fund_total: emergency.total,
        e_mand: emergency.e_mand,
        months_covered: emergency.months_covered,
        savings_avg_pct,
        savings_target_pct,
        savings_gap,
        creep: creep.clone(),
    };
    let (color, reasons) = judge(&facts);
    Ok(HealthReport {
        color,
        reasons,
        currency: budget_currency(books),
        emergency,
        debts,
        passive,
        savings,
        savings_target_pct,
        savings_avg_pct,
        creep,
        freedom_rate,
    })
}
