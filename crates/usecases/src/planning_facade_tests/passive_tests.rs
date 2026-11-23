//! Tests of the passive-income area: the old inline tests (names and bodies unchanged).

use crate::Amount;
use crate::model::category::CategoryGroup;
use crate::model::planning::EmergencyFund;
use crate::model::store::{Purpose, StoreKind, StoreScope};
use crate::model::transaction::Transaction;
use crate::service::test_support::*;
use crate::time::unix_secs;

fn booked(mut tx: Transaction, category: &str, ts: &str) -> Transaction {
    tx.meta_mut().categories = vec![category.into()];
    at(&mut tx, ts);
    tx
}

#[test]
fn passive_follows_the_defaults_until_the_user_says_otherwise() {
    let (_dir, mut vault) = empty_vault();
    assert!(crate::planning::passive::is_passive_category(
        &vault.doc,
        "deposit_interest"
    ));
    assert!(
        crate::planning::passive::is_passive_category(&vault.doc, "earn"),
        "Bybit Earn has no category record"
    );
    assert!(!crate::planning::passive::is_passive_category(
        &vault.doc, "cashback"
    ));
    assert!(!crate::planning::passive::is_passive_category(
        &vault.doc, "salary"
    ));
    crate::usecases::household::categories::update_category(
        &mut vault,
        "cashback",
        None,
        CategoryGroup::Income,
        None,
        Some(true),
    )
    .unwrap();
    crate::usecases::household::categories::update_category(
        &mut vault,
        "dividends",
        None,
        CategoryGroup::Income,
        None,
        Some(false),
    )
    .unwrap();
    assert!(crate::planning::passive::is_passive_category(
        &vault.doc, "cashback"
    ));
    assert!(!crate::planning::passive::is_passive_category(
        &vault.doc,
        "dividends"
    ));
    crate::usecases::household::categories::update_category(
        &mut vault,
        "cashback",
        None,
        CategoryGroup::Daily,
        None,
        None,
    )
    .unwrap();
    assert!(
        !crate::planning::passive::is_passive_category(&vault.doc, "cashback"),
        "only income can be passive"
    );
}

#[test]
fn old_vaults_get_the_new_categories_once() {
    let (_dir, mut vault) = empty_vault();
    vault
        .doc
        .categories
        .retain(|category| !matches!(category.key.as_str(), "coupons" | "staking" | "rent_income"));
    let dividends = vault
        .doc
        .categories
        .iter_mut()
        .find(|category| category.key == "dividends")
        .unwrap();
    dividends.name = Some("Дивы".into());
    dividends.archived = true;
    vault.doc.settings.categories_rev = 0;
    let before = vault.doc.categories.len();

    assert!(crate::usecases::planning::passive::ensure_default_categories(&mut vault));
    assert_eq!(vault.doc.categories.len(), before + 3);
    let dividends = vault
        .doc
        .categories
        .iter()
        .find(|category| category.key == "dividends")
        .unwrap();
    assert_eq!(dividends.name.as_deref(), Some("Дивы"));
    assert!(dividends.archived);
    assert!(
        !crate::usecases::planning::passive::ensure_default_categories(&mut vault),
        "a second open adds nothing"
    );
    assert_eq!(vault.doc.categories.len(), before + 3);

    // Deleted after its batch arrived: stays deleted.
    crate::usecases::household::categories::delete_category(&mut vault, "coupons").unwrap();
    assert!(!crate::usecases::planning::passive::ensure_default_categories(&mut vault));
    assert!(
        !vault
            .doc
            .categories
            .iter()
            .any(|category| category.key == "coupons")
    );
    // Only the batch's keys come back, not other deleted built-ins.
    // Both as a vault from before the batch has them: missing, with no
    // tombstone, so `taxi` is held back by the batch and not by the
    // tombstone check (that case is in `stale_base_tests`).
    crate::usecases::household::categories::delete_category(&mut vault, "taxi").unwrap();
    vault.doc.sync.tombstones.remove("categories:coupons");
    vault.doc.sync.tombstones.remove("categories:taxi");
    vault.doc.settings.categories_rev = 0;
    assert!(crate::usecases::planning::passive::ensure_default_categories(&mut vault));
    assert!(
        vault
            .doc
            .categories
            .iter()
            .any(|category| category.key == "coupons")
    );
    assert!(
        !vault
            .doc
            .categories
            .iter()
            .any(|category| category.key == "taxi")
    );
}

