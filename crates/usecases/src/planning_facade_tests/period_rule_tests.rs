//! op-period T19: the period a rule gives a new operation (D3, D4), the spread shares kept on
//! save (P6, P8) and acceptance A1 (the salary «for» the second half of October).

use crate::Amount;
use crate::model::category::{Category, CategoryGroup};
use crate::model::store::StoreKind;
use crate::model::transaction::Transaction;
use crate::planning::daily_budget::daily_budget;
use crate::planning::flows::Basis;
use crate::service::Session;
use crate::service::test_support::*;
use crate::usecases::money::transactions::{
    CreationRules, add_transaction, add_transaction_ruled, update_transaction,
};
use mwt_model::{MonthShare, OpPeriod, PeriodKind, PeriodRule, Spread, SpreadMode};

const SALARY_DAY: &str = "2026-11-05T10:00:00Z";

fn vault_with_card() -> (tempfile::TempDir, Session, String) {
    let (dir, mut vault) = empty_vault();
    vault.set_base_currency("usd");
    quote_usd(&mut vault);
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    (dir, vault, card)
}

fn rule_on_salary(vault: &mut Session, rule: Option<PeriodRule>) {
    vault
        .doc
        .categories
        .retain(|category| category.key != "salary");
    let mut category = Category::builtin("salary", CategoryGroup::Income);
    category.period_rule = rule;
    vault.doc.categories.push(category);
}

fn salary(card: &str, raw: &str) -> Transaction {
    let mut tx = income(card, raw, "salary");
    at(&mut tx, SALARY_DAY);
    tx
}

fn period_of(tx: &Transaction) -> Option<(PeriodKind, String, String)> {
    tx.meta()
        .period
        .as_ref()
        .map(|period| (period.kind, period.start.clone(), period.end.clone()))
}

fn october_second_half() -> Option<(PeriodKind, String, String)> {
    Some((PeriodKind::Half, "2026-10-16".into(), "2026-10-31".into()))
}

fn quarter() -> OpPeriod {
    OpPeriod {
        kind: PeriodKind::Quarter,
        start: "2026-10-01".into(),
        end: "2026-12-31".into(),
    }
}

fn spread_of(tx: &Transaction) -> Option<(SpreadMode, Vec<String>)> {
    tx.meta().spread.as_ref().map(|spread| {
        let shares = spread
            .shares
            .iter()
            .map(|share| format!("{}={}", share.month, share.amount))
            .collect();
        (spread.mode, shares)
    })
}

fn with_spread(
    mut tx: Transaction,
    period: OpPeriod,
    mode: SpreadMode,
    shares: &[(&str, &str)],
) -> Transaction {
    tx.meta_mut().period = Some(period);
    tx.meta_mut().spread = Some(Spread {
        mode,
        shares: shares
            .iter()
            .map(|(month, raw)| MonthShare {
                month: (*month).into(),
                amount: amount(raw),
            })
            .collect(),
    });
    tx
}

fn set_amount(tx: &mut Transaction, raw: &str) {
    if let Transaction::Topup(inner) = tx {
        inner.amount = amount(raw);
    }
}

/// D3: the rule of the main category gives the period; the template's rule beats it.
#[test]
fn d3_period_comes_from_the_template_then_the_category_rule() {
    let (_dir, mut vault, card) = vault_with_card();
    rule_on_salary(&mut vault, Some(PeriodRule::PrevHalfSecond));
    let by_category = add_transaction(&mut vault, salary(&card, "100")).unwrap();
    assert_eq!(period_of(&by_category), october_second_half());
    let rules = CreationRules {
        template_rule: Some(PeriodRule::PrevMonth),
        no_period_rule: false,
    };
    let by_template =
        add_transaction_ruled(&mut vault, salary(&card, "100"), None, &rules).unwrap();
    assert_eq!(
        period_of(&by_template),
        Some((PeriodKind::Month, "2026-10-01".into(), "2026-10-31".into()))
    );
}

