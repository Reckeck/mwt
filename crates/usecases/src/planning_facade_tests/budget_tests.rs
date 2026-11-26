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
use crate::planning::automations::AutomationDraft;
use crate::planning::budget::*;
use crate::planning::daily_budget::daily_budget;
use crate::service::Session;
use crate::service::StoreExtras;
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
fn budget_vault() -> (tempfile::TempDir, Session, String) {
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
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        earn(&card, "1000", "2026-03-01T10:00:00Z"),
    )
    .unwrap();
    let mut rent = spend(&card, "300", "2026-03-02T10:00:00Z");
    rent.meta_mut().categories = vec!["rent_mortgage".into()];
    crate::usecases::money::transactions::add_transaction(&mut vault, rent).unwrap();
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        spend(&card, "200", "2026-03-03T10:00:00Z"),
    )
    .unwrap();
    // Internal and debt moves are neither income nor spending.
    let inner = usd_move(&card, &pocket, "100", "2026-03-04T10:00:00Z");
    crate::usecases::money::transactions::add_transaction(&mut vault, inner).unwrap();
    let friend = crate::usecases::household::contacts::create_contact(
        &mut vault,
        crate::household::contacts::ContactDraft {
            group: String::new(),
            name: "Friend".into(),
            kind: crate::model::contact::ContactKind::Person,
            note: String::new(),
            accounts: Vec::new(),
        },
    )
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
    crate::usecases::money::transactions::add_transaction(&mut vault, lend).unwrap();
    // Another month does not count.
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        spend(&card, "999", "2026-04-01T10:00:00Z"),
    )
    .unwrap();

    let summary =
        crate::usecases::planning::budget::month_summary(&vault.books(), "2026-03", "UTC").unwrap();
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
    assert_eq!(
        crate::planning::budget::avg_month(&vault.books(), 3, now, "UTC"),
        None
    );
    for (sum, ts) in [
        ("900", "2026-01-10T10:00:00Z"),
        ("1000", "2026-02-10T10:00:00Z"),
        ("1200", "2026-04-10T10:00:00Z"),
        // The current month is not finished yet.
        ("5000", "2026-05-01T10:00:00Z"),
    ] {
        crate::usecases::money::transactions::add_transaction(&mut vault, earn(&card, sum, ts))
            .unwrap();
    }
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        spend(&card, "300", "2026-04-11T10:00:00Z"),
    )
    .unwrap();

    // March had no flows and is skipped: April, February.
    let two = crate::planning::budget::avg_month(&vault.books(), 2, now, "UTC").unwrap();
    assert_eq!(two.months, 2);
    assert_eq!(two.income, amount("1100"));
    assert_eq!(two.daily, amount("150"));
    assert_eq!(two.currency, "usd");
    // Fewer months than asked: what there is.
    let all = crate::planning::budget::avg_month(&vault.books(), 12, now, "UTC").unwrap();
    assert_eq!(all.months, 3);
    assert_eq!(all.income, amount("1033.3333333333333333333333333"));
}

#[test]
fn a_loan_payment_is_mandatory_and_exchange_to_card_tops_up_the_budget() {
    let (_dir, mut vault, card) = budget_vault();
    let loan = crate::usecases::money::stores::create_store_with(
        &mut vault,
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
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        earn(&exchange, "2000", "2026-03-01T10:00:00Z"),
    )
    .unwrap();
    let cash_out = usd_move(&exchange, &card, "1500", "2026-03-02T10:00:00Z");
    crate::usecases::money::transactions::add_transaction(&mut vault, cash_out).unwrap();
    let repay = usd_move(&card, &loan, "400", "2026-03-05T10:00:00Z");
    crate::usecases::money::transactions::add_transaction(&mut vault, repay).unwrap();

    let summary =
        crate::usecases::planning::budget::month_summary(&vault.books(), "2026-03", "UTC").unwrap();
    assert_eq!(summary.income, Amount::ZERO);
    assert_eq!(summary.budget_in, amount("1500"));
    assert_eq!(summary.fixed, amount("400"));
}

