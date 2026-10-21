//! The capital checkup (plan C4): one light over the emergency fund, loans
//! and DTI, the savings rate and lifestyle creep, each colour with its
//! reasons and a first step.

use std::collections::BTreeMap;

use chrono::{Duration, NaiveDate};

use super::UnlockedVault;
use super::debts::{DebtLoad, LoanClass};
use super::emergency::EmergencyStatus;
use super::insights::{Insight, InsightCode};
use super::passive::PassiveIncome;
use crate::calendar::{self, Calendar};
use crate::model::notify::{NoticeCategory, NoticeRecord};
use crate::model::planning::{HealthColor, HealthReasonCode, HealthSnapshot};
use crate::service::notifications::{notice, notice_title};
use crate::{Amount, Result};

/// DTI above this (percent) is red.
pub const DTI_HIGH_PCT: u32 = 50;
/// DTI above this (percent) is yellow.
pub const DTI_ELEVATED_PCT: u32 = 30;
/// A fund under this many months of mandatory spending is red.
pub const FUND_MIN_MONTHS: u32 = 1;
/// A fund under this many months is yellow.
pub const FUND_SAFE_MONTHS: u32 = 3;
/// The savings-rate chart's length, finished months.
const SAVINGS_MONTHS: usize = 12;
/// Finished months the savings rate and creep are judged over.
const RECENT_MONTHS: usize = 3;
/// The savings rate the chart points to (PDF 2.1), percent.
pub const SAVINGS_BENCHMARK_PCT: u32 = 20;

/// One reason for the light, with what its first step needs.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct HealthReason {
    pub code: HealthReasonCode,
    /// The colour this reason alone would give; `Green` marks a hint.
    pub level: HealthColor,
    /// What the step is about: the debt, the gap to the fund level, the
    /// monthly shortfall, the spending growth.
    #[serde(default)]
    pub amount: Option<Amount>,
    /// The rate, the DTI, the months covered, the savings rate.
    #[serde(default)]
    pub pct: Option<Amount>,
    /// The loan's store id.
    #[serde(default)]
    pub subject: Option<String>,
}

/// The savings rate of one finished month.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct SavingsMonth {
    /// `YYYY-MM`.
    pub month: String,
    pub income: Amount,
    /// `income − fixed − daily`.
    pub saved: Amount,
    /// `saved / income`, percent; `None` without income.
    pub saved_pct: Option<Amount>,
}

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

/// What the light is judged from; built by [`UnlockedVault::capital_health`].
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

/// `count` finished month starts before the month of `today`, oldest first.
fn finished_months(today: NaiveDate, count: usize) -> Vec<NaiveDate> {
    let mut month = calendar::first_of_month(today);
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        month = calendar::first_of_month(month - Duration::days(1));
        out.push(month);
    }
    out.reverse();
    out
}

/// The same month `years` back.
fn years_back(month: NaiveDate, years: i32) -> NaiveDate {
    NaiveDate::from_ymd_opt(
        chrono::Datelike::year(&month) - years,
        chrono::Datelike::month(&month),
        1,
    )
    .unwrap_or(month)
}

fn percent(part: Amount, whole: Amount) -> Option<Amount> {
    (whole.is_positive()).then(|| (part / whole * Amount::from(100u32)).round(1))
}

impl UnlockedVault {
    /// Average income and spending (`fixed + daily`) of the given months;
    /// `None` when any of them had no flow at all.
    fn months_average(&self, months: &[NaiveDate], zone: &str) -> Option<(Amount, Amount)> {
        let mut income = Amount::ZERO;
        let mut spent = Amount::ZERO;
        for month in months {
            let summary = self
                .month_summary(&calendar::month_key(*month), zone)
                .ok()?;
            if summary.income.is_zero() && summary.fixed.is_zero() && summary.daily.is_zero() {
                return None;
            }
            income += summary.income;
            spent += summary.fixed + summary.daily;
        }
        let count = Amount::from(months.len() as u32);
        (!months.is_empty()).then(|| (income / count, spent / count))
    }