/// D3: an explicit period wins; no period with `no_period_rule` means none; no rule means none.
#[test]
fn d3_explicit_period_and_no_period_rule_beat_the_rules() {
    let (_dir, mut vault, card) = vault_with_card();
    rule_on_salary(&mut vault, Some(PeriodRule::PrevHalfSecond));
    let mut explicit = salary(&card, "100");
    explicit.meta_mut().period = Some(OpPeriod {
        kind: PeriodKind::Month,
        start: "2026-09-01".into(),
        end: "2026-09-30".into(),
    });
    let saved = add_transaction(&mut vault, explicit).unwrap();
    assert_eq!(
        period_of(&saved),
        Some((PeriodKind::Month, "2026-09-01".into(), "2026-09-30".into()))
    );
    let none = CreationRules {
        template_rule: Some(PeriodRule::PrevMonth),
        no_period_rule: true,
    };
    let bare = add_transaction_ruled(&mut vault, salary(&card, "100"), None, &none).unwrap();
    assert_eq!(period_of(&bare), None);
    rule_on_salary(&mut vault, None);
    assert_eq!(
        period_of(&add_transaction(&mut vault, salary(&card, "100")).unwrap()),
        None
    );
}

/// D4: the rule is stored on the operation; changing it never rewrites what exists.
#[test]
fn d4_changing_a_rule_does_not_rewrite_existing_operations() {
    let (_dir, mut vault, card) = vault_with_card();
    rule_on_salary(&mut vault, Some(PeriodRule::PrevHalfSecond));
    let first = add_transaction(&mut vault, salary(&card, "100")).unwrap();
    rule_on_salary(&mut vault, Some(PeriodRule::NextMonth));
    let stored = vault
        .doc
        .transactions
        .iter()
        .find(|tx| tx.meta().id == first.meta().id)
        .unwrap();
    assert_eq!(period_of(stored), october_second_half());
    let second = add_transaction(&mut vault, salary(&card, "100")).unwrap();
    assert_eq!(
        period_of(&second),
        Some((PeriodKind::Month, "2026-12-01".into(), "2026-12-31".into()))
    );
}

/// P6: `even` shares are derived on save and stored.
#[test]
fn p6_even_shares_are_derived_on_save() {
    let (_dir, mut vault, card) = vault_with_card();
    let tx = with_spread(salary(&card, "100.00"), quarter(), SpreadMode::Even, &[]);
    let saved = add_transaction(&mut vault, tx).unwrap();
    let (mode, shares) = spread_of(&saved).unwrap();
    assert_eq!(mode, SpreadMode::Even);
    assert_eq!(shares, ["2026-10=33.33", "2026-11=33.33", "2026-12=33.34"]);
}

/// P6: `by_days` shares follow the days in each month and add up to the amount.
#[test]
fn p6_by_days_shares_are_derived_on_save() {
    let (_dir, mut vault, card) = vault_with_card();
    let period = OpPeriod {
        kind: PeriodKind::Range,
        start: "2026-10-16".into(),
        end: "2026-12-15".into(),
    };
    let saved = add_transaction(
        &mut vault,
        with_spread(salary(&card, "6100"), period, SpreadMode::ByDays, &[]),
    )
    .unwrap();
    let spread = saved.meta().spread.as_ref().unwrap();
    assert_eq!(spread.shares.len(), 3);
    let total = spread
        .shares
        .iter()
        .fold(Amount::ZERO, |sum, share| sum + share.amount);
    assert_eq!(total, amount("6100"));
    assert_eq!(spread.shares[0].amount, amount("1600"));
}

fn manual_income(vault: &mut Session, card: &str) -> Transaction {
    let tx = with_spread(
        salary(card, "90"),
        quarter(),
        SpreadMode::Manual,
        &[("2026-10", "10"), ("2026-11", "20"), ("2026-12", "60")],
    );
    add_transaction(vault, tx).unwrap()
}

