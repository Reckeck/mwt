//! Tests of the emergency area: the old inline tests (names and bodies unchanged).

use crate::model::category::CategoryGroup;
use crate::model::deposit::{Capitalization, DepositTerms, RatePeriod};
use crate::model::planning::EmergencyFund;
use crate::model::store::{Binding, Purpose, StoreKind, StoreScope, SubType};
use crate::model::transaction::{AccountRef, Transaction};
use crate::pricing::history::unix_secs;
use crate::service::StoreExtras;
use crate::service::UnlockedVault;
use crate::service::interest::INTEREST_CATEGORY;
use crate::service::test_support::*;

fn savings(vault: &mut UnlockedVault, name: &str) -> String {
    let bank = store(vault, &format!("{name} bank"), StoreKind::Bank, None);
    let extras = StoreExtras {
        sub_type: Some(SubType::Savings),
        binding: Some(Binding::Asset(usd())),
        ..StoreExtras::default()
    };
    vault
        .create_store_with(name, StoreKind::Bank, "#000", "", Some(bank), extras)
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

    vault
        .update_category("groceries", None, CategoryGroup::Daily, Some(false), None)
        .unwrap();
    vault
        .update_category(
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
    vault
        .update_category(
            "entertainment",
            Some("Fun"),
            CategoryGroup::Daily,
            None,
            None,
        )
        .unwrap();
    assert!(find(&vault, "entertainment").is_essential());
    // a mandatory category is mandatory whatever the flag says
    vault
        .update_category("utilities", None, CategoryGroup::Fixed, Some(false), None)
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
            vault
                .add_transaction(spend(&card, raw, category, &ts))
                .unwrap();
        }
    }
    let summary = vault.month_summary("2026-01", "UTC").unwrap();
    assert_eq!(summary.fixed, amount("500"));
    assert_eq!(summary.daily, amount("500"));
    assert_eq!(summary.essential_daily, amount("300"));

    let now = unix_secs("2026-03-05T10:00:00Z").unwrap();
    let avg = vault.avg_month(6, now, "UTC").unwrap();
    assert_eq!(avg.essential, amount("800"));
}

#[test]
fn the_fund_takes_only_live_savings_accounts() {
    let (_dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    let reserve = savings(&mut vault, "Reserve");
    assert!(vault.set_emergency_fund(fund_of(&[&card])).is_err());
    let mut long = fund_of(&[&reserve]);
    long.target_months = 25;
    assert!(vault.set_emergency_fund(long).is_err());
    let mut zero = fund_of(&[&reserve]);
    zero.avg_months = 0;
    assert!(vault.set_emergency_fund(zero).is_err());

    let saved = vault
        .set_emergency_fund(fund_of(&[&reserve, &reserve]))
        .unwrap();
    assert_eq!(saved.scope.store_ids, vec![reserve.clone()]);
    assert!(!saved.scope.include_children);
    assert!(vault.emergency_fund_toggle(&card, true).is_err());
}

#[test]
fn switching_off_the_last_account_leaves_no_fund() {
    let (_dir, mut vault) = empty_vault();
    let first = savings(&mut vault, "First");
    let second = savings(&mut vault, "Second");
    vault.emergency_fund_toggle(&first, true).unwrap();
    let both = vault.emergency_fund_toggle(&second, true).unwrap();
    assert_eq!(both.scope.store_ids, vec![first.clone(), second.clone()]);
    vault.emergency_fund_toggle(&first, false).unwrap();
    let none = vault.emergency_fund_toggle(&second, false).unwrap();
    assert!(!none.is_set());
    assert_eq!(none.target_months, 6);
    // an empty fund is not «every store»
    let status = vault.emergency_fund_status(0, "UTC");
    assert!(status.stores.is_empty());
    assert_eq!(status.months_covered, None);
}

#[test]
fn a_new_purpose_or_the_archive_takes_the_account_out() {
    let (_dir, mut vault) = empty_vault();
    let first = savings(&mut vault, "First");
    let second = savings(&mut vault, "Second");
    vault
        .set_emergency_fund(fund_of(&[&first, &second]))
        .unwrap();
    vault
        .update_store(
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
    assert_eq!(vault.emergency_fund().scope.store_ids, vec![second.clone()]);
    vault.archive_store(&second, true).unwrap();
    assert!(!vault.emergency_fund().is_set());
}

#[test]
fn coverage_surplus_interest_and_goal_overlap() {
    let (_dir, mut vault) = empty_vault();
    vault.set_base_currency("usd");
    quote_usd(&mut vault);
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    let reserve = savings(&mut vault, "Reserve");
    let spare = savings(&mut vault, "Spare");
    vault
        .set_deposit_terms(
            &reserve,
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
        vault
            .add_transaction(spend(&card, "1000", "rent_mortgage", &ts))
            .unwrap();
    }
    vault
        .add_transaction(dated(
            transfer_between(
                AccountRef::Store(card.clone()),
                AccountRef::Store(reserve.clone()),
                usd(),
                "7000",
            ),
            "2026-03-20T10:00:00Z",
        ))
        .unwrap();
    let mut interest = income(&reserve, "80", "interest");
    interest.meta_mut().categories = vec![INTEREST_CATEGORY.into()];
    vault
        .add_transaction(dated(interest, "2026-03-31T10:00:00Z"))
        .unwrap();
    vault.emergency_fund_toggle(&reserve, true).unwrap();
    let goal = vault
        .create_goal(crate::service::GoalDraft {
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
        })
        .unwrap();

    let now = unix_secs("2026-04-05T10:00:00Z").unwrap();
    let status = vault.emergency_fund_status(now, "UTC");
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
