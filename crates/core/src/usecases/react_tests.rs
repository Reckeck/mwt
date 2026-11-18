use std::collections::BTreeMap;

use crate::error::{Error, Result};
use crate::kernel::events::{
    AutomationDeleted, AutomationRetried, DomainEvent, GoalPaused, GoalResumed,
};
use crate::model::planning::{Schedule, TransactionDraft};
use crate::model::store::{StoreKind, StoreScope};
use crate::model::transaction::{PaymentTx, Transaction};
use crate::service::test_support::*;
use crate::service::{AutomationDraft, GoalDraft, UnlockedVault};

use super::react::{Reaction, react, run};

fn goal_on(store_id: &str) -> GoalDraft {
    GoalDraft {
        name: "Trip".into(),
        active: true,
        target_amount: amount("1000"),
        currency: "usd".into(),
        starts_at: "2026-01-01T00:00:00Z".into(),
        deadline: Some("2026-12-31T00:00:00Z".into()),
        priority: 3,
        monthly_commitment: None,
        scope: StoreScope {
            store_ids: vec![store_id.to_string()],
            include_children: true,
        },
        scenario_id: None,
        expected_return: None,
    }
}

/// A vault with a goal on a wallet and a workflow that saves into it.
fn saving_vault() -> (tempfile::TempDir, UnlockedVault, String, String) {
    let (dir, mut vault) = empty_vault();
    vault.set_base_currency("usd");
    let savings = store(&mut vault, "Savings", StoreKind::Bank, None);
    let goal = vault.create_goal(goal_on(&savings)).unwrap();
    let params: BTreeMap<String, String> = [
        ("category", "salary"),
        ("pct", "10"),
        ("target", savings.as_str()),
    ]
    .iter()
    .map(|(key, value)| (key.to_string(), value.to_string()))
    .collect();
    let saver = vault
        .workflow_from_template("pay_yourself_first", &params, "en", 1_767_312_000)
        .unwrap()
        .workflow;
    (dir, vault, goal.id, saver.id)
}

fn active(vault: &UnlockedVault, workflow_id: &str) -> bool {
    vault.workflow(workflow_id).unwrap().active
}

#[test]
fn a_paused_goal_stops_its_saver_and_a_resumed_one_starts_it_again() {
    let (_dir, mut vault, goal, saver) = saving_vault();
    let paused = react(&mut vault, vec![GoalPaused { id: goal.clone() }.into()]);
    assert!(paused.is_empty(), "{paused:?}");
    assert!(!active(&vault, &saver));
    let resumed = react(
        &mut vault,
        vec![
            GoalResumed {
                id: goal,
                workflow_ids: vec![saver.clone()],
            }
            .into(),
        ],
    );
    assert!(resumed.is_empty(), "{resumed:?}");
    assert!(active(&vault, &saver));
}

#[test]
fn a_saver_switched_off_by_hand_stays_off_after_pause_and_resume() {
    let (_dir, mut vault, goal, saver) = saving_vault();
    vault
        .set_workflow_active(&saver, false, 1_767_312_000)
        .unwrap();
    let paused = react(&mut vault, vec![GoalPaused { id: goal.clone() }.into()]);
    assert!(paused.is_empty(), "{paused:?}");
    // The pause stopped nothing, so the resume names nothing.
    let resumed = react(
        &mut vault,
        vec![
            GoalResumed {
                id: goal,
                workflow_ids: vec![],
            }
            .into(),
        ],
    );
    assert!(resumed.is_empty(), "{resumed:?}");
    assert!(!active(&vault, &saver));
}

#[test]
fn a_deleted_automation_removes_its_workflow() {
    let (_dir, mut vault, _goal, saver) = saving_vault();
    let failures = react(
        &mut vault,
        vec![AutomationDeleted { id: saver.clone() }.into()],
    );
    assert!(failures.is_empty(), "{failures:?}");
    assert!(vault.workflow(&saver).is_err());
}

#[test]
fn a_retry_of_an_unknown_run_is_reported() {
    let (_dir, mut vault) = empty_vault();
    let failures = react(
        &mut vault,
        vec![
            AutomationRetried {
                id: "run".into(),
                zone: "UTC".into(),
            }
            .into(),
        ],
    );
    assert_eq!(failures.len(), 1);
    assert!(matches!(failures[0].error, Error::Missing(_)));
}

#[test]
fn a_failing_reaction_is_reported_and_keeps_the_first_commit() {
    let (_dir, mut vault, _goal, _saver) = saving_vault();
    let savings = vault.doc.stores[0].id.clone();
    // The first commit: a new goal. Then the reaction to a missing automation fails.
    let committed = vault.create_goal(goal_on(&savings)).unwrap();
    let failures = react(
        &mut vault,
        vec![AutomationDeleted { id: "gone".into() }.into()],
    );
    assert_eq!(failures.len(), 1);
    assert!(matches!(failures[0].error, Error::Missing(_)));
    assert_eq!(
        failures[0].event,
        DomainEvent::from(AutomationDeleted { id: "gone".into() })
    );
    assert!(vault.doc.goals.iter().any(|each| each.id == committed.id));
}

fn first(vault: &mut UnlockedVault, _event: &DomainEvent) -> Result<()> {
    vault.doc.settings.schedule_timezone = Some("first".into());
    Ok(())
}

fn failing(_vault: &mut UnlockedVault, _event: &DomainEvent) -> Result<()> {
    Err(Error::InvalidInput("boom".into()))
}

fn second(vault: &mut UnlockedVault, _event: &DomainEvent) -> Result<()> {
    let seen = vault.doc.settings.schedule_timezone.clone();
    vault.doc.settings.schedule_timezone = Some(format!("{}+second", seen.unwrap_or_default()));
    Ok(())
}

#[test]
fn reactions_run_in_registration_order_and_a_failure_does_not_stop_the_rest() {
    let (_dir, mut vault) = empty_vault();
    let reactions: [Reaction; 3] = [first, failing, second];
    let failures = run(
        &mut vault,
        vec![GoalPaused { id: "any".into() }.into()],
        &reactions,
    );
    assert_eq!(failures.len(), 1);
    assert_eq!(
        vault.doc.settings.schedule_timezone.as_deref(),
        Some("first+second")
    );
}

#[test]
fn a_retry_with_the_event_zone_books_the_refused_run_without_a_vault_zone() {
    let (_dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    let Transaction::Payment(rent) = payment(&card, "800", "rent") else {
        unreachable!()
    };
    vault
        .create_automation(AutomationDraft {
            time: None,
            asks: false,
            for_month_offset: None,
            name: "Rent".into(),
            active: true,
            preset: None,
            schedule: Schedule::Monthly { day: 5 },
            starts_at: "2026-01-01T00:00:00Z".into(),
            template: TransactionDraft::Payment(PaymentTx { ..rent }),
            amount_mode: Default::default(),
            workday_shift: Default::default(),
            property_id: None,
        })
        .unwrap();
    let pass = vault
        .run_due_workflows(secs("2026-02-20T12:00:00Z"), "UTC")
        .unwrap();
    let refused = pass.runs[0].id.clone();
    vault
        .add_transaction(income(&card, "5000", "salary"))
        .unwrap();
    vault.doc.settings.schedule_timezone = None;
    let failures = react(
        &mut vault,
        vec![
            AutomationRetried {
                id: refused.clone(),
                zone: "UTC".into(),
            }
            .into(),
        ],
    );
    assert!(failures.is_empty(), "{failures:?}");
    assert!(vault.doc.workflow_runs.iter().all(|run| run.id != refused));
}
