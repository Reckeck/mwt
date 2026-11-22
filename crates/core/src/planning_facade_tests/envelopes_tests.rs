//! Tests of the envelopes area: the old inline tests (names and bodies unchanged),
//! then the pure functions on a bare document and the delegate and one-transact pins.

use crate::Amount;
use crate::error::Error;
use crate::model::ids::StoreId;
use crate::model::store::{AssetRef, StoreKind};
use crate::model::vault::VaultDoc;
use crate::planning::envelopes::*;
use crate::service::UnlockedVault;
use crate::service::test_support::{amount, empty_vault, income, payment, store, usd};
use crate::usecases::household::envelopes as scenarios;

fn draft(store_id: &str, name: &str) -> EnvelopeDraft {
    EnvelopeDraft {
        store_id: store_id.into(),
        name: name.into(),
        asset: usd(),
        target: Some(amount("100")),
        goal_id: None,
    }
}

#[test]
fn envelopes_never_earmark_more_than_the_store_has() {
    let (_dir, mut vault) = empty_vault();
    let cash = store(&mut vault, "Cash", StoreKind::Cash, None);
    vault.add_transaction(income(&cash, "100", "pay")).unwrap();
    let trip = vault.create_envelope(draft(&cash, "Trip")).unwrap();
    let tax = vault.create_envelope(draft(&cash, "Tax")).unwrap();

    vault.fund_envelope(&trip.id, amount("70")).unwrap();
    assert_eq!(vault.free_balance(&cash, &usd()), amount("30"));
    assert!(vault.fund_envelope(&tax.id, amount("40")).is_err());
    vault
        .move_between_envelopes(&trip.id, &tax.id, amount("20"))
        .unwrap();
    assert_eq!(vault.earmarked(&cash, &usd().key()), amount("70"));
    assert!(vault.fund_envelope(&tax.id, amount("-30")).is_err());

    // spending into the envelopes is allowed but reported
    vault.add_transaction(payment(&cash, "50", "food")).unwrap();
    let views = vault.envelopes(Some(&cash));
    assert!(views.iter().all(|view| view.short));
    assert_eq!(views[0].free, amount("-20"));

    vault.delete_envelope(&trip.id).unwrap();
    assert_eq!(vault.free_balance(&cash, &usd()), amount("30"));
}

#[test]
fn a_bound_store_only_takes_envelopes_of_its_asset() {
    let (_dir, mut vault) = empty_vault();
    let bank = store(&mut vault, "Bank", StoreKind::Bank, None);
    let card = store(&mut vault, "Card", StoreKind::Bank, Some(&bank));
    let mut eur = draft(&card, "Euro");
    eur.asset = AssetRef::fiat("EUR", "Euro");
    assert!(vault.create_envelope(eur).is_err());
    vault.create_envelope(draft(&card, "Usd")).unwrap();
}

#[test]
fn funding_a_goal_takes_only_what_is_free() {
    use crate::model::store::StoreScope;
    use crate::service::GoalDraft;
    let (_dir, mut vault) = empty_vault();
    let cash = store(&mut vault, "Cash", StoreKind::Cash, None);
    vault.add_transaction(income(&cash, "100", "pay")).unwrap();
    let goal = vault
        .create_goal(GoalDraft {
            name: "Cushion".into(),
            active: true,
            target_amount: amount("500"),
            currency: "usd".into(),
            starts_at: "2026-01-01T00:00:00Z".into(),
            deadline: None,
            priority: 3,
            monthly_commitment: None,
            scope: StoreScope {
                store_ids: vec![cash.clone()],
                include_children: true,
            },
            scenario_id: None,
            expected_return: None,
        })
        .unwrap();

    assert_eq!(
        vault.fund_goal(&goal.id, &cash, amount("60")).unwrap(),
        amount("60")
    );
    assert_eq!(
        vault.fund_goal(&goal.id, &cash, amount("60")).unwrap(),
        amount("40")
    );
    assert_eq!(vault.envelopes(Some(&cash)).len(), 1);
    assert_eq!(vault.earmarked(&cash, &usd().key()), amount("100"));
    assert_eq!(
        vault.fund_goal(&goal.id, &cash, amount("10")).unwrap(),
        Amount::ZERO
    );
}

