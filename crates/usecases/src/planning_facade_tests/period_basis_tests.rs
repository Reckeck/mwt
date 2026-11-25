//! op-period T12: acceptance A8 (and the reading part of A6, M3, R5): the design's November
//! totals in both readings.
//!
//! The design data (`ui-review/_notes/redesign-coverage.md`), all in USD at a price of 1:
//!
//! - income on 3 Nov: salary 120 000 «for» 16-31 Oct, advance 30 000 «for» 1-15 Nov;
//! - spending in November without a period: rent 40 000 (fixed) + groceries 23 200 (daily)
//!   = 63 200, against one total `Month` limit of 86 000;
//! - «себе»: deposit 20 000 + Bybit 15 000 = 35 000 moved out of the budget.
//!
//! Derived numbers, written down before the assertions:
//!
//! | reading    | Oct income | Nov income | Nov spent | Nov left | Nov `budget_out` |
//! |------------|-----------:|-----------:|----------:|---------:|-----------------:|
//! | money date |          0 |    150 000 |    63 200 |   22 800 |           35 000 |
//! | period     |    120 000 |     30 000 |    63 200 |   22 800 |           35 000 |
//!
//! Only the salary moves (120 000 from November to October); the advance sits in November in
//! both readings; spending and the «себе» moves carry no period and read the same.

use crate::Amount;
use crate::model::planning::{BudgetPeriod, BudgetTarget};
use crate::model::store::{Purpose, StoreKind};
use crate::model::transaction::{AccountRef, Transaction};
use crate::planning::budget::{BudgetDraft, MonthSummary};
use crate::planning::flows::{Basis, Flow, flows_for_basis};
use crate::service::Session;
use crate::service::test_support::*;
use crate::time::unix_secs;
use mwt_model::{OpPeriod, PeriodKind};
use mwt_money::worth::FlowClass;

const BASES: [Basis; 2] = [Basis::Period, Basis::MoneyDate];

struct Design {
    _dir: tempfile::TempDir,
    vault: Session,
    card: String,
}

fn dated(mut tx: Transaction, ts: &str) -> Transaction {
    at(&mut tx, ts);
    tx
}

fn labelled(mut tx: Transaction, kind: PeriodKind, start: &str, end: &str) -> Transaction {
    tx.meta_mut().period = Some(OpPeriod {
        kind,
        start: start.into(),
        end: end.into(),
    });
    tx
}

fn in_category(mut tx: Transaction, key: &str) -> Transaction {
    tx.meta_mut().categories = vec![key.into()];
    tx
}

fn add(vault: &mut Session, tx: Transaction) {
    crate::usecases::money::transactions::add_transaction(vault, tx).unwrap();
}

/// A card in the budget and a deposit outside it (savings purpose), USD everywhere.
fn design_vault() -> (Design, String) {
    let (dir, mut vault) = empty_vault();
    vault.set_base_currency("usd");
    quote_usd(&mut vault);
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    let deposit = store(&mut vault, "Deposit", StoreKind::Bank, None);
    vault
        .doc
        .stores
        .iter_mut()
        .find(|candidate| candidate.id == deposit)
        .unwrap()
        .purpose = Purpose::Savings;
    let design = Design {
        _dir: dir,
        vault,
        card,
    };
    (design, deposit)
}

fn november_design() -> Design {
    let (mut design, deposit) = design_vault();
    let bybit = store(&mut design.vault, "Bybit", StoreKind::Exchange, None);
    let card = design.card.clone();
    let vault = &mut design.vault;
    add(
        vault,
        labelled(
            dated(income(&card, "120000", "salary"), "2026-11-03T10:00:00Z"),
            PeriodKind::Range,
            "2026-10-16",
            "2026-10-31",
        ),
    );
    add(
        vault,
        labelled(
            dated(income(&card, "30000", "advance"), "2026-11-03T11:00:00Z"),
            PeriodKind::Half,
            "2026-11-01",
            "2026-11-15",
        ),
    );
    add(
        vault,
        in_category(
            dated(payment(&card, "40000", "rent"), "2026-11-05T10:00:00Z"),
            "rent_mortgage",
        ),
    );
    add(
        vault,
        dated(payment(&card, "23200", "food"), "2026-11-10T10:00:00Z"),
    );
    for (target, sum, ts) in [
        (&deposit, "20000", "2026-11-12T10:00:00Z"),
        (&bybit, "15000", "2026-11-13T10:00:00Z"),
    ] {
        add(
            vault,
            dated(
                transfer_between(
                    AccountRef::Store(card.clone()),
                    AccountRef::Store(target.clone()),
                    usd(),
                    sum,
                ),
                ts,
            ),
        );
    }
    crate::usecases::planning::budget::create_budget(
        vault,
        BudgetDraft {
            target: BudgetTarget::Total,
            period: BudgetPeriod::Month,
            limit: amount("86000"),
            currency: "usd".into(),
            active: true,
            warn: false,
        },
    )
    .unwrap();
    design
}

fn summary(vault: &Session, month: &str, basis: Basis) -> MonthSummary {
    crate::usecases::planning::budget::month_summary_with_basis(
        &vault.books(),
        month,
        "UTC",
        Some(basis),
    )
    .unwrap()
}

fn november_status(vault: &Session, basis: Basis) -> (Amount, Amount) {
    let statuses = crate::usecases::planning::budget::budget_status_by(
        &vault.books(),
        unix_secs("2026-11-20T12:00:00Z").unwrap(),
        "UTC",
        Some(basis),
    );
    (statuses[0].spent, statuses[0].remaining)
}