#[test]
fn month_mode_carries_what_was_left_and_resets_next_month() {
    let (_dir, mut vault, card) = budget_vault();
    vault.doc.settings.savings_rate = Amount::ZERO;
    // 3100 income in a 31-day month: 100 a day.
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        earn(&card, "3100", "2026-03-01T08:00:00Z"),
    )
    .unwrap();
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        spend(&card, "40", "2026-03-01T12:00:00Z"),
    )
    .unwrap();
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        spend(&card, "180", "2026-03-02T12:00:00Z"),
    )
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
    crate::usecases::planning::automations::create_automation(
        &mut vault,
        AutomationDraft {
            time: None,
            asks: false,
            for_month_offset: None,
            operation_template_id: None,
            name: "Salary".into(),
            active: true,
            preset: None,
            schedule: Schedule::Monthly { day: 25 },
            starts_at: "2026-01-01T00:00:00Z".into(),
            template: TransactionDraft::Topup(salary),
            amount_mode: Default::default(),
            workday_shift: Default::default(),
            property_id: None,
        },
    )
    .unwrap();
    let early = daily_budget(&vault.books(), secs("2026-06-03T12:00:00Z"), "UTC");
    assert_eq!(early.income, amount("3000"));
    assert_eq!(early.savings_plan, amount("300"));
    assert_eq!(early.per_day, amount("90"));

    vault.doc.settings.savings_rate = Amount::ZERO;
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        earn(&card, "4500", "2026-06-04T08:00:00Z"),
    )
    .unwrap();
    let after = daily_budget(&vault.books(), secs("2026-06-05T12:00:00Z"), "UTC");
    assert_eq!(after.income, amount("4500"));
}

#[test]
fn until_income_mode_without_salary_spreads_to_month_end() {
    let (_dir, mut vault, card) = budget_vault();
    vault.doc.settings.daily_budget_mode = DailyBudgetMode::UntilIncome;
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        earn(&card, "1100", "2026-03-01T08:00:00Z"),
    )
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
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        earn(&card, "3100", "2026-03-01T08:00:00Z"),
    )
    .unwrap();
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        earn(&exchange, "500", "2026-03-01T08:00:00Z"),
    )
    .unwrap();
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        spend(&exchange, "300", "2026-03-01T12:00:00Z"),
    )
    .unwrap();
    let day = daily_budget(&vault.books(), secs("2026-03-01T13:00:00Z"), "UTC");
    assert_eq!(day.spent_today, Amount::ZERO);
}

#[test]
fn the_day_is_cut_at_local_midnight() {
    let (_dir, mut vault, card) = budget_vault();
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        earn(&card, "3100", "2026-03-01T08:00:00Z"),
    )
    .unwrap();
    // 20:00 UTC on the 1st is 03:00 on the 2nd in Bangkok.
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        spend(&card, "50", "2026-03-01T20:00:00Z"),
    )
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
    crate::usecases::money::transactions::add_transaction(&mut vault, seed).unwrap();
    crate::usecases::money::rates::apply_fx(
        &mut vault,
        &[
            fx("rub", "100", "2026-03-01T00:00:00Z"),
            fx("thb", "40", "2026-03-01T00:00:00Z"),
        ],
    );
    crate::usecases::money::rates::apply_fx(
        &mut vault,
        &[
            fx("rub", "50", "2026-04-01T00:00:00Z"),
            fx("thb", "20", "2026-04-01T00:00:00Z"),
        ],
    );

    let mut groceries = payment(&card, "1000", "food");
    if let Transaction::Payment(inner) = &mut groceries {
        inner.asset = rub.clone();
        inner.price = PriceStamp::default();
    }
    groceries.meta_mut().categories = vec!["groceries".into()];
    at(&mut groceries, "2026-03-10T10:00:00Z");
    crate::usecases::money::transactions::add_transaction(&mut vault, groceries).unwrap();

    // March rate: 1000 RUB = 10 USD = 400 THB, even though April's rates are newer.
    let march =
        crate::usecases::planning::budget::month_summary(&vault.books(), "2026-03", "UTC").unwrap();
    assert_eq!(march.currency, "thb");
    assert_eq!(march.daily, amount("400"));

    crate::usecases::planning::budget::create_budget(
        &mut vault,
        BudgetDraft {
            active: true,
            target: BudgetTarget::Category("groceries".into()),
            period: BudgetPeriod::Month,
            limit: amount("5000"),
            currency: "rub".into(),
            warn: true,
        },
    )
    .unwrap();
    let status = crate::usecases::planning::budget::budget_status(
        &vault.books(),
        secs("2026-03-20T00:00:00Z"),
        "UTC",
    );
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
    crate::usecases::money::transactions::add_transaction(&mut vault, abroad).unwrap();
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        spend(&card, "30", "2026-03-11T10:00:00Z"),
    )
    .unwrap();
    crate::usecases::planning::budget::create_budget(
        &mut vault,
        BudgetDraft {
            active: true,
            target: BudgetTarget::Category("groceries".into()),
            period: BudgetPeriod::Month,
            limit: amount("500"),
            currency: "usd".into(),
            warn: true,
        },
    )
    .unwrap();
    let status = crate::usecases::planning::budget::budget_status(
        &vault.books(),
        secs("2026-03-20T00:00:00Z"),
        "UTC",
    );
    // The valued part still counts; the rest is flagged, not taken as zero.
    assert_eq!(status[0].spent, amount("30"));
    assert!(status[0].not_valued);
}

