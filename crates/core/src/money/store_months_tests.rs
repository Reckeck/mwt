//! Tests of the store_months area: the old inline test (name and body unchanged),
//! then the pure function on a read context and the delegate pin.

use super::store_months::*;
use crate::model::ids::StoreId;
use crate::model::store::StoreKind;
use crate::service::UnlockedVault;
use crate::service::test_support::*;

#[test]
fn store_months_split_inflow_and_outflow_by_month() {
    let (_dir, mut vault) = empty_vault();
    let bank = store(&mut vault, "Bank", StoreKind::Bank, None);
    let mut salary = income_of(&bank, usd(), "1000", "1", "salary");
    at(&mut salary, "2026-09-10T12:00:00Z");
    vault.add_transaction(salary).unwrap();
    let mut rent = payment(&bank, "300", "rent");
    at(&mut rent, "2026-10-05T12:00:00Z");
    vault.add_transaction(rent).unwrap();
    let mut food = payment(&bank, "50", "food");
    at(&mut food, "2026-10-06T12:00:00Z");
    vault.add_transaction(food).unwrap();

    let now = crate::time::secs_or_now("2026-10-19T12:00:00Z");
    let rows = vault.store_months(&bank, true, 2, now, "UTC");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].month, "2026-09");
    assert_eq!(rows[0].inflow, amount("1000"));
    assert_eq!(rows[1].month, "2026-10");
    assert_eq!(rows[1].outflow, amount("350"));
    assert_eq!(rows[1].tx_count, 2);
}

fn rows(
    vault: &UnlockedVault,
    store_id: &str,
    include_children: bool,
    months: u32,
    now: i64,
) -> Vec<StoreMonthRow> {
    let books = vault.books();
    store_months(
        &books,
        &StoreId::from(store_id),
        include_children,
        months,
        now,
        "UTC",
        |leg, currency, secs| UnlockedVault::leg_value_in(books.rate_book(), leg, currency, secs),
    )
}

fn two_wallets() -> (tempfile::TempDir, UnlockedVault, String, String) {
    let (dir, mut vault) = empty_vault();
    let parent = store(&mut vault, "Bank", StoreKind::Bank, None);
    let child = store(&mut vault, "Card", StoreKind::Bank, Some(&parent));
    let mut salary = income_of(&child, usd(), "100", "1", "salary");
    at(&mut salary, "2026-10-02T12:00:00Z");
    vault.add_transaction(salary).unwrap();
    (dir, vault, parent, child)
}

#[test]
fn children_count_only_when_asked_and_an_empty_scope_has_empty_months() {
    let (_dir, vault, parent, _child) = two_wallets();
    let now = crate::time::secs_or_now("2026-10-19T12:00:00Z");
    let with_children = rows(&vault, &parent, true, 1, now);
    let alone = rows(&vault, &parent, false, 1, now);
    assert_eq!(with_children[0].inflow, amount("100"));
    assert_eq!(with_children[0].tx_count, 1);
    assert_eq!(
        alone[0],
        StoreMonthRow {
            month: "2026-10".into(),
            ..StoreMonthRow::default()
        }
    );
}

#[test]
fn zero_months_gives_the_current_one_and_the_delegate_equals_the_function() {
    let (_dir, vault, parent, _child) = two_wallets();
    let now = crate::time::secs_or_now("2026-10-19T12:00:00Z");
    let direct = rows(&vault, &parent, true, 0, now);
    assert_eq!(direct.len(), 1);
    assert_eq!(direct, vault.store_months(&parent, true, 0, now, "UTC"));
}