fn month_flows(vault: &Session, basis: Basis, from: &str, to: &str) -> Vec<Flow> {
    let cal = crate::calendar::Calendar::new("UTC");
    flows_for_basis(
        &vault.books(),
        &cal,
        "USD",
        unix_secs(from).unwrap(),
        unix_secs(to).unwrap(),
        basis,
    )
}

/// A8, R1, R4: by money date November holds all 150 000 and 22 800 of the limit is left.
#[test]
fn a8_money_date_reading_of_november() {
    let design = november_design();
    let november = summary(&design.vault, "2026-11", Basis::MoneyDate);
    assert_eq!(november.income, amount("150000"));
    assert_eq!(november.fixed + november.daily, amount("63200"));
    assert_eq!(
        summary(&design.vault, "2026-10", Basis::MoneyDate).income,
        amount("0")
    );
    let (spent, remaining) = november_status(&design.vault, Basis::MoneyDate);
    assert_eq!(spent, amount("63200"));
    assert_eq!(remaining, amount("22800"));
}

/// A8, R1, R4: by period the salary moves to October, the advance stays in November.
#[test]
fn a8_period_reading_of_november_and_october() {
    let design = november_design();
    let november = summary(&design.vault, "2026-11", Basis::Period);
    assert_eq!(november.income, amount("30000"));
    assert_eq!(
        summary(&design.vault, "2026-10", Basis::Period).income,
        amount("120000")
    );
    assert_eq!(november.fixed + november.daily, amount("63200"));
    let (spent, remaining) = november_status(&design.vault, Basis::Period);
    assert_eq!(spent, amount("63200"));
    assert_eq!(remaining, amount("22800"));
}

/// A8, R8: spending and «себе» read the same in both readings; «себе» is never spending.
#[test]
fn a8_spending_and_own_moves_are_equal_in_both_readings() {
    let design = november_design();
    let by_period = summary(&design.vault, "2026-11", Basis::Period);
    let by_date = summary(&design.vault, "2026-11", Basis::MoneyDate);
    for reading in [&by_period, &by_date] {
        assert_eq!(reading.fixed, amount("40000"));
        assert_eq!(reading.daily, amount("23200"));
        assert_eq!(reading.budget_out, amount("35000"));
        assert_eq!(reading.savings_moved, amount("0"));
    }
    assert_eq!(
        (by_period.fixed, by_period.daily, by_period.budget_out),
        (by_date.fixed, by_date.daily, by_date.budget_out)
    );
}

/// A6, R2, Q3: a quarter-long period without a spread counts whole in the money-date month.
#[test]
fn a6_quarter_period_without_spread_counts_whole_in_the_money_date_month() {
    let (mut design, _deposit) = design_vault();
    let card = design.card.clone();
    add(
        &mut design.vault,
        labelled(
            dated(payment(&card, "900", "insurance"), "2026-11-10T10:00:00Z"),
            PeriodKind::Quarter,
            "2026-10-01",
            "2026-12-31",
        ),
    );
    for basis in BASES {
        assert_eq!(
            summary(&design.vault, "2026-11", basis).daily,
            amount("900")
        );
        for other in ["2026-10", "2026-12"] {
            assert_eq!(summary(&design.vault, other, basis).daily, amount("0"));
        }
    }
}

/// M3: an old income with `details.for_month` and no `period` counts in the month of its date.
#[test]
fn m3_for_month_without_a_period_counts_in_the_month_of_the_date() {
    let (mut design, _deposit) = design_vault();
    let card = design.card.clone();
    let mut old = dated(income(&card, "5000", "salary"), "2026-11-03T10:00:00Z");
    if let Transaction::Topup(inner) = &mut old {
        inner.details.for_month = Some("2026-10".into());
    }
    add(&mut design.vault, old);
    for basis in BASES {
        assert_eq!(
            summary(&design.vault, "2026-11", basis).income,
            amount("5000")
        );
        assert_eq!(summary(&design.vault, "2026-10", basis).income, amount("0"));
    }
}

/// R5: a transfer to an own account or out of the budget is never `Daily`/`Fixed`, in either
/// reading (a transfer cannot carry a period, so the readings agree).
#[test]
fn r5_own_transfers_are_never_daily_or_fixed() {
    let (mut design, deposit) = design_vault();
    let pocket = store(
        &mut design.vault,
        "Pocket",
        StoreKind::Bank,
        Some(&design.card),
    );
    let card = design.card.clone();
    for target in [&pocket, &deposit] {
        add(
            &mut design.vault,
            dated(
                transfer_between(
                    AccountRef::Store(card.clone()),
                    AccountRef::Store(target.clone()),
                    usd(),
                    "700",
                ),
                "2026-11-12T10:00:00Z",
            ),
        );
    }
    for basis in BASES {
        let flows = month_flows(
            &design.vault,
            basis,
            "2026-10-01T00:00:00Z",
            "2026-12-01T00:00:00Z",
        );
        assert!(
            flows
                .iter()
                .all(|flow| !matches!(flow.class, FlowClass::Daily | FlowClass::Fixed)),
            "{basis:?}: {flows:?}"
        );
        let november = summary(&design.vault, "2026-11", basis);
        assert_eq!(november.daily + november.fixed, amount("0"));
        assert_eq!(november.budget_out, amount("700"));
    }
}