#[test]
fn a_limit_whose_spending_is_all_valued_is_not_marked() {
    let (_dir, mut vault, card) = budget_vault();
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        spend(&card, "30", "2026-03-11T10:00:00Z"),
    )
    .unwrap();
    crate::usecases::planning::budget::create_budget(
        &mut vault,
        BudgetDraft {
            active: true,
            target: BudgetTarget::Total,
            period: BudgetPeriod::Month,
            limit: amount("500"),
            currency: "usd".into(),
            warn: true,
        },
    )
    .unwrap();
    let status = crate::usecases::planning::budget::budget_status(
        &vault.books(),
        secs("2026-03-20T00:00:00Z"),
        "UTC",
    );
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
    assert_eq!(
        crate::planning::budget::budget_currency(&vault.books()),
        "rub"
    );
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

// -- op-period T9: budget readers by basis (R1, R3, R4) ---------------------

use crate::planning::flows::{Basis, flows_between};
use mwt_model::{OpPeriod, PeriodKind};

/// March card: an income of 1000 on 5 March and spending of 200 on 10 March.
/// With `with_period` the income is «for» February.
fn period_vault(with_period: bool) -> (tempfile::TempDir, Session) {
    let (dir, mut vault, card) = budget_vault();
    let mut salary = earn(&card, "1000", "2026-03-05T10:00:00Z");
    if with_period {
        salary.meta_mut().period = Some(OpPeriod {
            kind: PeriodKind::Month,
            start: "2026-02-01".into(),
            end: "2026-02-28".into(),
        });
    }
    crate::usecases::money::transactions::add_transaction(&mut vault, salary).unwrap();
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        spend(&card, "200", "2026-03-10T10:00:00Z"),
    )
    .unwrap();
    (dir, vault)
}

fn summary_by(vault: &Session, month: &str, basis: Option<Basis>) -> MonthSummary {
    crate::usecases::planning::budget::month_summary_with_basis(&vault.books(), month, "UTC", basis)
        .unwrap()
}

/// R1, R4: the month summary follows the basis; none means `period`.
#[test]
fn r4_month_summary_reads_the_period_by_default_and_the_money_date_on_request() {
    let (_dir, vault) = period_vault(true);
    let default = summary_by(&vault, "2026-03", None);
    assert_eq!(default, summary_by(&vault, "2026-03", Some(Basis::Period)));
    assert_eq!(default.income, amount("0"));
    assert_eq!(default.daily, amount("200"));
    let by_date = summary_by(&vault, "2026-03", Some(Basis::MoneyDate));
    assert_eq!(by_date.income, amount("1000"));
    assert_eq!(
        summary_by(&vault, "2026-02", None).income,
        amount("1000"),
        "the income counts in the month it is for"
    );
}

