//! Tests of the dashboard area: the old inline tests (names and bodies unchanged).

use crate::Amount;
use crate::model::store::StoreKind;
use crate::model::transaction::Transaction;
use crate::model::vault::DashPeriod;
use crate::pricing::history::unix_secs;
use crate::service::UnlockedVault;
use crate::service::test_support::*;

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
        vault
            .add_transaction(spend(&card, sum, category, ts))
            .unwrap();
    }
    let now = unix_secs("2026-10-22T12:00:00Z").unwrap();
    let view = vault.spending_overview(DashPeriod::Month, now, "UTC");
    assert_eq!(view.start, "2026-10-01");
    assert_eq!(view.spent.value, amount("80"));
    assert_eq!(view.spent.base, Some(amount("540")));
    assert_eq!(view.daily.base, Some(amount("40")));
    assert_eq!(view.daily.delta_pct(), Some(amount("100")));
    assert_eq!(view.series.len(), 22);
    assert_eq!(view.series[2], amount("60"));
    // The week: 16..22 October, 20 on the 20th; the week before has nothing.
    assert_eq!(view.week.len(), 7);
    assert_eq!(view.week[0].date, "2026-10-16");
    assert_eq!(view.week[4].daily, amount("20"));
    assert_eq!(view.week_total.value, amount("20"));
    assert_eq!(view.week_total.base, Some(Amount::ZERO));
}

#[test]
fn a_new_vault_has_nothing_to_compare_with() {
    let (_dir, mut vault, card) = vault_with_card();
    vault
        .add_transaction(spend(&card, "15", "groceries", "2026-10-21T10:00:00Z"))
        .unwrap();
    let now = unix_secs("2026-10-22T12:00:00Z").unwrap();
    let view = vault.spending_overview(DashPeriod::Year, now, "UTC");
    assert_eq!(view.start, "2026-01-01");
    assert_eq!(view.spent.base, None);
    assert_eq!(view.series.len(), 10);
    assert_eq!(view.series[9], amount("15"));
}