/// P8: a new amount on a manual spread clears the shares and the save is refused.
#[test]
fn p8_amount_edit_of_a_manual_spread_is_refused_until_new_shares() {
    let (_dir, mut vault, card) = vault_with_card();
    let saved = manual_income(&mut vault, &card);
    let id = saved.meta().id.clone();
    let mut edited = saved.clone();
    set_amount(&mut edited, "120");
    let refused = update_transaction(&mut vault, &id, edited.clone());
    assert!(matches!(refused, Err(crate::error::Error::InvalidInput(_))));
    edited.meta_mut().spread = Some(Spread {
        mode: SpreadMode::Manual,
        shares: ["2026-10", "2026-11", "2026-12"]
            .iter()
            .zip(["20", "40", "60"])
            .map(|(month, raw)| MonthShare {
                month: (*month).into(),
                amount: amount(raw),
            })
            .collect(),
    });
    let kept = update_transaction(&mut vault, &id, edited).unwrap();
    assert_eq!(spread_of(&kept).unwrap().1.len(), 3);
}

/// P8: a new period on a manual spread is refused too; an untouched manual spread saves.
#[test]
fn p8_period_edit_of_a_manual_spread_is_refused_and_a_plain_edit_keeps_it() {
    let (_dir, mut vault, card) = vault_with_card();
    let saved = manual_income(&mut vault, &card);
    let id = saved.meta().id.clone();
    let mut moved = saved.clone();
    moved.meta_mut().period = Some(OpPeriod {
        kind: PeriodKind::Range,
        start: "2026-10-01".into(),
        end: "2026-11-30".into(),
    });
    assert!(update_transaction(&mut vault, &id, moved).is_err());
    let mut commented = saved.clone();
    commented.meta_mut().comment = "bonus".into();
    let kept = update_transaction(&mut vault, &id, commented).unwrap();
    assert_eq!(spread_of(&kept), spread_of(&saved));
}

/// P8: editing an `even` spread re-derives it; a single-month period drops it.
#[test]
fn p8_even_is_rederived_and_a_single_month_period_drops_the_spread() {
    let (_dir, mut vault, card) = vault_with_card();
    let tx = with_spread(salary(&card, "100.00"), quarter(), SpreadMode::Even, &[]);
    let saved = add_transaction(&mut vault, tx).unwrap();
    let id = saved.meta().id.clone();
    let mut edited = saved.clone();
    set_amount(&mut edited, "90");
    let rederived = update_transaction(&mut vault, &id, edited).unwrap();
    assert_eq!(
        spread_of(&rederived).unwrap().1,
        ["2026-10=30", "2026-11=30", "2026-12=30"]
    );
    let mut one_month = rederived.clone();
    one_month.meta_mut().period = Some(OpPeriod {
        kind: PeriodKind::Month,
        start: "2026-11-01".into(),
        end: "2026-11-30".into(),
    });
    let dropped = update_transaction(&mut vault, &id, one_month).unwrap();
    assert_eq!(spread_of(&dropped), None);
}

fn income_in(vault: &Session, month: &str, basis: Basis) -> Amount {
    crate::usecases::planning::budget::month_summary_with_basis(
        &vault.books(),
        month,
        "UTC",
        Some(basis),
    )
    .unwrap()
    .income
}

/// A1: the salary of 5 Nov with `prev_half_second` is «for» 16-31 Oct; the readings differ,
/// the balance and the daily budget do not.
#[test]
fn a1_salary_for_the_second_half_of_october() {
    let (_dir, mut vault, card) = vault_with_card();
    rule_on_salary(&mut vault, Some(PeriodRule::PrevHalfSecond));
    let saved = add_transaction(&mut vault, salary(&card, "120000")).unwrap();
    assert_eq!(period_of(&saved), october_second_half());
    assert_eq!(
        income_in(&vault, "2026-10", Basis::Period),
        amount("120000")
    );
    assert_eq!(income_in(&vault, "2026-11", Basis::Period), amount("0"));
    assert_eq!(income_in(&vault, "2026-10", Basis::MoneyDate), amount("0"));
    assert_eq!(
        income_in(&vault, "2026-11", Basis::MoneyDate),
        amount("120000")
    );
    assert_eq!(holding_of(&vault, &card, &usd()), amount("120000"));

    let (_other_dir, mut plain, plain_card) = vault_with_card();
    rule_on_salary(&mut plain, None);
    add_transaction(&mut plain, salary(&plain_card, "120000")).unwrap();
    let day = secs(SALARY_DAY);
    assert_eq!(
        daily_budget(&vault.books(), day, "UTC"),
        daily_budget(&plain.books(), day, "UTC")
    );
}