/// R1, R8: without any period the two readings give equal summaries.
#[test]
fn r1_without_a_period_both_bases_give_equal_summaries() {
    let (_dir, vault) = period_vault(false);
    for month in ["2026-02", "2026-03", "2026-04"] {
        assert_eq!(
            summary_by(&vault, month, Some(Basis::Period)),
            summary_by(&vault, month, Some(Basis::MoneyDate)),
        );
    }
}

fn month_budget_status(vault: &mut Session, period: BudgetPeriod, basis: Basis) -> BudgetStatus {
    vault
        .transact(|tx| {
            tx.doc_mut().budgets.clear();
            Ok(())
        })
        .unwrap();
    crate::usecases::planning::budget::create_budget(
        vault,
        BudgetDraft {
            target: BudgetTarget::Total,
            period,
            limit: amount("1000"),
            currency: "usd".into(),
            active: true,
            warn: false,
        },
    )
    .unwrap();
    let mut statuses = crate::usecases::planning::budget::budget_status_by(
        &vault.books(),
        secs("2026-03-12T12:00:00Z"),
        "UTC",
        Some(basis),
    );
    statuses.remove(0)
}

/// R4, R3: a `Month` limit follows the basis, a `Day` limit stays on the money date.
#[test]
fn r4_month_budget_follows_the_basis_but_day_budget_stays_on_the_money_date() {
    let (_dir, mut vault, card) = budget_vault();
    // Spending on 12 March that is «for» February.
    let mut lunch = spend(&card, "50", "2026-03-12T09:00:00Z");
    lunch.meta_mut().period = Some(OpPeriod {
        kind: PeriodKind::Month,
        start: "2026-02-01".into(),
        end: "2026-02-28".into(),
    });
    crate::usecases::money::transactions::add_transaction(&mut vault, lunch).unwrap();

    let month_period = month_budget_status(&mut vault, BudgetPeriod::Month, Basis::Period);
    let month_date = month_budget_status(&mut vault, BudgetPeriod::Month, Basis::MoneyDate);
    assert_eq!(month_period.spent, amount("0"));
    assert_eq!(month_date.spent, amount("50"));
    let day_period = month_budget_status(&mut vault, BudgetPeriod::Day, Basis::Period);
    assert_eq!(day_period.spent, amount("50"));
}

/// R3: the time-bound readers give identical numbers with and without a period.
#[test]
fn r3_time_bound_readers_ignore_the_period() {
    let (_dir_a, with_period) = period_vault(true);
    let (_dir_b, without) = period_vault(false);
    let now = secs("2026-03-20T12:00:00Z");

    assert_eq!(
        daily_budget(&with_period.books(), now, "UTC"),
        daily_budget(&without.books(), now, "UTC"),
        "daily_budget, month_allowance and left_today"
    );
    let allowance = |vault: &Session| {
        let cal = crate::calendar::Calendar::new("UTC");
        let (from, to) = cal.month_bounds(cal.date_of(now));
        let books = vault.books();
        let flows = flows_between(&books, &cal, "USD", from, to);
        crate::planning::daily_budget::month_allowance(
            &books,
            &cal,
            cal.date_of(now),
            "USD",
            &flows,
        )
    };
    assert_eq!(allowance(&with_period), allowance(&without));
    assert_eq!(
        crate::planning::upcoming::upcoming(&with_period.books(), 30, now, "UTC"),
        crate::planning::upcoming::upcoming(&without.books(), 30, now, "UTC"),
    );
    let balance = |vault: &Session| {
        crate::usecases::household::month_balance::month_balance(
            vault,
            &mwt_reports::overview::ReportFilter::default(),
            now,
            "UTC",
        )
        .unwrap()
    };
    assert_eq!(
        balance(&with_period),
        balance(&without),
        "month_balance.forecast"
    );
}
