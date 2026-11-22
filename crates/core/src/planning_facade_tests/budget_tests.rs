//! Tests of the budget area: the old inline tests (names and bodies unchanged).

use crate::Amount;
use crate::model::planning::Schedule;
use crate::model::planning::TransactionDraft;
use crate::model::planning::{BudgetPeriod, BudgetTarget};
use crate::model::store::StoreKind;
use crate::model::store::{AssetRef, LiabilityKind, LiabilityTerms};
use crate::model::transaction::Transaction;
use crate::model::transaction::{AccountRef, PriceStamp};
use crate::model::vault::DailyBudgetMode;
use crate::model::vault::FxPoint;
use crate::planning::budget::*;
use crate::planning::daily_budget::daily_budget;
use crate::service::AutomationDraft;
use crate::service::StoreExtras;
use crate::service::UnlockedVault;
use crate::service::test_support::*;
use crate::time::unix_secs;

fn secs(iso: &str) -> i64 {
    unix_secs(iso).unwrap()
}

fn dated(mut tx: Transaction, ts: &str) -> Transaction {
    at(&mut tx, ts);
    tx
}

fn spend(store: &str, sum: &str, ts: &str) -> Transaction {
    dated(payment(store, sum, "food"), ts)
}

/// USD between two stores of the user's.
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

fn earn(store: &str, sum: &str, ts: &str) -> Transaction {
    dated(income(store, sum, "salary"), ts)
}

/// A bank card in the budget, USD everywhere.
fn budget_vault() -> (tempfile::TempDir, UnlockedVault, String) {
    let (dir, mut vault) = empty_vault();
    vault.set_base_currency("usd");
    quote_usd(&mut vault);
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    (dir, vault, card)
}