#[test]
fn passive_income_and_the_freedom_index() {
    let (_dir, mut vault) = empty_vault();
    let bank = store(&mut vault, "Bank", StoreKind::Bank, None);
    let broker = store(&mut vault, "Broker", StoreKind::Exchange, None);
    vault.doc.stores.iter_mut().for_each(|store| {
        if store.id == broker {
            store.purpose = Purpose::Investment;
        }
    });
    vault.doc.emergency_fund.avg_months = 3;
    // Three finished months of mandatory spending: 1000 a month.
    for month in ["2026-07", "2026-08", "2026-09"] {
        let salary = income(&bank, "5000", "salary");
        crate::usecases::money::transactions::add_transaction(
            &mut vault,
            booked(salary, "salary", &format!("{month}-01T10:00:00Z")),
        )
        .unwrap();
        let rent = payment(&bank, "1000", "rent");
        crate::usecases::money::transactions::add_transaction(
            &mut vault,
            booked(rent, "utilities", &format!("{month}-05T10:00:00Z")),
        )
        .unwrap();
    }
    let interest = income(&bank, "60", "deposit_interest");
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        booked(interest, "deposit_interest", "2026-08-10T10:00:00Z"),
    )
    .unwrap();
    let dividend = income(&broker, "60", "dividends");
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        booked(dividend, "dividends", "2026-09-10T10:00:00Z"),
    )
    .unwrap();
    let early = income(&bank, "999", "deposit_interest");
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        booked(early, "deposit_interest", "2025-09-30T10:00:00Z"),
    )
    .unwrap();
    let current = income(&bank, "7", "staking");
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        booked(current, "staking", "2026-10-02T10:00:00Z"),
    )
    .unwrap();
    let cashback = income(&bank, "50", "cashback");
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        booked(cashback, "cashback", "2026-09-12T10:00:00Z"),
    )
    .unwrap();

    let now = unix_secs("2026-10-15T12:00:00Z").unwrap();
    let report =
        crate::planning::passive::passive_income(&vault.books(), now, "UTC", Some(amount("12")))
            .unwrap();
    assert_eq!(report.months.len(), 12);
    assert_eq!(report.months[0].month, "2025-10");
    assert_eq!(report.months[11].month, "2026-09");
    assert_eq!(report.total_12m, amount("120"));
    assert_eq!(report.this_month, amount("7"));
    assert_eq!(report.reinvested_12m, amount("60"));
    assert_eq!(report.by_category.len(), 2);
    assert_eq!(report.e_mand, Some(amount("1000")));
    // 120 / 12 = 10 a month against 1000.
    assert_eq!(report.freedom_fact_pct, Some(amount("1")));

    // The broker's 60 at 12% a year: 0.6 a month against 1000 = 0.06%.
    assert_eq!(report.invested, amount("60"));
    assert_eq!(report.freedom_potential_pct, Some(amount("0.1")));

    // An emergency fund account stays out of the potential.
    vault.doc.emergency_fund = EmergencyFund {
        scope: StoreScope {
            store_ids: vec![broker.clone()],
            include_children: false,
        },
        ..EmergencyFund::default()
    };
    let report =
        crate::planning::passive::passive_income(&vault.books(), now, "UTC", Some(amount("12")))
            .unwrap();
    assert_eq!(report.invested, Amount::ZERO);
    assert_eq!(report.total_12m, amount("120"), "its income still counts");
}

#[test]
fn a_bare_document_knows_which_category_books_passive_income() {
    let doc = crate::model::vault::VaultDoc::default();
    assert!(crate::planning::passive::is_passive_category(&doc, "earn"));
    assert!(!crate::planning::passive::is_passive_category(&doc, "food"));
}
