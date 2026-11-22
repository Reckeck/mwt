//! Tests of the dashboard area: the old inline tests (names and bodies unchanged).

use crate::model::store::StoreKind;
use crate::model::transaction::Transaction;
use crate::model::vault::DashPeriod;
use crate::service::UnlockedVault;
use crate::service::test_support::*;
use crate::time::unix_secs;
use pretty_assertions::assert_eq;

fn spend(store: &str, sum: &str, category: &str, ts: &str) -> Transaction {
    let mut tx = payment(store, sum, "spend");
    tx.meta_mut().categories = vec![category.into()];
    at(&mut tx, ts);
    tx
}

fn vault_with_card() -> (tempfile::TempDir, UnlockedVault, String) {
    let (dir, mut vault) = empty_vault();
    vault.set_base_currency("usd");
    quote_usd(&mut vault);
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    (dir, vault, card)
}

#[test]
fn spending_compares_to_the_same_date_of_last_month() {
    let (_dir, mut vault, card) = vault_with_card();
    for (sum, category, ts) in [
        ("40", "groceries", "2026-09-05T10:00:00Z"),
        ("500", "rent_mortgage", "2026-09-10T10:00:00Z"),
        // After the same date last month: not in the base.
        ("70", "groceries", "2026-09-25T10:00:00Z"),
        ("60", "groceries", "2026-10-03T10:00:00Z"),
        ("20", "groceries", "2026-10-20T10:00:00Z"),
    ] {
        crate::usecases::money::transactions::add_transaction(
            &mut vault,
            spend(&card, sum, category, ts),
        )
        .unwrap();
    }
    let now = unix_secs("2026-10-22T12:00:00Z").unwrap();
    let view =
        crate::reports::dashboard::spending_overview(&vault.books(), DashPeriod::Month, now, "UTC");
    assert_eq!(view.start, "2026-10-01");
    assert_eq!(view.daily.delta_pct(), Some(amount("100")));
    // The whole view, pinned: the month so far (80) against the same date of
    // last month (540), the daily series, and the week 16..22 October with 20
    // on the 20th while the week before has nothing.
    insta::assert_json_snapshot!(view);
}

#[test]
fn a_new_vault_has_nothing_to_compare_with() {
    let (_dir, mut vault, card) = vault_with_card();
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        spend(&card, "15", "groceries", "2026-10-21T10:00:00Z"),
    )
    .unwrap();
    let now = unix_secs("2026-10-22T12:00:00Z").unwrap();
    let view =
        crate::reports::dashboard::spending_overview(&vault.books(), DashPeriod::Year, now, "UTC");
    assert_eq!(view.start, "2026-01-01");
    assert_eq!(view.spent.base, None);
    assert_eq!(view.series.len(), 10);
    assert_eq!(view.series[9], amount("15"));
}
