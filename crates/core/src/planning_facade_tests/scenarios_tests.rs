//! Tests of the saved-calculations area: the old inline tests (names and bodies unchanged).

use crate::Amount;
use crate::calc::invest;
use crate::calc::invest::{CapFreq, ContributionStage, Freq, InvestInput};
use crate::calc::invest::{TaxRule, Timing};
use crate::model::deposit::Capitalization;
use crate::model::deposit::{DepositTerms, RatePeriod};
use crate::model::planning::ScenarioSource;
use crate::model::store::StoreScope;
use crate::model::store::{Binding, StoreKind, SubType};
use crate::model::transaction::{AccountRef, Transaction};
use crate::planning::scenarios::*;
use crate::service::UnlockedVault;
use crate::service::test_support::*;
use crate::service::{EntityKind, EntityRef, RelationKind, StoreExtras};
use crate::time::unix_secs;

pub(super) fn input(initial: &str, monthly: &str, months: u32) -> InvestInput {
    InvestInput {
        currency: "usd".into(),
        start: "2026-01".into(),
        initial: amount(initial),
        rate_pct: amount("12"),
        term_months: months,
        capitalization: CapFreq::Monthly,
        stages: vec![ContributionStage {
            from_month: 1,
            amount: amount(monthly),
            freq: Freq::Monthly,
            timing: Timing::End,
        }],
        indexation_pct: Amount::ZERO,
        inflation_pct: Amount::ZERO,
        tax: TaxRule::default(),
        withdrawal: None,
    }
}

fn draft(name: &str) -> CalcScenarioDraft {
    CalcScenarioDraft {
        id: None,
        name: name.into(),
        input: input("1000", "100", 12),
        source: None,
        prefilled_at: None,
    }
}

fn deposit_store(vault: &mut UnlockedVault, name: &str) -> String {
    let bank = store(vault, &format!("{name} bank"), StoreKind::Bank, None);
    let extras = StoreExtras {
        sub_type: Some(SubType::Deposit),
        binding: Some(Binding::Asset(usd())),
        ..StoreExtras::default()
    };
    vault
        .create_store_with(name, StoreKind::Bank, "#000", "", Some(bank), extras)
        .unwrap()
        .id
}

fn usd_transfer(from: &str, to: &str, raw: &str) -> Transaction {
    let from = AccountRef::Store(from.into());
    transfer_between(from, AccountRef::Store(to.into()), usd(), raw)
}

fn dated(mut tx: Transaction, ts: &str) -> Transaction {
    at(&mut tx, ts);
    tx
}

#[test]
fn scenarios_are_saved_updated_and_checked() {
    let (_dir, mut vault) = empty_vault();
    assert!(vault.save_calc_scenario(draft("  ")).is_err());
    let mut broken = draft("Broken");
    broken.input.initial = amount("-1");
    assert!(vault.save_calc_scenario(broken).is_err());
    let mut odd = draft("Odd period");
    odd.source = Some(ScenarioSource {
        scope: StoreScope::default(),
        income_months: 5,
        income_pct: amount("10"),
    });
    assert!(vault.save_calc_scenario(odd).is_err());

    let saved = vault.save_calc_scenario(draft(" Pension ")).unwrap();
    assert_eq!(saved.name, "Pension");
    let mut edit = draft("Pension 2");
    edit.id = Some(saved.id.clone());
    let updated = vault.save_calc_scenario(edit).unwrap();
    assert_eq!(updated.id, saved.id);
    assert_eq!(updated.created_at, saved.created_at);
    assert_eq!(vault.calc_scenarios().len(), 1);
    assert_eq!(vault.calc_scenarios()[0].name, "Pension 2");

    let mut missing = draft("Missing");
    missing.id = Some("nope".into());
    assert!(vault.save_calc_scenario(missing).is_err());
}

