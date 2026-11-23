//! Tests of the emergency area: the old inline tests (names and bodies unchanged).

use crate::model::category::CategoryGroup;
use crate::model::deposit::{Capitalization, DepositTerms, RatePeriod};
use crate::model::ids::StoreId;
use crate::model::planning::EmergencyFund;
use crate::model::store::{Binding, Purpose, StoreKind, StoreScope, SubType};
use crate::model::transaction::{AccountRef, Transaction};
use crate::money::interest::INTEREST_CATEGORY;
use crate::service::StoreExtras;
use crate::service::UnlockedVault;
use crate::service::test_support::*;
use crate::time::unix_secs;

fn savings(vault: &mut UnlockedVault, name: &str) -> String {
    let bank = store(vault, &format!("{name} bank"), StoreKind::Bank, None);
    let extras = StoreExtras {
        sub_type: Some(SubType::Savings),
        binding: Some(Binding::Asset(usd())),
        ..StoreExtras::default()
    };
    crate::usecases::money::stores::create_store_with(
        vault,
        name,
        StoreKind::Bank,
        "#000",
        "",
        Some(bank),
        extras,
    )
    .unwrap()
    .id
}

fn dated(mut tx: Transaction, ts: &str) -> Transaction {
    at(&mut tx, ts);
    tx
}

fn spend(store: &str, raw: &str, category: &str, ts: &str) -> Transaction {
    let mut tx = payment(store, raw, category);
    tx.meta_mut().categories = vec![category.into()];
    dated(tx, ts)
}

fn fund_of(ids: &[&String]) -> EmergencyFund {
    EmergencyFund {
        scope: StoreScope {
            store_ids: ids.iter().map(|id| id.to_string()).collect(),
            include_children: false,
        },
        ..EmergencyFund::default()
    }
}

#[test]
fn essential_follows_the_defaults_until_the_user_says_otherwise() {
    let (_dir, mut vault) = empty_vault();
    let find = |vault: &UnlockedVault, key: &str| {
        vault
            .doc
            .categories
            .iter()
            .find(|category| category.key == key)
            .unwrap()
            .clone()
    };
    assert!(find(&vault, "groceries").is_essential());
    assert!(!find(&vault, "entertainment").is_essential());
    assert!(find(&vault, "utilities").is_essential());
    assert!(!find(&vault, "salary").is_essential());

    crate::usecases::household::categories::update_category(
        &mut vault,
        "groceries",
        None,
        CategoryGroup::Daily,
        Some(false),
        None,
    )
    .unwrap();
    crate::usecases::household::categories::update_category(
        &mut vault,
        "entertainment",
        None,
        CategoryGroup::Daily,
        Some(true),
        None,
    )
    .unwrap();
    assert!(!find(&vault, "groceries").is_essential());
    assert!(find(&vault, "entertainment").is_essential());
    // a rename without the flag keeps the choice
    crate::usecases::household::categories::update_category(
        &mut vault,
        "entertainment",
        Some("Fun"),
        CategoryGroup::Daily,
        None,
        None,
    )
    .unwrap();
    assert!(find(&vault, "entertainment").is_essential());
    // a mandatory category is mandatory whatever the flag says
    crate::usecases::household::categories::update_category(
        &mut vault,
        "utilities",
        None,
        CategoryGroup::Fixed,
        Some(false),
        None,
    )
    .unwrap();
    assert!(find(&vault, "utilities").is_essential());
}

#[test]
fn mandatory_spending_is_fixed_plus_essential_everyday() {
    let (_dir, mut vault) = empty_vault();
    vault.set_base_currency("usd");
    quote_usd(&mut vault);
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    for month in ["01", "02"] {
        let ts = format!("2026-{month}-10T10:00:00Z");
        for (raw, category) in [
            ("500", "rent_mortgage"),
            ("300", "groceries"),
            ("200", "entertainment"),
        ] {
            crate::usecases::money::transactions::add_transaction(
                &mut vault,
                spend(&card, raw, category, &ts),
            )
            .unwrap();
        }
    }
    let summary =
        crate::usecases::planning::budget::month_summary(&vault.books(), "2026-01", "UTC").unwrap();
    assert_eq!(summary.fixed, amount("500"));
    assert_eq!(summary.daily, amount("500"));
    assert_eq!(summary.essential_daily, amount("300"));

    let now = unix_secs("2026-03-05T10:00:00Z").unwrap();
    let avg = crate::planning::budget::avg_month(&vault.books(), 6, now, "UTC").unwrap();
    assert_eq!(avg.essential, amount("800"));
}