    /// Spending growing as fast as income or faster (decision 26): the last
    /// finished months against the same months a year back, or against the
    /// months just before when the year has no data.
    pub fn lifestyle_creep(&self, now: i64, zone: &str) -> Option<Insight> {
        let today = Calendar::new(zone).date_of(now);
        let recent = finished_months(today, RECENT_MONTHS);
        let (income, spent) = self.months_average(&recent, zone)?;
        let year_ago: Vec<NaiveDate> = recent.iter().map(|month| years_back(*month, 1)).collect();
        let (subject, (base_income, base_spent)) = match self.months_average(&year_ago, zone) {
            Some(base) => ("year", base),
            None => {
                let before = finished_months(recent[0], RECENT_MONTHS);
                ("previous", self.months_average(&before, zone)?)
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

    /// The savings rate of the last finished months, oldest first.
    pub fn savings_series(&self, now: i64, zone: &str) -> Result<Vec<SavingsMonth>> {
        let today = Calendar::new(zone).date_of(now);
        finished_months(today, SAVINGS_MONTHS)
            .into_iter()
            .map(|month| {
                let summary = self.month_summary(&calendar::month_key(month), zone)?;
                Ok(SavingsMonth {
                    saved_pct: percent(summary.savings_fact, summary.income),
                    month: summary.month,
                    income: summary.income,
                    saved: summary.savings_fact,
                })
            })
            .collect()
    }

    /// The checkup. `manual` is the hand-entered inflation table, as for the
    /// other inflation readers.
    pub fn capital_health(
        &self,
        now: i64,
        zone: &str,
        manual: &BTreeMap<String, Amount>,
    ) -> Result<HealthReport> {
        let emergency = self.emergency_fund_status(now, zone, manual);
        let debts = self.debt_load(now, zone, manual)?;
        let freedom_rate = self.doc.settings.freedom_rate;
        let passive = self.passive_income(now, zone, freedom_rate)?;
        let savings = self.savings_series(now, zone)?;
        let creep = self.lifestyle_creep(now, zone);

        let hundred = Amount::from(100u32);
        let savings_target_pct = (self.doc.settings.savings_rate * hundred).round(1);
        let recent: Vec<&SavingsMonth> = savings
            .iter()
            .rev()
            .filter(|month| month.income.is_positive())
            .take(RECENT_MONTHS)
            .collect();
        let income: Amount = recent
            .iter()
            .fold(Amount::ZERO, |sum, month| sum + month.income);
        let saved: Amount = recent
            .iter()
            .fold(Amount::ZERO, |sum, month| sum + month.saved);
        let savings_avg_pct = percent(saved, income);
        let savings_gap = (!recent.is_empty()).then(|| {
            let plan = income * self.doc.settings.savings_rate;
            ((plan - saved).max(Amount::ZERO) / Amount::from(recent.len() as u32)).round(2)
        });

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
            currency: self.budget_currency(),
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

    /// Set the yearly return the freedom potential assumes; `None` = the key rate.
    pub fn set_freedom_rate(&mut self, rate: Option<Amount>) -> Result<()> {
        if rate.is_some_and(|rate| rate.is_negative()) {
            return Err(crate::Error::InvalidInput(
                "expected return: must not be negative".into(),
            ));
        }
        self.doc.settings.freedom_rate = rate;
        self.persist()
    }

    /// The monthly checkup notice (decision 28): once a month, only when the
    /// colour or the reasons changed since the last check. The check itself
    /// is remembered either way.
    pub(crate) fn health_notice(
        &mut self,
        now: i64,
        zone: &str,
        ru: bool,
    ) -> Result<Option<NoticeRecord>> {
        let month = calendar::month_key(Calendar::new(zone).date_of(now));
        if self
            .doc
            .health_last
            .as_ref()
            .is_some_and(|last| last.month == month)
        {
            return Ok(None);
        }
        let report = self.capital_health(now, zone, &BTreeMap::new())?;
        let snapshot = HealthSnapshot {
            month: month.clone(),
            color: report.color,
            reasons: report.reasons.iter().map(|item| item.code).collect(),
        };
        let changed = match &self.doc.health_last {
            None => snapshot.color != HealthColor::NoData,
            Some(last) => last.color != snapshot.color || last.reasons != snapshot.reasons,
        };
        self.doc.health_last = Some(snapshot);
        self.persist()?;
        if !changed {
            return Ok(None);
        }
        let colour = color_text(report.color, ru);
        let body = match report.reasons.first() {
            Some(first)
                if first.level > HealthColor::Green || report.color == HealthColor::Green =>
            {
                if ru {
                    format!(
                        "Светофор капитала: {colour}. {}",
                        reason_text(first.code, ru)
                    )
                } else {
                    format!("Capital light: {colour}. {}", reason_text(first.code, ru))
                }
            }
            _ if ru => format!("Светофор капитала: {colour}."),
            _ => format!("Capital light: {colour}."),
        };
        Ok(Some(notice(
            NoticeCategory::Health,
            format!("health:{month}"),
            notice_title(NoticeCategory::Health, ru),
            body,
            Some("analytics-health".into()),
            calendar::iso(now),
        )))
    }
}

fn color_text(color: HealthColor, ru: bool) -> &'static str {
    match (color, ru) {
        (HealthColor::NoData, true) => "мало данных",
        (HealthColor::NoData, false) => "not enough data",
        (HealthColor::Green, true) => "зелёный",
        (HealthColor::Green, false) => "green",
        (HealthColor::Yellow, true) => "жёлтый",
        (HealthColor::Yellow, false) => "yellow",
        (HealthColor::Red, true) => "красный",
        (HealthColor::Red, false) => "red",
    }
}

fn reason_text(code: HealthReasonCode, ru: bool) -> &'static str {
    use HealthReasonCode as Code;
    match (code, ru) {
        (Code::NoHistory, true) => "Нет истории за полный месяц",
        (Code::NoHistory, false) => "No full month of history yet",
        (Code::CostlyLoan, true) => "Есть дорогой кредит",
        (Code::CostlyLoan, false) => "There is a costly loan",
        (Code::LoanWithoutRate, true) => "У кредита не указана ставка",
        (Code::LoanWithoutRate, false) => "A loan has no rate",
        (Code::DtiHigh, true) => "Платежи по кредитам больше половины дохода",
        (Code::DtiHigh, false) => "Loan payments take over half of income",
        (Code::DtiElevated, true) => "Платежи по кредитам больше 30% дохода",
        (Code::DtiElevated, false) => "Loan payments take over 30% of income",
        (Code::FundNotSet, true) => "Подушка безопасности не выбрана",
        (Code::FundNotSet, false) => "No emergency fund chosen",
        (Code::FundBelowOne, true) => "Подушки не хватит и на месяц",
        (Code::FundBelowOne, false) => "The emergency fund covers less than a month",
        (Code::FundBelowThree, true) => "Подушка меньше трёх месяцев",
        (Code::FundBelowThree, false) => "The emergency fund covers less than three months",
        (Code::SavingsBelowPlan, true) => "Откладываете меньше плана",
        (Code::SavingsBelowPlan, false) => "Saving less than planned",
        (Code::LifestyleCreep, true) => "Расходы растут не медленнее дохода",
        (Code::LifestyleCreep, false) => "Spending grows as fast as income",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::planning::EmergencyFund;
    use crate::model::store::{StoreKind, StoreScope, SubType};
    use crate::model::transaction::Transaction;
    use crate::pricing::history::secs_or_now;
    use crate::service::StoreExtras;
    use crate::service::test_support::*;

    fn pct(raw: &str) -> Amount {
        Amount::parse(raw).unwrap()
    }

    fn base_facts() -> HealthFacts {
        HealthFacts {
            has_history: true,
            fund_set: true,
            fund_total: pct("600"),
            e_mand: Some(pct("100")),
            months_covered: Some(pct("6")),
            savings_avg_pct: Some(pct("20")),
            savings_target_pct: pct("10"),
            ..HealthFacts::default()
        }
    }

    fn codes(facts: &HealthFacts) -> (HealthColor, Vec<HealthReasonCode>) {
        let (color, reasons) = judge(facts);
        (color, reasons.into_iter().map(|item| item.code).collect())
    }

    #[test]
    fn healthy_facts_are_green_without_reasons() {
        assert_eq!(codes(&base_facts()), (HealthColor::Green, vec![]));
    }

    #[test]
    fn dti_boundaries_are_strict() {
        let mut facts = base_facts();
        facts.dti_pct = Some(pct("30"));
        assert_eq!(codes(&facts).0, HealthColor::Green);
        facts.dti_pct = Some(pct("30.1"));
        assert_eq!(
            codes(&facts),
            (HealthColor::Yellow, vec![HealthReasonCode::DtiElevated])
        );
        facts.dti_pct = Some(pct("50"));
        assert_eq!(codes(&facts).0, HealthColor::Yellow);
        facts.dti_pct = Some(pct("50.1"));
        assert_eq!(
            codes(&facts),
            (HealthColor::Red, vec![HealthReasonCode::DtiHigh])
        );
    }

    #[test]
    fn fund_boundaries_and_gap() {
        let mut facts = base_facts();
        facts.months_covered = Some(pct("3"));
        facts.fund_total = pct("300");
        assert_eq!(codes(&facts).0, HealthColor::Green);
        facts.months_covered = Some(pct("2.5"));
        facts.fund_total = pct("250");
        let (color, reasons) = judge(&facts);
        assert_eq!(color, HealthColor::Yellow);
        assert_eq!(reasons[0].code, HealthReasonCode::FundBelowThree);
        assert_eq!(reasons[0].amount, Some(pct("50")));
        facts.months_covered = Some(pct("0.5"));
        facts.fund_total = pct("50");
        let (color, reasons) = judge(&facts);
        assert_eq!(color, HealthColor::Red);
        assert_eq!(reasons[0].code, HealthReasonCode::FundBelowOne);
        assert_eq!(reasons[0].amount, Some(pct("50")));
        facts.months_covered = Some(pct("1"));
        assert_eq!(codes(&facts).1, vec![HealthReasonCode::FundBelowThree]);
    }

    #[test]
    fn a_missing_fund_is_yellow_not_red() {
        let mut facts = base_facts();
        facts.fund_set = false;
        facts.months_covered = None;
        assert_eq!(
            codes(&facts),
            (HealthColor::Yellow, vec![HealthReasonCode::FundNotSet])
        );
    }

    #[test]
    fn no_history_is_no_data_but_a_costly_card_is_still_red() {
        let facts = HealthFacts::default();
        assert_eq!(
            codes(&facts),
            (HealthColor::NoData, vec![HealthReasonCode::NoHistory])
        );
        let facts = HealthFacts {
            loans: vec![("card".into(), LoanClass::Costly, pct("1000"), None)],
            ..HealthFacts::default()
        };
        let (color, reasons) = judge(&facts);
        assert_eq!(color, HealthColor::Red);
        assert_eq!(reasons[0].subject.as_deref(), Some("card"));
        assert_eq!(reasons[1].code, HealthReasonCode::NoHistory);
    }

    #[test]
    fn hints_do_not_change_the_colour() {
        let mut facts = base_facts();
        facts.loans = vec![("loan".into(), LoanClass::Unknown, pct("10"), None)];
        facts.creep = Some(Insight {
            code: InsightCode::LifestyleCreep,
            amount: Some(pct("50")),
            pct: Some(pct("100")),
            subject: Some("year".into()),
        });
        assert_eq!(
            codes(&facts),
            (
                HealthColor::Green,
                vec![
                    HealthReasonCode::LoanWithoutRate,
                    HealthReasonCode::LifestyleCreep
                ]
            )
        );
    }

    #[test]
    fn savings_below_plan_is_yellow_and_sorted_after_red() {
        let mut facts = base_facts();
        facts.savings_avg_pct = Some(pct("5"));
        facts.savings_gap = Some(pct("25"));
        facts.dti_pct = Some(pct("60"));
        assert_eq!(
            codes(&facts),
            (
                HealthColor::Red,
                vec![
                    HealthReasonCode::DtiHigh,
                    HealthReasonCode::SavingsBelowPlan
                ]
            )
        );
    }

    fn dated(mut tx: Transaction, ts: &str) -> Transaction {
        at(&mut tx, ts);
        tx
    }

    /// Income and spending for each `(month, income, spent)`.
    fn months(vault: &mut UnlockedVault, store: &str, rows: &[(&str, &str, &str)]) {
        for (month, earned, spent) in rows {
            vault
                .add_transaction(dated(
                    income(store, earned, "pay"),
                    &format!("{month}-05T10:00:00Z"),
                ))
                .unwrap();
            vault
                .add_transaction(dated(
                    payment(store, spent, "food"),
                    &format!("{month}-10T10:00:00Z"),
                ))
                .unwrap();
        }
    }

    #[test]
    fn creep_compares_with_the_same_months_a_year_back() {
        let (_dir, mut vault) = empty_vault();
        quote_usd(&mut vault);
        vault.set_base_currency("usd");
        let cash = store(&mut vault, "Cash", StoreKind::Cash, None);
        months(
            &mut vault,
            &cash,
            &[
                ("2025-03", "1000", "800"),
                ("2025-04", "1000", "800"),
                ("2025-05", "1000", "800"),
                ("2026-03", "1200", "1000"),
                ("2026-04", "1200", "1000"),
                ("2026-05", "1200", "1000"),
            ],
        );
        let now = secs_or_now("2026-06-15T10:00:00Z");
        let creep = vault.lifestyle_creep(now, "UTC").expect("creep");
        assert_eq!(creep.subject.as_deref(), Some("year"));
        assert_eq!(creep.amount, Some(pct("200")));
        assert_eq!(creep.pct, Some(pct("100")));
    }

    #[test]
    fn creep_without_a_year_takes_the_months_before() {
        let (_dir, mut vault) = empty_vault();
        quote_usd(&mut vault);
        vault.set_base_currency("usd");
        let cash = store(&mut vault, "Cash", StoreKind::Cash, None);
        months(
            &mut vault,
            &cash,
            &[
                ("2026-01", "1000", "500"),
                ("2026-02", "1000", "500"),
                ("2026-03", "1000", "500"),
                ("2026-04", "1100", "550"),
                ("2026-05", "1100", "550"),
                ("2026-06", "1100", "550"),
            ],
        );
        let now = secs_or_now("2026-07-02T10:00:00Z");
        assert_eq!(vault.lifestyle_creep(now, "UTC"), None, "spent grew slower");
        for month in ["2026-04", "2026-05", "2026-06"] {
            vault
                .add_transaction(dated(
                    payment(&cash, "100", "extra"),
                    &format!("{month}-20T10:00:00Z"),
                ))
                .unwrap();
        }
        let creep = vault.lifestyle_creep(now, "UTC").expect("creep");
        assert_eq!(creep.subject.as_deref(), Some("previous"));
        assert_eq!(creep.amount, Some(pct("150")));
    }

    #[test]
    fn the_checkup_reads_the_vault_and_notices_only_changes() {
        let (_dir, mut vault) = empty_vault();
        quote_usd(&mut vault);
        vault.set_base_currency("usd");
        let cash = store(&mut vault, "Cash", StoreKind::Cash, None);
        let start = secs_or_now("2026-04-02T10:00:00Z");
        let report = vault
            .capital_health(start, "UTC", &BTreeMap::new())
            .unwrap();
        assert_eq!(report.color, HealthColor::NoData);
        assert_eq!(vault.health_notice(start, "UTC", true).unwrap(), None);
        assert_eq!(
            vault.doc.health_last.as_ref().map(|last| last.color),
            Some(HealthColor::NoData)
        );

        months(
            &mut vault,
            &cash,
            &[
                ("2026-02", "1000", "950"),
                ("2026-03", "1000", "950"),
                ("2026-04", "1000", "950"),
            ],
        );
        let may = secs_or_now("2026-05-03T10:00:00Z");
        let report = vault.capital_health(may, "UTC", &BTreeMap::new()).unwrap();
        assert_eq!(report.savings.len(), 12);
        assert_eq!(report.savings_avg_pct, Some(pct("5")));
        assert_eq!(report.color, HealthColor::Yellow);
        let found: Vec<HealthReasonCode> = report.reasons.iter().map(|item| item.code).collect();
        assert_eq!(
            found,
            vec![
                HealthReasonCode::FundNotSet,
                HealthReasonCode::SavingsBelowPlan
            ]
        );
        let sent = vault
            .health_notice(may, "UTC", true)
            .unwrap()
            .expect("sent");
        assert_eq!(sent.key, "health:2026-05");
        assert_eq!(sent.link.as_deref(), Some("analytics-health"));
        assert!(sent.body.contains("жёлтый"), "{}", sent.body);
        assert_eq!(vault.health_notice(may, "UTC", true).unwrap(), None);

        let june = secs_or_now("2026-06-03T10:00:00Z");
        months(&mut vault, &cash, &[("2026-05", "1000", "950")]);
        assert_eq!(vault.health_notice(june, "UTC", true).unwrap(), None);

        let bank = store(&mut vault, "Bank", StoreKind::Bank, None);
        let extras = StoreExtras {
            sub_type: Some(SubType::Savings),
            binding: Some(crate::model::store::Binding::Asset(usd())),
            ..StoreExtras::default()
        };
        let savings = vault
            .create_store_with("Reserve", StoreKind::Bank, "#000", "", Some(bank), extras)
            .unwrap()
            .id;
        vault
            .set_emergency_fund(EmergencyFund {
                scope: StoreScope {
                    store_ids: vec![savings],
                    include_children: false,
                },
                ..EmergencyFund::default()
            })
            .unwrap();
        let july = secs_or_now("2026-07-03T10:00:00Z");
        let sent = vault
            .health_notice(july, "UTC", false)
            .unwrap()
            .expect("sent");
        assert!(sent.body.contains("red"), "{}", sent.body);
    }
}