#[test]
fn deleting_a_scenario_unlinks_its_goals_and_undo_links_them_back() {
    let (_dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    let scenario = vault.save_calc_scenario(draft("Flat")).unwrap();
    let mut goal = vault
        .goal_from_scenario(
            &scenario.id,
            unix_secs("2026-03-10T10:00:00Z").unwrap(),
            "UTC",
        )
        .unwrap();
    goal.scope = StoreScope {
        store_ids: vec![card],
        include_children: false,
    };
    let goal = vault.create_goal(goal.into()).unwrap();
    assert_eq!(goal.scenario_id.as_deref(), Some(scenario.id.as_str()));

    let deleted = vault.delete_calc_scenario(&scenario.id).unwrap();
    assert_eq!(deleted.goal_ids, vec![goal.id.clone()]);
    assert!(vault.calc_scenarios().is_empty());
    assert_eq!(vault.goals()[0].scenario_id, None);
    assert!(vault.delete_calc_scenario(&scenario.id).is_err());

    vault.restore_calc_scenario(deleted.clone()).unwrap();
    assert_eq!(vault.calc_scenarios().len(), 1);
    assert_eq!(
        vault.goals()[0].scenario_id.as_deref(),
        Some(scenario.id.as_str())
    );
    assert!(vault.restore_calc_scenario(deleted).is_err());
}

#[test]
fn a_goal_with_an_unknown_scenario_is_refused() {
    let (_dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    let scenario = vault.save_calc_scenario(draft("Flat")).unwrap();
    let mut goal = vault
        .goal_from_scenario(
            &scenario.id,
            unix_secs("2026-03-10T10:00:00Z").unwrap(),
            "UTC",
        )
        .unwrap();
    goal.scope.store_ids = vec![card];
    goal.scenario_id = Some("gone".into());
    assert!(vault.create_goal(goal.into()).is_err());
}

#[test]
fn goal_from_scenario_aims_at_the_final_amount_over_the_term() {
    let (_dir, mut vault) = empty_vault();
    let mut quarterly = draft("Car");
    quarterly.input.stages[0].freq = Freq::Quarterly;
    quarterly.input.stages[0].amount = amount("300");
    let scenario = vault.save_calc_scenario(quarterly.clone()).unwrap();
    let goal = vault
        .goal_from_scenario(
            &scenario.id,
            unix_secs("2026-03-10T10:00:00Z").unwrap(),
            "UTC",
        )
        .unwrap();
    let expected = invest::simulate(&quarterly.input).unwrap().final_nominal;
    assert_eq!(goal.target_amount, expected);
    assert_eq!(goal.name, "Car");
    assert_eq!(goal.starts_at, "2026-03-10T00:00:00Z");
    assert_eq!(goal.deadline.as_deref(), Some("2027-03-10T00:00:00Z"));
    assert_eq!(goal.monthly_commitment, Some(amount("100")));
    assert_eq!(goal.scenario_id, Some(scenario.id));
    assert!(vault.goal_from_scenario("nope", 0, "UTC").is_err());
}

#[test]
fn prefill_takes_wallet_value_deposit_terms_and_a_share_of_income() {
    let (_dir, mut vault) = empty_vault();
    vault.set_base_currency("usd");
    quote_usd(&mut vault);
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    let deposit = deposit_store(&mut vault, "Deposit");
    let small = deposit_store(&mut vault, "Small deposit");
    for (target, rate, cap) in [
        (&deposit, "16", Capitalization::Daily),
        (&small, "20", Capitalization::None),
    ] {
        vault
            .set_deposit_terms(
                target,
                Some(DepositTerms {
                    rates: vec![RatePeriod {
                        from: "2026-01-01".into(),
                        rate: amount(rate),
                    }],
                    capitalization: cap,
                    opened_at: None,
                    term_months: None,
                    auto_accrue: false,
                    pds: None,
                }),
            )
            .unwrap();
    }
    for (month, sum) in [("01", "1000"), ("02", "2000"), ("03", "3000")] {
        let ts = format!("2026-{month}-05T10:00:00Z");
        vault
            .add_transaction(dated(income(&card, sum, "salary"), &ts))
            .unwrap();
    }
    vault
        .add_transaction(dated(
            usd_transfer(&card, &deposit, "1500"),
            "2026-03-06T10:00:00Z",
        ))
        .unwrap();
    vault
        .add_transaction(dated(
            usd_transfer(&card, &small, "500"),
            "2026-03-06T10:00:00Z",
        ))
        .unwrap();

    let now = unix_secs("2026-04-10T10:00:00Z").unwrap();
    let source = ScenarioSource {
        scope: StoreScope {
            store_ids: vec![deposit.clone(), small],
            include_children: false,
        },
        income_months: 3,
        income_pct: amount("10"),
    };
    let prefill = vault.scenario_prefill(&source, "usd", now, "UTC").unwrap();
    assert_eq!(prefill.initial, amount("2000"));
    // The larger deposit gives the rate.
    assert_eq!(prefill.rate_store_id, Some(deposit));
    assert_eq!(prefill.rate_pct, Some(amount("16")));
    assert_eq!(prefill.capitalization, Some(CapFreq::Daily));
    assert!(!prefill.pays_out);
    assert_eq!(prefill.income_avg, Some(amount("2000")));
    assert_eq!(prefill.income_months_used, 3);
    assert_eq!(prefill.monthly_contribution, Some(amount("200")));
    assert_eq!(prefill.prefilled_at, "2026-04-10T10:00:00Z");
}

#[test]
fn prefill_without_history_or_deposits_gives_only_the_value() {
    let (_dir, mut vault) = empty_vault();
    let source = ScenarioSource {
        scope: StoreScope::default(),
        income_months: 12,
        income_pct: amount("10"),
    };
    let prefill = vault
        .scenario_prefill(
            &source,
            "usd",
            unix_secs("2026-04-10T10:00:00Z").unwrap(),
            "UTC",
        )
        .unwrap();
    assert_eq!(prefill.initial, Amount::ZERO);
    assert_eq!(prefill.rate_pct, None);
    assert_eq!(prefill.monthly_contribution, None);
    assert_eq!(prefill.income_months_used, 0);
    let mut bad = source.clone();
    bad.income_pct = amount("101");
    assert!(vault.scenario_prefill(&bad, "usd", 0, "UTC").is_err());
    bad.income_pct = amount("10");
    bad.scope.store_ids = vec!["gone".into()];
    assert!(vault.scenario_prefill(&bad, "usd", 0, "UTC").is_err());
    let _ = &mut vault;
}

#[test]
fn a_goal_shows_what_its_scenario_planned_by_now() {
    let (_dir, mut vault) = empty_vault();
    quote_usd(&mut vault);
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    let scenario = vault.save_calc_scenario(draft("Flat")).unwrap();
    let mut goal = vault
        .goal_from_scenario(
            &scenario.id,
            unix_secs("2026-03-10T10:00:00Z").unwrap(),
            "UTC",
        )
        .unwrap();
    goal.scope.store_ids = vec![card];
    let goal = vault.create_goal(goal.into()).unwrap();
    let groups = vault.relations(&EntityRef::new(EntityKind::Scenario, &scenario.id));
    assert_eq!(groups[0].kind, RelationKind::Goals);
    assert_eq!(groups[0].items[0].target.id, goal.id);
    let back = vault.relations(&EntityRef::new(EntityKind::Goal, &goal.id));
    assert!(
        back.iter()
            .any(|group| group.kind == RelationKind::Scenarios)
    );
    let now = unix_secs("2026-05-20T10:00:00Z").unwrap();
    let progress = vault.goal_progress(now, "UTC");
    let expected = planned_balance(&scenario.input, 2).unwrap();
    assert_eq!(progress[0].planned_now, Some(expected.round(2)));
}

#[test]
fn planned_balance_follows_the_simulation_and_stops_at_the_end() {
    let plan = input("1000", "100", 3);
    let result = invest::simulate(&plan).unwrap();
    let months: Vec<_> = result.years.iter().flat_map(|year| &year.months).collect();
    assert_eq!(planned_balance(&plan, 0).unwrap(), amount("1000"));
    assert_eq!(planned_balance(&plan, 2).unwrap(), months[1].balance);
    assert_eq!(planned_balance(&plan, 40).unwrap(), result.final_nominal);
}

// -- the deciders on a bare document --

#[test]
fn a_bare_document_decides_save_delete_and_restore() {
    let mut doc = crate::model::vault::VaultDoc::default();
    let saved = plan_save_calc_scenario(&doc, draft(" Plan "), "s1".into(), "2026-01-01").unwrap();
    assert_eq!((saved.id.as_str(), saved.name.as_str()), ("s1", "Plan"));
    assert!(doc.calc_scenarios.is_empty(), "a decision writes nothing");
    assert!(plan_save_calc_scenario(&doc, draft(" "), "s2".into(), "now").is_err());
    doc.calc_scenarios.push(saved);
    let edit = CalcScenarioDraft {
        id: Some("s1".into()),
        ..draft("Renamed")
    };
    let edited = plan_save_calc_scenario(&doc, edit, "unused".into(), "2026-02-01").unwrap();
    assert_eq!(edited.name, "Renamed");
    assert_eq!(edited.updated_at, "2026-02-01");
    let deleted = plan_delete_calc_scenario(&doc, "s1").unwrap();
    assert!(deleted.goal_ids.is_empty());
    assert!(plan_restore_calc_scenario(&doc, &deleted).is_err());
    doc.calc_scenarios.clear();
    assert!(
        plan_restore_calc_scenario(&doc, &deleted)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn a_scenario_source_is_checked_on_a_bare_document() {
    let mut doc = crate::model::vault::VaultDoc::default();
    doc.stores.push(crate::model::store::Store {
        id: "wallet".into(),
        ..crate::model::store::Store::default()
    });
    let source = |months: u8, pct: &str, stores: &[&str]| ScenarioSource {
        scope: StoreScope {
            store_ids: stores.iter().map(|id| id.to_string()).collect(),
            include_children: false,
        },
        income_months: months,
        income_pct: amount(pct),
    };
    assert!(check_scenario_source(&doc, &source(6, "10", &["wallet"])).is_ok());
    assert!(check_scenario_source(&doc, &source(4, "10", &["wallet"])).is_err());
    assert!(check_scenario_source(&doc, &source(6, "101", &["wallet"])).is_err());
    assert!(check_scenario_source(&doc, &source(6, "-1", &["wallet"])).is_err());
    assert!(matches!(
        check_scenario_source(&doc, &source(6, "10", &["gone"])),
        Err(crate::Error::Missing(_))
    ));
}

#[test]
fn the_scenario_delegates_equal_the_scenarios() {
    use crate::usecases::planning::scenarios as case;
    let (_a, mut by_method) = empty_vault();
    let (_b, mut by_case) = empty_vault();
    let saved = by_method.save_calc_scenario(draft("Flat")).unwrap();
    let made = case::save_calc_scenario(&mut by_case, draft("Flat")).unwrap();
    assert_eq!(
        super::automations_tests::without_ids(&saved, &[saved.id.as_str()]),
        super::automations_tests::without_ids(&made, &[made.id.as_str()])
    );
    let gone = by_method.delete_calc_scenario(&saved.id).unwrap();
    let gone_by_case = case::delete_calc_scenario(&mut by_case, &made.id).unwrap();
    assert_eq!(gone.goal_ids, gone_by_case.goal_ids);
    assert_eq!(
        by_method.calc_scenarios().len(),
        by_case.calc_scenarios().len()
    );
    let back = by_method.restore_calc_scenario(gone).unwrap();
    let back_by_case = case::restore_calc_scenario(&mut by_case, gone_by_case).unwrap();
    assert_eq!(back, saved);
    assert_eq!(back_by_case, made);
    assert_eq!(
        by_method.calc_scenarios().len(),
        by_case.calc_scenarios().len()
    );
}