#[test]
fn the_fund_takes_only_live_savings_accounts() {
    let (_dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    let reserve = savings(&mut vault, "Reserve");
    assert!(
        crate::usecases::planning::emergency::set_emergency_fund(&mut vault, fund_of(&[&card]))
            .is_err()
    );
    let mut long = fund_of(&[&reserve]);
    long.target_months = 25;
    assert!(crate::usecases::planning::emergency::set_emergency_fund(&mut vault, long).is_err());
    let mut zero = fund_of(&[&reserve]);
    zero.avg_months = 0;
    assert!(crate::usecases::planning::emergency::set_emergency_fund(&mut vault, zero).is_err());

    let saved = crate::usecases::planning::emergency::set_emergency_fund(
        &mut vault,
        fund_of(&[&reserve, &reserve]),
    )
    .unwrap();
    assert_eq!(saved.scope.store_ids, vec![reserve.clone()]);
    assert!(!saved.scope.include_children);
    assert!(
        crate::usecases::planning::emergency::emergency_fund_toggle(&mut vault, &card, true)
            .is_err()
    );
}

#[test]
fn switching_off_the_last_account_leaves_no_fund() {
    let (_dir, mut vault) = empty_vault();
    let first = savings(&mut vault, "First");
    let second = savings(&mut vault, "Second");
    crate::usecases::planning::emergency::emergency_fund_toggle(&mut vault, &first, true).unwrap();
    let both =
        crate::usecases::planning::emergency::emergency_fund_toggle(&mut vault, &second, true)
            .unwrap();
    assert_eq!(both.scope.store_ids, vec![first.clone(), second.clone()]);
    crate::usecases::planning::emergency::emergency_fund_toggle(&mut vault, &first, false).unwrap();
    let none =
        crate::usecases::planning::emergency::emergency_fund_toggle(&mut vault, &second, false)
            .unwrap();
    assert!(!none.is_set());
    assert_eq!(none.target_months, 6);
    // an empty fund is not «every store»
    let status = crate::planning::emergency::emergency_fund_status(&vault.books(), 0, "UTC");
    assert!(status.stores.is_empty());
    assert_eq!(status.months_covered, None);
}

#[test]
fn a_new_purpose_or_the_archive_takes_the_account_out() {
    let (_dir, mut vault) = empty_vault();
    let first = savings(&mut vault, "First");
    let second = savings(&mut vault, "Second");
    crate::usecases::planning::emergency::set_emergency_fund(
        &mut vault,
        fund_of(&[&first, &second]),
    )
    .unwrap();
    crate::usecases::money::stores::update_store(
        &mut vault,
        &first,
        "First",
        "#000",
        "",
        StoreExtras {
            purpose: Some(Purpose::Everyday),
            ..StoreExtras::default()
        },
    )
    .unwrap();
    assert_eq!(
        crate::planning::emergency::emergency_fund(&vault.doc)
            .scope
            .store_ids,
        vec![second.clone()]
    );
    crate::usecases::money::stores::archive_store(&mut vault, &second, true).unwrap();
    assert!(!crate::planning::emergency::emergency_fund(&vault.doc).is_set());
}

#[test]
fn coverage_surplus_interest_and_goal_overlap() {
    let (_dir, mut vault) = empty_vault();
    vault.set_base_currency("usd");
    quote_usd(&mut vault);
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    let reserve = savings(&mut vault, "Reserve");
    let spare = savings(&mut vault, "Spare");
    crate::usecases::money::interest::set_deposit_terms(
        &mut vault,
        &StoreId::from(&reserve),
        Some(DepositTerms {
            rates: vec![RatePeriod {
                from: "2025-01-01".into(),
                rate: amount("2"),
            }],
            capitalization: Capitalization::Monthly,
            opened_at: None,
            term_months: None,
            auto_accrue: false,
            pds: None,
        }),
    )
    .unwrap();
    for month in ["01", "02", "03"] {
        let ts = format!("2026-{month}-10T10:00:00Z");
        crate::usecases::money::transactions::add_transaction(
            &mut vault,
            spend(&card, "1000", "rent_mortgage", &ts),
        )
        .unwrap();
    }
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        dated(
            transfer_between(
                AccountRef::Store(card.clone()),
                AccountRef::Store(reserve.clone()),
                usd(),
                "7000",
            ),
            "2026-03-20T10:00:00Z",
        ),
    )
    .unwrap();
    let mut interest = income(&reserve, "80", "interest");
    interest.meta_mut().categories = vec![INTEREST_CATEGORY.into()];
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        dated(interest, "2026-03-31T10:00:00Z"),
    )
    .unwrap();
    crate::usecases::planning::emergency::emergency_fund_toggle(&mut vault, &reserve, true)
        .unwrap();
    let goal = crate::usecases::planning::goals::create_goal(
        &mut vault,
        crate::service::GoalDraft {
            name: "Trip".into(),
            active: true,
            target_amount: amount("10000"),
            currency: "usd".into(),
            starts_at: "2026-01-01T00:00:00Z".into(),
            deadline: None,
            priority: 3,
            monthly_commitment: None,
            scope: StoreScope::store(&reserve, false),
            scenario_id: None,
            expected_return: None,
        },
    )
    .unwrap();

    let now = unix_secs("2026-04-05T10:00:00Z").unwrap();
    let status = crate::planning::emergency::emergency_fund_status(&vault.books(), now, "UTC");
    assert_eq!(status.total, amount("7080"));
    assert_eq!(status.e_mand, Some(amount("1000")));
    assert_eq!(status.months_covered, Some(amount("7.1")));
    assert_eq!(status.target_amount, Some(amount("6000")));
    assert_eq!(status.surplus, Some(amount("1080")));
    assert_eq!(status.interest_12m, amount("80"));
    assert_eq!(status.candidates, vec![spare]);
    let card_row = &status.stores[0];
    assert_eq!(card_row.rate, Some(amount("2")));
    assert!(card_row.below_inflation);
    assert_eq!(card_row.goal_ids, vec![goal.id]);
}