#[test]
fn allocated_is_the_sum_of_moves() {
    use crate::model::store::StoreScope;
    use crate::service::GoalDraft;
    let (_dir, mut vault) = empty_vault();
    let cash = store(&mut vault, "Cash", StoreKind::Cash, None);
    vault.add_transaction(income(&cash, "500", "pay")).unwrap();
    let check = |vault: &UnlockedVault| {
        for envelope in &vault.doc.envelopes {
            assert_eq!(
                envelope.allocated,
                envelope.moves_sum(),
                "{}",
                envelope.name
            );
        }
    };
    let trip = vault.create_envelope(draft(&cash, "Trip")).unwrap();
    let tax = vault.create_envelope(draft(&cash, "Tax")).unwrap();
    assert!(trip.moves.is_empty(), "a new envelope holds nothing");
    check(&vault);
    // fund, release, move, update, goal
    let funded = vault.fund_envelope(&trip.id, amount("70")).unwrap();
    assert_eq!(funded.moves.len(), 1);
    check(&vault);
    vault.fund_envelope(&trip.id, amount("-10")).unwrap();
    check(&vault);
    vault
        .move_between_envelopes(&trip.id, &tax.id, amount("20"))
        .unwrap();
    check(&vault);
    assert_eq!(vault.doc.envelopes[0].allocated, amount("40"));
    assert_eq!(vault.doc.envelopes[1].allocated, amount("20"));
    vault
        .update_envelope(&trip.id, draft(&cash, "Trip 2"))
        .unwrap();
    check(&vault);
    let goal = vault
        .create_goal(GoalDraft {
            name: "Cushion".into(),
            active: true,
            target_amount: amount("500"),
            currency: "usd".into(),
            starts_at: "2026-01-01T00:00:00Z".into(),
            deadline: None,
            priority: 3,
            monthly_commitment: None,
            scope: StoreScope {
                store_ids: vec![cash.clone()],
                include_children: true,
            },
            scenario_id: None,
            expected_return: None,
        })
        .unwrap();
    vault.fund_goal(&goal.id, &cash, amount("100")).unwrap();
    vault.fund_goal(&goal.id, &cash, amount("50")).unwrap();
    check(&vault);
    // a zero change leaves no entry
    let before = vault.doc.envelopes[0].moves.len();
    vault.fund_envelope(&trip.id, Amount::ZERO).unwrap();
    assert_eq!(vault.doc.envelopes[0].moves.len(), before);
    // an envelope from before the journal gets its opening entry
    vault.doc.envelopes[1].moves.clear();
    assert!(vault.settle_envelopes());
    assert_eq!(
        vault.doc.envelopes[1].moves[0].id,
        format!("open:{}", tax.id)
    );
    assert!(!vault.settle_envelopes());
    check(&vault);
}

fn funded_cash() -> (tempfile::TempDir, UnlockedVault, String) {
    let (dir, mut vault) = empty_vault();
    let cash = store(&mut vault, "Cash", StoreKind::Cash, None);
    vault.add_transaction(income(&cash, "100", "pay")).unwrap();
    (dir, vault, cash)
}

#[test]
fn free_money_and_views_are_read_from_a_bare_document() {
    let (_dir, mut vault, cash) = funded_cash();
    let trip = vault.create_envelope(draft(&cash, "Trip")).unwrap();
    vault.fund_envelope(&trip.id, amount("70")).unwrap();
    let doc = vault.doc.clone();
    let id = StoreId::from(&cash);
    assert_eq!(earmarked(&doc, &id, &usd().key()), amount("70"));
    assert_eq!(free_balance(&doc, &id, &usd()), amount("30"));
    let views = envelopes(&doc, Some(id));
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].pct, Some(70.0));
    assert!(!views[0].short);
    assert!(envelopes(&VaultDoc::default(), None).is_empty());
}

#[test]
fn the_deciders_refuse_without_touching_the_document() {
    let (_dir, mut vault, cash) = funded_cash();
    let trip = vault.create_envelope(draft(&cash, "Trip")).unwrap();
    let tax = vault.create_envelope(draft(&cash, "Tax")).unwrap();
    let before = vault.doc.clone();
    assert!(matches!(
        plan_fund_envelope(&vault.doc, "nope", amount("1")),
        Err(Error::Missing(_))
    ));
    assert!(matches!(
        plan_fund_envelope(&vault.doc, &trip.id, amount("101")),
        Err(Error::InsufficientFunds(_))
    ));
    assert!(plan_fund_envelope(&vault.doc, &trip.id, amount("-1")).is_err());
    assert!(plan_move_between_envelopes(&vault.doc, &trip.id, &tax.id, amount("0")).is_err());
    assert!(plan_move_between_envelopes(&vault.doc, &trip.id, &tax.id, amount("1")).is_err());
    assert!(check_draft_name(&draft(&cash, "  ")).is_err());
    let mut bad_target = draft(&cash, "Trip");
    bad_target.target = Some(amount("0"));
    let live = vault.find_store(&cash).unwrap().clone();
    assert!(check_draft_against(&vault.doc, &live, &bad_target).is_err());
    assert_eq!(vault.doc, before);
}