#[test]
fn month_summary_counts_income_fixed_and_daily_but_not_internal_or_debts() {
    let (_dir, mut vault, card) = budget_vault();
    let pocket = store(&mut vault, "Pocket", StoreKind::Bank, Some(&card));
    vault
        .add_transaction(earn(&card, "1000", "2026-03-01T10:00:00Z"))
        .unwrap();
    let mut rent = spend(&card, "300", "2026-03-02T10:00:00Z");
    rent.meta_mut().categories = vec!["rent_mortgage".into()];
    vault.add_transaction(rent).unwrap();
    vault
        .add_transaction(spend(&card, "200", "2026-03-03T10:00:00Z"))
        .unwrap();
    // Internal and debt moves are neither income nor spending.
    let inner = usd_move(&card, &pocket, "100", "2026-03-04T10:00:00Z");
    vault.add_transaction(inner).unwrap();
    let friend = vault
        .create_contact(crate::service::ContactDraft {
            group: String::new(),
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
        "50",
    );
    if let Transaction::Transfer(tx) = &mut lend {
        tx.debt = Some(crate::model::transaction::DebtEffect::Lend);
    }
    at(&mut lend, "2026-03-05T10:00:00Z");
    vault.add_transaction(lend).unwrap();
    // Another month does not count.
    vault
        .add_transaction(spend(&card, "999", "2026-04-01T10:00:00Z"))
        .unwrap();

    let summary = vault.month_summary("2026-03", "UTC").unwrap();
    assert_eq!(summary.income, amount("1000"));
    assert_eq!(summary.fixed, amount("300"));
    assert_eq!(summary.daily, amount("200"));
    assert_eq!(summary.savings_plan, amount("100"));
    assert_eq!(summary.savings_fact, amount("500"));
    assert_eq!(summary.deviation, amount("400"));
    assert_eq!(summary.saved_pct, amount("50"));
}

#[test]
fn avg_month_takes_finished_months_with_flows_only() {
    let (_dir, mut vault, card) = budget_vault();
    let now = secs("2026-05-15T10:00:00Z");
    assert_eq!(vault.avg_month(3, now, "UTC"), None);
    for (sum, ts) in [
        ("900", "2026-01-10T10:00:00Z"),
        ("1000", "2026-02-10T10:00:00Z"),
        ("1200", "2026-04-10T10:00:00Z"),
        // The current month is not finished yet.
        ("5000", "2026-05-01T10:00:00Z"),
    ] {
        vault.add_transaction(earn(&card, sum, ts)).unwrap();
    }
    vault
        .add_transaction(spend(&card, "300", "2026-04-11T10:00:00Z"))
        .unwrap();

    // March had no flows and is skipped: April, February.
    let two = vault.avg_month(2, now, "UTC").unwrap();
    assert_eq!(two.months, 2);
    assert_eq!(two.income, amount("1100"));
    assert_eq!(two.daily, amount("150"));
    assert_eq!(two.currency, "usd");
    // Fewer months than asked: what there is.
    let all = vault.avg_month(12, now, "UTC").unwrap();
    assert_eq!(all.months, 3);
    assert_eq!(all.income, amount("1033.3333333333333333333333333"));
}

#[test]
fn a_loan_payment_is_mandatory_and_exchange_to_card_tops_up_the_budget() {
    let (_dir, mut vault, card) = budget_vault();
    let loan = vault
        .create_store_with(
            "Mortgage",
            StoreKind::Liability,
            "",
            "",
            None,
            StoreExtras {
                liability: Some(LiabilityTerms {
                    kind: LiabilityKind::Mortgage,
                    rate: None,
                    limit: None,
                    due_day: Some(5),
                    monthly_payment: Some(amount("400")),
                }),
                ..StoreExtras::default()
            },
        )
        .unwrap()
        .id;
    let exchange = store(&mut vault, "Exchange", StoreKind::Exchange, None);
    vault
        .add_transaction(earn(&exchange, "2000", "2026-03-01T10:00:00Z"))
        .unwrap();
    let cash_out = usd_move(&exchange, &card, "1500", "2026-03-02T10:00:00Z");
    vault.add_transaction(cash_out).unwrap();
    let repay = usd_move(&card, &loan, "400", "2026-03-05T10:00:00Z");
    vault.add_transaction(repay).unwrap();

    let summary = vault.month_summary("2026-03", "UTC").unwrap();
    assert_eq!(summary.income, Amount::ZERO);
    assert_eq!(summary.budget_in, amount("1500"));
    assert_eq!(summary.fixed, amount("400"));
}

#[test]
fn month_mode_carries_what_was_left_and_resets_next_month() {
    let (_dir, mut vault, card) = budget_vault();
    vault.doc.settings.savings_rate = Amount::ZERO;
    // 3100 income in a 31-day month: 100 a day.
    vault
        .add_transaction(earn(&card, "3100", "2026-03-01T08:00:00Z"))
        .unwrap();
    vault
        .add_transaction(spend(&card, "40", "2026-03-01T12:00:00Z"))
        .unwrap();
    vault
        .add_transaction(spend(&card, "180", "2026-03-02T12:00:00Z"))
        .unwrap();

    let day3 = daily_budget(&vault.books(), secs("2026-03-03T12:00:00Z"), "UTC");
    assert_eq!(day3.per_day, amount("100"));
    // +60 left on the 1st, −80 overspent on the 2nd.
    assert_eq!(day3.carry, amount("-20"));
    assert_eq!(day3.available, amount("80"));

    let april = daily_budget(&vault.books(), secs("2026-04-01T12:00:00Z"), "UTC");
    assert_eq!(april.carry, Amount::ZERO);
    assert_eq!(april.income, Amount::ZERO);
}

#[test]
fn planned_income_counts_until_the_salary_arrives_and_savings_rate_applies() {
    let (_dir, mut vault, card) = budget_vault();
    let Transaction::Topup(salary) = income(&card, "3000", "salary") else {
        unreachable!()
    };
    vault
        .create_automation(AutomationDraft {
            time: None,
            asks: false,
            for_month_offset: None,
            name: "Salary".into(),
            active: true,
            preset: None,
            schedule: Schedule::Monthly { day: 25 },
            starts_at: "2026-01-01T00:00:00Z".into(),
            template: TransactionDraft::Topup(salary),
            amount_mode: Default::default(),
            workday_shift: Default::default(),
            property_id: None,
        })
        .unwrap();
    let early = daily_budget(&vault.books(), secs("2026-06-03T12:00:00Z"), "UTC");
    assert_eq!(early.income, amount("3000"));
    assert_eq!(early.savings_plan, amount("300"));
    assert_eq!(early.per_day, amount("90"));

    vault.doc.settings.savings_rate = Amount::ZERO;
    vault
        .add_transaction(earn(&card, "4500", "2026-06-04T08:00:00Z"))
        .unwrap();
    let after = daily_budget(&vault.books(), secs("2026-06-05T12:00:00Z"), "UTC");
    assert_eq!(after.income, amount("4500"));
}

#[test]
fn until_income_mode_without_salary_spreads_to_month_end() {
    let (_dir, mut vault, card) = budget_vault();
    vault.doc.settings.daily_budget_mode = DailyBudgetMode::UntilIncome;
    vault
        .add_transaction(earn(&card, "1100", "2026-03-01T08:00:00Z"))
        .unwrap();
    // The 21st: 11 days left in March.
    let today = daily_budget(&vault.books(), secs("2026-03-21T12:00:00Z"), "UTC");
    assert_eq!(today.days, 11);
    assert_eq!(today.per_day, amount("100"));
    assert_eq!(today.next_income, None);
}

#[test]
fn spending_off_budget_stores_does_not_touch_the_daily_budget() {
    let (_dir, mut vault, card) = budget_vault();
    let exchange = store(&mut vault, "Exchange", StoreKind::Exchange, None);
    vault
        .add_transaction(earn(&card, "3100", "2026-03-01T08:00:00Z"))
        .unwrap();
    vault
        .add_transaction(earn(&exchange, "500", "2026-03-01T08:00:00Z"))
        .unwrap();
    vault
        .add_transaction(spend(&exchange, "300", "2026-03-01T12:00:00Z"))
        .unwrap();
    let day = daily_budget(&vault.books(), secs("2026-03-01T13:00:00Z"), "UTC");
    assert_eq!(day.spent_today, Amount::ZERO);
}

#[test]
fn the_day_is_cut_at_local_midnight() {
    let (_dir, mut vault, card) = budget_vault();
    vault
        .add_transaction(earn(&card, "3100", "2026-03-01T08:00:00Z"))
        .unwrap();
    // 20:00 UTC on the 1st is 03:00 on the 2nd in Bangkok.
    vault
        .add_transaction(spend(&card, "50", "2026-03-01T20:00:00Z"))
        .unwrap();
    let utc = daily_budget(&vault.books(), secs("2026-03-01T21:00:00Z"), "UTC");
    let bangkok = daily_budget(&vault.books(), secs("2026-03-01T21:00:00Z"), "Asia/Bangkok");
    assert_eq!(utc.spent_today, amount("50"));
    assert_eq!(bangkok.date, "2026-03-02");
    assert_eq!(bangkok.spent_today, amount("50"));
    let bangkok_first = daily_budget(&vault.books(), secs("2026-03-01T15:00:00Z"), "Asia/Bangkok");
    assert_eq!(bangkok_first.spent_today, Amount::ZERO);
}

#[test]
fn a_rub_limit_with_a_usd_base_and_a_thb_budget_use_the_day_rate() {
    let (_dir, mut vault) = empty_vault();
    vault.set_base_currency("usd");
    vault.doc.settings.budget_currency = "thb".into();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    let rub = AssetRef::fiat("RUB", "Ruble");
    let fx = |code: &str, rate: &str, ts: &str| FxPoint {
        currency: code.into(),
        per_usd: amount(rate),
        ts: ts.into(),
        source: "test".into(),
    };
    // Mark RUB as used so it keeps history.
    let mut seed = income_of(&card, rub.clone(), "10000", "0.01", "seed");
    at(&mut seed, "2026-01-01T00:00:00Z");
    vault.add_transaction(seed).unwrap();
    vault.apply_fx(&[
        fx("rub", "100", "2026-03-01T00:00:00Z"),
        fx("thb", "40", "2026-03-01T00:00:00Z"),
    ]);
    vault.apply_fx(&[
        fx("rub", "50", "2026-04-01T00:00:00Z"),
        fx("thb", "20", "2026-04-01T00:00:00Z"),
    ]);

    let mut groceries = payment(&card, "1000", "food");
    if let Transaction::Payment(inner) = &mut groceries {
        inner.asset = rub.clone();
        inner.price = PriceStamp::default();
    }
    groceries.meta_mut().categories = vec!["groceries".into()];
    at(&mut groceries, "2026-03-10T10:00:00Z");
    vault.add_transaction(groceries).unwrap();

    // March rate: 1000 RUB = 10 USD = 400 THB, even though April's rates are newer.
    let march = vault.month_summary("2026-03", "UTC").unwrap();
    assert_eq!(march.currency, "thb");
    assert_eq!(march.daily, amount("400"));

    vault
        .create_budget(BudgetDraft {
            active: true,
            target: BudgetTarget::Category("groceries".into()),
            period: BudgetPeriod::Month,
            limit: amount("5000"),
            currency: "rub".into(),
            warn: true,
        })
        .unwrap();
    let status = vault.budget_status(secs("2026-03-20T00:00:00Z"), "UTC");
    assert_eq!(status[0].spent, amount("1000"));
    assert_eq!(status[0].remaining, amount("4000"));
}

/// C2-8: spending that has no rate to the limit's currency is not worth
/// zero, it is not valued: the status says so.
#[test]
fn a_limit_with_spending_that_has_no_rate_is_marked_not_valued() {
    let (_dir, mut vault, card) = budget_vault();
    let mut abroad = payment(&card, "1000", "food");
    if let Transaction::Payment(inner) = &mut abroad {
        inner.asset = AssetRef::fiat("RUB", "Ruble");
        inner.price = PriceStamp::default();
    }
    abroad.meta_mut().categories = vec!["groceries".into()];
    at(&mut abroad, "2026-03-10T10:00:00Z");
    vault.add_transaction(abroad).unwrap();
    vault
        .add_transaction(spend(&card, "30", "2026-03-11T10:00:00Z"))
        .unwrap();
    vault
        .create_budget(BudgetDraft {
            active: true,
            target: BudgetTarget::Category("groceries".into()),
            period: BudgetPeriod::Month,
            limit: amount("500"),
            currency: "usd".into(),
            warn: true,
        })
        .unwrap();
    let status = vault.budget_status(secs("2026-03-20T00:00:00Z"), "UTC");
    // The valued part still counts; the rest is flagged, not taken as zero.
    assert_eq!(status[0].spent, amount("30"));
    assert!(status[0].not_valued);
}

#[test]
fn a_limit_whose_spending_is_all_valued_is_not_marked() {
    let (_dir, mut vault, card) = budget_vault();
    vault
        .add_transaction(spend(&card, "30", "2026-03-11T10:00:00Z"))
        .unwrap();
    vault
        .create_budget(BudgetDraft {
            active: true,
            target: BudgetTarget::Total,
            period: BudgetPeriod::Month,
            limit: amount("500"),
            currency: "usd".into(),
            warn: true,
        })
        .unwrap();
    let status = vault.budget_status(secs("2026-03-20T00:00:00Z"), "UTC");
    assert_eq!(status[0].spent, amount("30"));
    assert!(!status[0].not_valued);
}

/// 26-b: a typed monthly figure fixes the currency it was typed in, so a
/// device with another base does not read the number as its own money.
#[test]
fn saving_a_manual_figure_fixes_the_budget_currency() {
    use crate::ipc_contract::UpdateSettingsInput;
    let (_dir, mut vault, _card) = budget_vault();
    vault.set_base_currency("rub");
    assert!(vault.doc.settings.budget_currency.is_empty());
    vault
        .update_settings(UpdateSettingsInput {
            manual_income: Some(amount("3000")),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(vault.doc.settings.budget_currency, "rub");
    // The base moves on another device; the figure stays in rubles.
    vault.set_base_currency("usd");
    assert_eq!(vault.budget_currency(), "rub");
}

#[test]
fn the_budget_currency_is_not_touched_without_a_manual_figure() {
    use crate::ipc_contract::UpdateSettingsInput;
    let (_dir, mut vault, _card) = budget_vault();
    vault
        .update_settings(UpdateSettingsInput {
            savings_rate: Some(amount("0.2")),
            manual_income: Some(Amount::ZERO),
            ..Default::default()
        })
        .unwrap();
    assert!(vault.doc.settings.budget_currency.is_empty());
    // An explicit choice is kept.
    vault.doc.settings.budget_currency = "thb".into();
    vault
        .update_settings(UpdateSettingsInput {
            manual_expenses: Some(amount("100")),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(vault.doc.settings.budget_currency, "thb");
}