#[test]
fn an_envelope_with_money_keeps_its_store_and_asset() {
    let (_dir, mut vault, cash) = funded_cash();
    let other = store(&mut vault, "Other", StoreKind::Cash, Some(&cash));
    let trip = vault.create_envelope(draft(&cash, "Trip")).unwrap();
    assert!(plan_update_envelope(&vault.doc, &trip.id, draft(&other, "Trip")).is_ok());
    vault.fund_envelope(&trip.id, amount("10")).unwrap();
    assert!(plan_update_envelope(&vault.doc, &trip.id, draft(&other, "Trip")).is_err());
    let renamed = plan_update_envelope(&vault.doc, &trip.id, draft(&cash, " Trip 2 ")).unwrap();
    assert_eq!(renamed.name, "Trip 2");
    assert_eq!(renamed.allocated, amount("10"));
}

#[test]
fn a_goal_funding_is_decided_before_anything_is_made() {
    use crate::model::store::StoreScope;
    use crate::service::GoalDraft;
    let (_dir, mut vault, cash) = funded_cash();
    let goal = vault
        .create_goal(GoalDraft {
            name: "Cushion".into(),
            active: true,
            target_amount: amount("500"),
            currency: "usd".into(),
            starts_at: "2026-01-01T00:00:00Z".into(),
            deadline: None,
            priority: 3,
            monthly_commitment: None,
            scope: StoreScope {
                store_ids: vec![cash.clone()],
                include_children: true,
            },
            scenario_id: None,
            expected_return: None,
        })
        .unwrap();
    let id = StoreId::from(&cash);
    let funding = plan_fund_goal(&vault.doc, &goal.id, &id, amount("60")).unwrap();
    assert_eq!(funding.take, amount("60"));
    assert_eq!(funding.envelope_id, None);
    assert_eq!(
        plan_fund_goal(&vault.doc, &goal.id, &id, amount("500"))
            .unwrap()
            .take,
        amount("100")
    );
    assert!(plan_fund_goal(&vault.doc, &goal.id, &id, amount("0")).is_err());
    assert!(plan_fund_goal(&vault.doc, "nope", &id, amount("1")).is_err());
    assert!(vault.doc.envelopes.is_empty());
    scenarios::fund_goal(&mut vault, &goal.id, &id, amount("60")).unwrap();
    let again = plan_fund_goal(&vault.doc, &goal.id, &id, amount("10")).unwrap();
    assert_eq!(again.envelope_id, Some(vault.doc.envelopes[0].id.clone()));
}

#[test]
fn a_refused_write_leaves_the_envelopes_untouched() {
    let (_dir, mut vault, cash) = funded_cash();
    let trip = vault.create_envelope(draft(&cash, "Trip")).unwrap();
    let before = vault.doc.clone();
    assert!(vault.fund_envelope(&trip.id, amount("101")).is_err());
    assert!(
        vault
            .move_between_envelopes(&trip.id, "nope", amount("1"))
            .is_err()
    );
    assert!(vault.create_envelope(draft(&cash, " ")).is_err());
    assert!(vault.delete_envelope("nope").is_err());
    assert_eq!(vault.doc, before);
}

#[test]
fn the_delegates_equal_the_scenarios() {
    let (_first_dir, mut delegated, first) = funded_cash();
    let (_second_dir, mut direct, second) = funded_cash();
    let by_delegate = delegated.create_envelope(draft(&first, "Trip")).unwrap();
    let by_scenario = scenarios::create_envelope(&mut direct, draft(&second, "Trip")).unwrap();
    let other_delegate = delegated.create_envelope(draft(&first, "Tax")).unwrap();
    let other_scenario = scenarios::create_envelope(&mut direct, draft(&second, "Tax")).unwrap();
    let funded = delegated
        .fund_envelope(&by_delegate.id, amount("70"))
        .unwrap();
    let funded_directly =
        scenarios::fund_envelope(&mut direct, &by_scenario.id, amount("70")).unwrap();
    assert_eq!(funded.allocated, funded_directly.allocated);
    delegated
        .move_between_envelopes(&by_delegate.id, &other_delegate.id, amount("20"))
        .unwrap();
    scenarios::move_between_envelopes(
        &mut direct,
        &by_scenario.id,
        &other_scenario.id,
        amount("20"),
    )
    .unwrap();
    let held = |vault: &UnlockedVault| {
        vault
            .doc
            .envelopes
            .iter()
            .map(|envelope| {
                (
                    envelope.name.clone(),
                    envelope.allocated,
                    envelope.moves.len(),
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(held(&delegated), held(&direct));
    assert_eq!(delegated.free_balance(&first, &usd()), amount("30"));
    assert_eq!(
        delegated.free_balance(&first, &usd()),
        direct.free_balance(&second, &usd())
    );
    assert_eq!(delegated.envelopes(Some(&first)).len(), 2);
    assert!(
        delegated
            .envelope_saved_for_goal("none", "usd", 0)
            .is_none()
    );
    assert!(!scenarios::settle_envelopes(&mut direct));
}
