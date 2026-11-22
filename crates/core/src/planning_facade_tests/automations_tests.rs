//! Tests of the automations area: the old inline tests (names and bodies unchanged).

use crate::calendar::{self, Calendar};
use crate::model::planning::draft_to_transaction;
use crate::model::planning::{
    AmountMode, Automation, AutomationOutcome, AutomationPreset, RejectReason, Schedule,
    TransactionDraft, WorkdayShift,
};
use crate::model::store::StoreKind;
use crate::model::store::{LiabilityKind, LiabilityTerms};
use crate::model::transaction::{AccountRef, PaymentTx, TopupTx};
use crate::model::transaction::{Transaction, TxOrigin};
use crate::model::vault::now_iso;
use crate::planning::automation_plan::{
    automation_occurrences, automation_of, automation_run_of, automation_runs,
};
use crate::planning::automations::*;
use crate::service::StoreExtras;
use crate::service::UnlockedVault;
use crate::service::WorkflowPass;
use crate::service::test_support::*;
use crate::time::unix_secs;
use crate::usecases::planning::automation_rules::{
    check_draft_moment, manual_run_record, new_automation_workflow, rewrite_automation,
    simulation_report,
};
use crate::workflow::automation as graph_of;
use crate::workflow::model::RunOutcome;
use pretty_assertions::assert_eq;
use std::collections::HashSet;

fn secs(iso: &str) -> i64 {
    unix_secs(iso).unwrap()
}

fn salary_draft(store: &str, sum: &str) -> TransactionDraft {
    let Transaction::Topup(inner) = income(store, sum, "salary") else {
        unreachable!()
    };
    TransactionDraft::Topup(TopupTx { ..inner })
}

fn rent_draft(store: &str, sum: &str) -> TransactionDraft {
    let Transaction::Payment(mut inner) = payment(store, sum, "rent") else {
        unreachable!()
    };
    inner.meta.categories = vec!["rent_mortgage".into()];
    TransactionDraft::Payment(PaymentTx { ..inner })
}

fn monthly_draft(name: &str, day: u8, template: TransactionDraft) -> AutomationDraft {
    AutomationDraft {
        time: None,
        asks: false,
        for_month_offset: None,
        name: name.into(),
        active: true,
        preset: None,
        schedule: Schedule::Monthly { day },
        starts_at: "2026-01-01T00:00:00Z".into(),
        template,
        amount_mode: Default::default(),
        workday_shift: Default::default(),
        property_id: None,
    }
}

fn monthly(
    vault: &mut UnlockedVault,
    name: &str,
    day: u8,
    template: TransactionDraft,
) -> Automation {
    crate::usecases::planning::automations::create_automation(
        vault,
        monthly_draft(name, day, template),
    )
    .unwrap()
}

fn rejected_of(pass: &WorkflowPass) -> Vec<RejectReason> {
    pass.runs
        .iter()
        .filter_map(|run| match &run.outcome {
            RunOutcome::Rejected { reason, .. } => Some(reason.clone()),
            _ => None,
        })
        .collect()
}

fn outcomes(vault: &UnlockedVault, id: &str) -> Vec<&'static str> {
    let mut runs = crate::planning::automation_plan::automation_runs(&vault.books(), Some(id));
    runs.reverse();
    runs.iter()
        .map(|run| match run.outcome {
            AutomationOutcome::Created { .. } => "created",
            AutomationOutcome::Rejected { .. } => "rejected",
            AutomationOutcome::Skipped => "skipped",
        })
        .collect()
}

/// An emulation from today reaches months ahead: the runs are dated on
/// the emulated days and must not trip the future-date rule (V11).
#[test]
fn an_emulation_from_today_runs_a_year_ahead() {
    let (_dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    let salary = monthly(&mut vault, "Salary", 10, salary_draft(&card, "1000"));
    let now = secs(&now_iso());

    let sim = crate::usecases::planning::automations::simulate_automations(
        &vault,
        Some(&salary.id),
        365,
        now,
        "UTC",
    )
    .unwrap();
    assert!(sim.runs.len() >= 12, "{} runs", sim.runs.len());
    assert_eq!(sim.rejected, 0, "{:?}", sim.runs);
    assert_eq!(sim.created as usize, sim.runs.len());
}

/// An automation may start months from now; a manual entry that far
/// ahead is still refused.
#[test]
fn a_future_start_is_accepted_but_a_future_entry_is_not() {
    let (_dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    let later = calendar::iso(secs(&now_iso()) + 30 * 86_400);
    crate::usecases::planning::automations::create_automation(
        &mut vault,
        AutomationDraft {
            time: None,
            asks: false,
            for_month_offset: None,
            name: "Later".into(),
            active: true,
            preset: None,
            schedule: Schedule::Monthly { day: 1 },
            starts_at: later.clone(),
            template: salary_draft(&card, "100"),
            amount_mode: Default::default(),
            workday_shift: Default::default(),
            property_id: None,
        },
    )
    .unwrap();

    let mut tx = income(&card, "100", "salary");
    tx.meta_mut().timestamp = later;
    let err = crate::usecases::money::transactions::add_transaction(&mut vault, tx).unwrap_err();
    assert!(err.to_string().contains("too far in the future"), "{err}");
}

/// Emulation runs on a copy: what would happen is reported, nothing of it
/// reaches the vault, and a later real run is unaffected.
#[test]
fn an_emulation_reports_the_runs_and_writes_nothing() {
    let (_dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    let salary = monthly(&mut vault, "Salary", 10, salary_draft(&card, "1000"));
    let rent = monthly(&mut vault, "Rent", 5, rent_draft(&card, "800"));
    let before = vault.doc.clone();
    let now = secs("2026-01-01T00:00:00Z");

    let all =
        crate::usecases::planning::automations::simulate_automations(&vault, None, 50, now, "UTC")
            .unwrap();
    // Jan 5 rent: nothing yet. Jan 10 salary. Feb 5 rent. Feb 10 salary.
    assert_eq!(all.runs.len(), 4);
    assert_eq!((all.created, all.rejected), (3, 1));
    assert!(
        all.runs
            .windows(2)
            .all(|pair| pair[0].planned_at <= pair[1].planned_at)
    );
    let balance = all
        .balances
        .iter()
        .find(|row| row.store_id == card)
        .unwrap();
    assert_eq!(
        (balance.before, balance.after),
        (amount("0"), amount("1200"))
    );

    let only_rent = crate::usecases::planning::automations::simulate_automations(
        &vault,
        Some(&rent.id),
        50,
        now,
        "UTC",
    )
    .unwrap();
    assert!(
        only_rent
            .runs
            .iter()
            .all(|run| run.automation_id == rent.id)
    );
    assert_eq!(
        only_rent.runs.len(),
        2,
        "the salary still runs, it is just not listed"
    );

    assert_eq!(vault.doc, before, "the vault is untouched");
    let pass = crate::usecases::workflow::runs::run_due_workflows(
        &mut vault,
        secs("2026-02-20T12:00:00Z"),
        "UTC",
    )
    .unwrap();
    assert_eq!(pass.created.len(), 3);
    assert!(
        crate::usecases::planning::automations::simulate_automations(
            &vault,
            Some("nope"),
            10,
            now,
            "UTC"
        )
        .is_err()
    );
    let _ = salary;
}

#[test]
fn the_31st_runs_on_the_last_day_of_february() {
    let (_dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    let rent = monthly(&mut vault, "Rent", 31, rent_draft(&card, "1"));
    let dates: Vec<String> = vault
        .automation_occurrences(
            &rent,
            &Calendar::new("UTC"),
            secs("2026-01-01T00:00:00Z"),
            secs("2026-03-31T23:00:00Z"),
        )
        .into_iter()
        .map(|(moment, _)| calendar::iso(moment))
        .collect();
    assert_eq!(
        dates,
        vec![
            "2026-01-31T09:00:00Z",
            "2026-02-28T09:00:00Z",
            "2026-03-31T09:00:00Z"
        ]
    );
    let every = crate::usecases::planning::automations::create_automation(
        &mut vault,
        AutomationDraft {
            time: None,
            asks: false,
            for_month_offset: None,
            name: "Fortnight".into(),
            active: true,
            preset: None,
            schedule: Schedule::EveryNDays { days: 14 },
            starts_at: "2026-03-04T00:00:00Z".into(),
            template: rent_draft(&card, "1"),
            amount_mode: AmountMode::Fixed,
            workday_shift: WorkdayShift::None,
            property_id: None,
        },
    )
    .unwrap();
    let dates: Vec<String> = vault
        .automation_occurrences(
            &every,
            &Calendar::new("UTC"),
            secs("2026-03-01T00:00:00Z"),
            secs("2026-04-01T00:00:00Z"),
        )
        .into_iter()
        .map(|(moment, _)| calendar::iso(moment))
        .collect();
    // Counted from the first date, not from some fixed day.
    assert_eq!(dates, vec!["2026-03-04T09:00:00Z", "2026-03-18T09:00:00Z"]);
}

#[test]
fn salary_before_rent_covers_it_and_rent_before_salary_is_rejected() {
    let (_dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    let salary = monthly(&mut vault, "Salary", 10, salary_draft(&card, "1000"));
    let rent = monthly(&mut vault, "Rent", 5, rent_draft(&card, "800"));

    let pass = crate::usecases::workflow::runs::run_due_workflows(
        &mut vault,
        secs("2026-02-20T12:00:00Z"),
        "UTC",
    )
    .unwrap();
    // Jan 5 rent: nothing yet. Jan 10 salary. Feb 5 rent: 1000 ≥ 800. Feb 10 salary.
    assert_eq!(outcomes(&vault, &rent.id), vec!["rejected", "created"]);
    assert_eq!(outcomes(&vault, &salary.id), vec!["created", "created"]);
    assert_eq!(pass.created.len(), 3);
    let rejected = rejected_of(&pass);
    assert_eq!(rejected.len(), 1);
    let RejectReason::InsufficientFunds {
        required,
        available,
        ..
    } = &rejected[0]
    else {
        panic!("{rejected:?}");
    };
    assert_eq!((*required, *available), (amount("800"), amount("0")));
    assert_eq!(holding_of(&vault, &card, &usd()), amount("1200"));

    // The processed dates are not processed again.
    let again = crate::usecases::workflow::runs::run_due_workflows(
        &mut vault,
        secs("2026-02-20T13:00:00Z"),
        "UTC",
    )
    .unwrap();
    assert!(again.runs.is_empty() && again.created.is_empty());
}

#[test]
fn the_fee_counts_toward_the_required_amount() {
    let (_dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    let mut seed = income(&card, "100", "seed");
    at(&mut seed, "2025-12-01T00:00:00Z");
    crate::usecases::money::transactions::add_transaction(&mut vault, seed).unwrap();
    let TransactionDraft::Payment(mut bill) = rent_draft(&card, "95") else {
        unreachable!()
    };
    bill.fee = fee_usd("10");
    let automation = monthly(&mut vault, "Bill", 1, TransactionDraft::Payment(bill));
    let pass = crate::usecases::workflow::runs::run_due_workflows(
        &mut vault,
        secs("2026-01-02T00:00:00Z"),
        "UTC",
    )
    .unwrap();
    assert_eq!(outcomes(&vault, &automation.id), vec!["rejected"]);
    let RejectReason::InsufficientFunds { required, .. } = &rejected_of(&pass)[0] else {
        panic!()
    };
    assert_eq!(*required, amount("105"));
}

#[test]
fn catch_up_creates_at_most_24_and_skips_the_rest() {
    let (_dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    let daily = crate::usecases::planning::automations::create_automation(
        &mut vault,
        AutomationDraft {
            time: None,
            asks: false,
            for_month_offset: None,
            name: "Pocket money".into(),
            active: true,
            preset: None,
            schedule: Schedule::EveryNDays { days: 1 },
            starts_at: "2026-01-01T00:00:00Z".into(),
            template: salary_draft(&card, "1"),
            amount_mode: Default::default(),
            workday_shift: Default::default(),
            property_id: None,
        },
    )
    .unwrap();
    let pass = crate::usecases::workflow::runs::run_due_workflows(
        &mut vault,
        secs("2026-01-30T12:00:00Z"),
        "UTC",
    )
    .unwrap();
    assert_eq!(pass.created.len(), MAX_CATCH_UP);
    assert_eq!(pass.skipped, 30 - MAX_CATCH_UP);
    assert_eq!(
        crate::planning::automation_plan::automation_runs(&vault.books(), Some(&daily.id)).len(),
        30
    );

    // Even with the processed marker rolled back nothing is created twice.
    crate::usecases::workflow::runs::wf_set_mark(
        &mut vault,
        &daily.id,
        "schedule:t",
        "2025-12-31T00:00:00Z",
    );
    let again = crate::usecases::workflow::runs::run_due_workflows(
        &mut vault,
        secs("2026-01-30T12:00:00Z"),
        "UTC",
    )
    .unwrap();
    assert!(
        again.runs.is_empty() && again.created.is_empty(),
        "{:?}",
        again
            .runs
            .iter()
            .map(|run| (&run.trigger, &run.outcome))
            .collect::<Vec<_>>()
    );
}

#[test]
fn credit_card_checks_the_limit_and_without_one_passes() {
    let (_dir, mut vault) = empty_vault();
    let terms = |limit: Option<&str>| LiabilityTerms {
        kind: LiabilityKind::CreditCard,
        rate: None,
        limit: limit.map(amount),
        due_day: None,
        monthly_payment: None,
    };
    let limited = crate::usecases::money::stores::create_store_with(
        &mut vault,
        "Card",
        StoreKind::Liability,
        "",
        "",
        None,
        StoreExtras {
            liability: Some(terms(Some("100"))),
            ..StoreExtras::default()
        },
    )
    .unwrap()
    .id;
    let open = crate::usecases::money::stores::create_store_with(
        &mut vault,
        "Open card",
        StoreKind::Liability,
        "",
        "",
        None,
        StoreExtras {
            liability: Some(terms(None)),
            ..StoreExtras::default()
        },
    )
    .unwrap()
    .id;
    let over = draft_to_transaction(
        &rent_draft(&limited, "150"),
        "a",
        "",
        "2026-01-01T09:00:00Z",
    );
    assert!(matches!(
        crate::planning::automations::funds_shortfall(
            &vault.doc,
            &over,
            secs("2026-01-01T09:00:00Z")
        ),
        Some(RejectReason::CreditLimitExceeded { .. })
    ));
    let within = draft_to_transaction(&rent_draft(&limited, "90"), "a", "", "2026-01-01T09:00:00Z");
    assert!(
        crate::planning::automations::funds_shortfall(
            &vault.doc,
            &within,
            secs("2026-01-01T09:00:00Z")
        )
        .is_none()
    );
    let unlimited =
        draft_to_transaction(&rent_draft(&open, "5000"), "a", "", "2026-01-01T09:00:00Z");
    assert!(
        crate::planning::automations::funds_shortfall(
            &vault.doc,
            &unlimited,
            secs("2026-01-01T09:00:00Z")
        )
        .is_none()
    );
}

#[test]
fn incoming_money_and_a_contact_source_are_not_checked() {
    let (_dir, vault) = empty_vault();
    let topup = draft_to_transaction(&salary_draft("x", "100"), "a", "", "");
    assert!(crate::planning::automations::funds_shortfall(&vault.doc, &topup, 0).is_none());
    let from_contact = transfer_between(
        AccountRef::Raw("Mom".into()),
        AccountRef::Store("x".into()),
        usd(),
        "100",
    );
    assert!(crate::planning::automations::funds_shortfall(&vault.doc, &from_contact, 0).is_none());
}

#[test]
fn retry_checks_the_current_balance_and_skip_dismisses() {
    let (_dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    let rent = monthly(&mut vault, "Rent", 5, rent_draft(&card, "800"));
    crate::usecases::workflow::runs::run_due_workflows(
        &mut vault,
        secs("2026-02-20T12:00:00Z"),
        "UTC",
    )
    .unwrap();
    let runs = crate::planning::automation_plan::automation_runs(&vault.books(), Some(&rent.id));
    assert_eq!(runs.len(), 2);

    let now = secs("2026-02-21T12:00:00Z");
    let err = crate::usecases::planning::automations::retry_automation_run(
        &mut vault,
        &runs[0].id,
        now,
        "UTC",
    )
    .unwrap_err();
    assert_eq!(err.code(), "INSUFFICIENT_FUNDS");

    let mut salary = income(&card, "1000", "salary");
    at(&mut salary, "2026-02-21T10:00:00Z");
    crate::usecases::money::transactions::add_transaction(&mut vault, salary).unwrap();
    let created = crate::usecases::planning::automations::retry_automation_run(
        &mut vault,
        &runs[0].id,
        now,
        "UTC",
    )
    .unwrap();
    assert_eq!(created.meta().timestamp, "2026-02-21T12:00:00Z");
    assert!(matches!(
        crate::planning::automation_plan::automation_runs(&vault.books(), Some(&rent.id))[0]
            .outcome,
        AutomationOutcome::Created { .. }
    ));

    let skipped =
        crate::usecases::planning::automations::skip_automation_run(&mut vault, &runs[1].id)
            .unwrap();
    assert_eq!(skipped.outcome, AutomationOutcome::Skipped);
}

#[test]
fn the_planned_moment_is_nine_in_the_local_zone() {
    let (_dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    monthly(&mut vault, "Salary", 10, salary_draft(&card, "1"));
    // 09:00 in Bangkok on Jan 10 is 02:00 UTC.
    let pass = crate::usecases::workflow::runs::run_due_workflows(
        &mut vault,
        secs("2026-01-10T02:30:00Z"),
        "Asia/Bangkok",
    )
    .unwrap();
    assert_eq!(pass.created.len(), 1);
    let tx = vault
        .doc
        .transactions
        .iter()
        .find(|tx| tx.meta().id == pass.created[0])
        .unwrap();
    assert_eq!(tx.meta().timestamp, "2026-01-10T02:00:00Z");
}

#[test]
fn a_run_made_days_after_its_plan_is_dated_when_it_ran() {
    let (_dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    monthly(&mut vault, "Salary", 10, salary_draft(&card, "1"));
    // The app was closed on the 10th: the run is caught up on the 14th.
    let now = secs("2026-01-14T12:00:00Z");
    let pass = crate::usecases::workflow::runs::run_due_workflows(&mut vault, now, "UTC").unwrap();
    assert_eq!(pass.created.len(), 1);
    let tx = vault
        .doc
        .transactions
        .iter()
        .find(|tx| tx.meta().id == pass.created[0])
        .unwrap();
    assert_eq!(tx.meta().timestamp, "2026-01-14T12:00:00Z");
    assert!(matches!(
        &tx.meta().origin,
        TxOrigin::Workflow { planned_at: Some(planned), .. } if planned == "2026-01-10T09:00:00Z"
    ));
}

#[test]
fn a_template_that_breaks_the_rules_is_refused() {
    let (_dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    let TransactionDraft::Payment(mut bill) = rent_draft(&card, "10") else {
        unreachable!()
    };
    bill.meta.categories.clear();
    let err = crate::usecases::planning::automations::create_automation(
        &mut vault,
        AutomationDraft {
            time: None,
            asks: false,
            for_month_offset: None,
            name: "Bill".into(),
            active: true,
            preset: None,
            schedule: Schedule::Monthly { day: 1 },
            starts_at: "2026-01-01T00:00:00Z".into(),
            template: TransactionDraft::Payment(bill),
            amount_mode: Default::default(),
            workday_shift: Default::default(),
            property_id: None,
        },
    )
    .unwrap_err();
    assert_eq!(err.code(), "INVALID_INPUT");
}

fn advance(vault: &mut UnlockedVault, card: &str) -> Automation {
    crate::usecases::planning::automations::create_automation(
        vault,
        AutomationDraft {
            time: None,
            asks: false,
            for_month_offset: None,
            name: "Advance".into(),
            active: true,
            preset: Some(AutomationPreset::Salary),
            schedule: Schedule::Monthly { day: 20 },
            starts_at: "2025-06-01T00:00:00Z".into(),
            template: salary_draft(card, "1000"),
            amount_mode: AmountMode::Varies,
            workday_shift: WorkdayShift::Back,
            property_id: None,
        },
    )
    .unwrap()
}

fn made(vault: &UnlockedVault, pass: &WorkflowPass) -> Transaction {
    vault
        .doc
        .transactions
        .iter()
        .find(|tx| tx.meta().id == pass.created[0])
        .unwrap()
        .clone()
}

fn salary_for_month(
    vault: &mut UnlockedVault,
    card: &str,
    day: u8,
    offset: i8,
    starts_at: &str,
) -> Automation {
    crate::usecases::planning::automations::create_automation(
        vault,
        AutomationDraft {
            time: None,
            asks: false,
            for_month_offset: Some(offset),
            name: format!("Pay {day}"),
            active: true,
            preset: Some(AutomationPreset::Salary),
            schedule: Schedule::Monthly { day },
            starts_at: starts_at.into(),
            template: salary_draft(card, "1000"),
            amount_mode: AmountMode::Fixed,
            workday_shift: WorkdayShift::Back,
            property_id: None,
        },
    )
    .unwrap()
}

/// "For which month": the advance of Sunday July 20, 2025 comes on Friday
/// the 18th and is for July; the salary of the 5th, a Sunday in Oct 2025
/// (paid on Friday the 3rd), is for the month before. The shift changes
/// neither.
#[test]
fn an_income_made_by_an_automation_carries_its_for_month() {
    let (_dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    let advance = salary_for_month(&mut vault, &card, 20, 0, "2025-07-01T00:00:00Z");
    assert_eq!(advance.for_month_offset, Some(0));
    let pass = crate::usecases::workflow::runs::run_due_workflows(
        &mut vault,
        secs("2025-07-18T12:00:00Z"),
        "UTC",
    )
    .unwrap();
    let Transaction::Topup(july) = made(&vault, &pass) else {
        panic!("a top-up")
    };
    assert_eq!(july.meta.timestamp, "2025-07-18T09:00:00Z");
    assert_eq!(july.details.for_month.as_deref(), Some("2025-07"));
    crate::usecases::planning::automations::delete_automation(&mut vault, &advance.id).unwrap();

    salary_for_month(&mut vault, &card, 5, -1, "2025-10-01T00:00:00Z");
    let pass = crate::usecases::workflow::runs::run_due_workflows(
        &mut vault,
        secs("2025-10-03T12:00:00Z"),
        "UTC",
    )
    .unwrap();
    let Transaction::Topup(october) = made(&vault, &pass) else {
        panic!("a top-up")
    };
    assert_eq!(october.meta.timestamp, "2025-10-03T09:00:00Z");
    assert_eq!(october.details.for_month.as_deref(), Some("2025-09"));
}

/// The advance of July 20, 2025, a Sunday, comes on Friday the 18th at the
/// template's amount, marked approximate; the real amount put in makes the
/// next estimate.
#[test]
fn a_salary_that_differs_is_booked_approximately_and_corrected() {
    let (_dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    let salary = advance(&mut vault, &card);
    assert_eq!(salary.estimate, None);

    let forecast = vault.automation_occurrences(
        &salary,
        &Calendar::new("UTC"),
        secs("2025-07-01T00:00:00Z"),
        secs("2025-07-31T00:00:00Z"),
    );
    assert_eq!(calendar::iso(forecast[0].0), "2025-07-18T09:00:00Z");
    assert!(forecast[0].1.meta().origin.is_estimated());

    // June 20, 2025 is a Friday; July 20 a Sunday.
    let pass = crate::usecases::workflow::runs::run_due_workflows(
        &mut vault,
        secs("2025-06-20T10:00:00Z"),
        "UTC",
    )
    .unwrap();
    let june = made(&vault, &pass);
    assert!(june.meta().origin.is_estimated());
    assert_eq!(main_amount(&june), Some(amount("1000")));
    let runs = crate::planning::automation_plan::automation_runs(&vault.books(), Some(&salary.id));
    assert!(runs[0].estimated);

    let corrected = crate::usecases::planning::automations::confirm_estimated_amount(
        &mut vault,
        &june.meta().id,
        amount("1300"),
    )
    .unwrap();
    assert!(!corrected.meta().origin.is_estimated());
    assert_eq!(main_amount(&corrected), Some(amount("1300")));
    assert!(
        !crate::planning::automation_plan::automation_runs(&vault.books(), Some(&salary.id))[0]
            .estimated
    );
    assert_eq!(
        crate::usecases::planning::automations::automation(&vault, &salary.id)
            .unwrap()
            .estimate,
        Some(amount("1300"))
    );
    // Only an approximate operation takes a real amount.
    assert!(
        crate::usecases::planning::automations::confirm_estimated_amount(
            &mut vault,
            &june.meta().id,
            amount("1")
        )
        .is_err()
    );

    let pass = crate::usecases::workflow::runs::run_due_workflows(
        &mut vault,
        secs("2025-07-18T10:00:00Z"),
        "UTC",
    )
    .unwrap();
    let july = made(&vault, &pass);
    assert_eq!(july.meta().timestamp, "2025-07-18T09:00:00Z");
    assert_eq!(main_amount(&july), Some(amount("1300")));
    // Not again on the Sunday it was planned for.
    let pass = crate::usecases::workflow::runs::run_due_workflows(
        &mut vault,
        secs("2025-07-20T10:00:00Z"),
        "UTC",
    )
    .unwrap();
    assert!(pass.created.is_empty());

    // An ordinary edit of the amount counts as the real one too.
    let mut edited = july.clone();
    if let Transaction::Topup(inner) = &mut edited {
        inner.amount = amount("1500");
    }
    let saved = crate::usecases::money::transactions::update_transaction(
        &mut vault,
        &july.meta().id,
        edited,
    )
    .unwrap();
    assert!(!saved.meta().origin.is_estimated());
    assert_eq!(
        crate::usecases::planning::automations::automation(&vault, &salary.id)
            .unwrap()
            .estimate,
        Some(amount("1400"))
    );
}

#[test]
fn an_automation_is_edited_in_its_form_until_made_an_ordinary_workflow() {
    let (_dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    let salary = advance(&mut vault, &card);
    let workflow = crate::workflow::service::records::workflow(&vault.doc, &salary.id).unwrap();
    let draft = crate::service::WorkflowDraft {
        name: workflow.name.clone(),
        description: String::new(),
        active: true,
        graph: workflow.graph.clone(),
        react_to_workflows: false,
        max_amount: None,
        max_amount_currency: None,
    };
    let err = crate::usecases::workflow::records::save_workflow(
        &mut vault,
        Some(&salary.id),
        draft.clone(),
        secs(&now_iso()),
    )
    .unwrap_err();
    assert_eq!(err.code(), "INVALID_INPUT");

    // A change in the form rebuilds the graph and keeps the old one.
    let mut changed = AutomationDraft {
        time: None,
        asks: false,
        for_month_offset: None,
        name: "Advance".into(),
        active: true,
        preset: Some(AutomationPreset::Salary),
        schedule: Schedule::Monthly { day: 20 },
        starts_at: "2025-06-01T00:00:00Z".into(),
        template: salary_draft(&card, "1000"),
        amount_mode: AmountMode::Fixed,
        workday_shift: WorkdayShift::Forward,
        property_id: None,
    };
    crate::usecases::planning::automations::update_automation(
        &mut vault,
        &salary.id,
        changed.clone(),
    )
    .unwrap();
    let workflow = crate::workflow::service::records::workflow(&vault.doc, &salary.id).unwrap();
    assert_eq!(workflow.versions.len(), 1);
    assert!(workflow.graph.contains("\"forward\""), "{}", workflow.graph);

    let ordinary =
        crate::usecases::planning::automations::detach_automation(&mut vault, &salary.id).unwrap();
    assert!(ordinary.automation.is_none());
    assert!(crate::usecases::planning::automations::automations(&vault).is_empty());
    crate::usecases::workflow::records::save_workflow(
        &mut vault,
        Some(&salary.id),
        draft,
        secs(&now_iso()),
    )
    .unwrap();
    changed.name = "Again".into();
    assert!(
        crate::usecases::planning::automations::update_automation(&mut vault, &salary.id, changed)
            .is_err()
    );
}

#[test]
fn an_exchange_cannot_have_an_amount_that_differs() {
    let (_dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    let Some(Transaction::Convert(mut inner)) = crate::model::fixtures::full_doc()
        .transactions
        .into_iter()
        .find(|tx| matches!(tx, Transaction::Convert(_)))
    else {
        unreachable!()
    };
    inner.from.account = crate::model::transaction::AccountRef::Store(card.clone());
    inner.to.account = crate::model::transaction::AccountRef::Store(card);
    let err = crate::usecases::planning::automations::create_automation(
        &mut vault,
        AutomationDraft {
            time: None,
            asks: false,
            for_month_offset: None,
            name: "DCA".into(),
            active: true,
            preset: None,
            schedule: Schedule::Weekly { weekday: 4 },
            starts_at: "2026-01-01T00:00:00Z".into(),
            template: TransactionDraft::Convert(inner),
            amount_mode: AmountMode::Varies,
            workday_shift: WorkdayShift::None,
            property_id: None,
        },
    )
    .unwrap_err();
    assert_eq!(err.code(), "INVALID_INPUT");
}

#[test]
fn a_repeat_fires_at_its_time_and_asks_before_booking() {
    let (_dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    let draft = AutomationDraft {
        time: Some("18:30".into()),
        asks: true,
        for_month_offset: None,
        name: "Asks".into(),
        active: true,
        preset: None,
        schedule: Schedule::Monthly { day: 10 },
        starts_at: "2026-01-01T00:00:00Z".into(),
        template: salary_draft(&card, "1000"),
        amount_mode: Default::default(),
        workday_shift: Default::default(),
        property_id: None,
    };
    let automation =
        crate::usecases::planning::automations::create_automation(&mut vault, draft.clone())
            .unwrap();
    assert_eq!(automation.time.as_deref(), Some("18:30"));
    assert!(automation.asks);

    let workflow = vault
        .doc
        .workflows
        .iter()
        .find(|wf| wf.id == automation.id)
        .unwrap();
    let graph = crate::workflow::graph::Graph::parse(&workflow.graph).unwrap();
    assert!(graph.nodes.iter().any(|node| node.kind == "action.confirm"));
    assert!(
        graph
            .edges
            .iter()
            .any(|edge| edge.from_port == "true" && edge.to == graph_of::ACTION)
    );

    let cal = Calendar::new("UTC");
    let after = secs("2026-03-01T00:00:00Z");
    let moments = vault.automation_occurrences(&automation, &cal, after, after + 40 * 86_400);
    assert_eq!(calendar::iso(moments[0].0), "2026-03-10T18:30:00Z");

    let bad = AutomationDraft {
        time: Some("25:00".into()),
        ..draft
    };
    assert!(crate::usecases::planning::automations::create_automation(&mut vault, bad).is_err());
}

#[test]
fn the_form_lists_the_next_three_dates_moved_off_days_off() {
    let input = crate::ipc_contract::AutomationDatesInput {
        schedule: Schedule::Monthly { day: 1 },
        starts_at: "2026-10-24T00:00:00Z".into(),
        workday_shift: WorkdayShift::Back,
        time: None,
    };
    let dates = input.upcoming(secs("2026-10-24T12:00:00Z"), "UTC");
    assert_eq!(dates.len(), 3);
    // 1 November 2026 is a Sunday: back to Friday.
    assert_eq!(dates[0], "2026-10-30T09:00:00Z");
    assert_eq!(dates[1], "2026-12-01T09:00:00Z");
}

/// Planned moments of every booking an automation workflow made, oldest first.
fn planned_of_bookings(vault: &UnlockedVault) -> Vec<String> {
    vault
        .doc
        .transactions
        .iter()
        .filter_map(|tx| match &tx.meta().origin {
            TxOrigin::Workflow {
                planned_at: Some(planned),
                ..
            } => Some(planned.clone()),
            _ => None,
        })
        .collect()
}

/// Monthly rent on the 15th at 09:00 Berlin, started on Jan 1.
fn berlin_rent(vault: &mut UnlockedVault, from: &str) -> Automation {
    let account = store(vault, "Cash", StoreKind::Cash, None);
    let income = {
        let mut funding = income(&account, "1000", "float");
        at(&mut funding, "2026-01-01T00:00:00Z");
        funding
    };
    crate::usecases::money::transactions::add_transaction(vault, income).unwrap();
    monthly(vault, "Rent", 15, rent_draft(&account, from))
}

/// G6: one booking per planned date; a later tick the same day books nothing.
#[test]
fn golden_g6_a_monthly_automation_fires_once_per_planned_date() {
    let (_dir, mut vault) = empty_vault();
    berlin_rent(&mut vault, "300");
    let tick = |vault: &mut UnlockedVault, iso: &str| {
        crate::usecases::workflow::runs::run_due_workflows(vault, secs(iso), "Europe/Berlin")
            .unwrap()
            .created
            .len()
    };
    assert_eq!(tick(&mut vault, "2026-01-14T12:00:00Z"), 0);
    assert_eq!(planned_of_bookings(&vault).len(), 0);
    assert_eq!(tick(&mut vault, "2026-01-15T09:00:00Z"), 1);
    assert_eq!(planned_of_bookings(&vault), ["2026-01-15T08:00:00Z"]);
    assert_eq!(tick(&mut vault, "2026-01-15T20:00:00Z"), 0);
    assert_eq!(planned_of_bookings(&vault).len(), 1);
    assert_eq!(tick(&mut vault, "2026-02-20T12:00:00Z"), 1);
    assert_eq!(
        planned_of_bookings(&vault),
        ["2026-01-15T08:00:00Z", "2026-02-15T08:00:00Z"]
    );
}

/// G7: a long absence books up to `MAX_CATCH_UP` dates and journals the older ones as skipped.
#[test]
fn golden_g7_catch_up_books_the_newest_and_skips_the_older() {
    let (_dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    // The one monthly date inside 40 days is booked, nothing is skipped.
    let rent = monthly(&mut vault, "Rent", 15, salary_draft(&card, "1"));
    let pass = crate::usecases::workflow::runs::run_due_workflows(
        &mut vault,
        secs("2026-02-10T12:00:00Z"),
        "Europe/Berlin",
    )
    .unwrap();
    assert_eq!((pass.created.len(), pass.skipped), (1, 0));
    assert_eq!(planned_of_bookings(&vault), ["2026-01-15T08:00:00Z"]);
    assert_eq!(outcomes(&vault, &rent.id), ["created"]);

    // A daily automation over the same 40 days: 24 booked, 16 skipped.
    let daily = crate::usecases::planning::automations::create_automation(
        &mut vault,
        AutomationDraft {
            time: None,
            asks: false,
            for_month_offset: None,
            name: "Daily".into(),
            active: true,
            preset: None,
            schedule: Schedule::EveryNDays { days: 1 },
            starts_at: "2026-01-01T00:00:00Z".into(),
            template: salary_draft(&card, "1"),
            amount_mode: Default::default(),
            workday_shift: Default::default(),
            property_id: None,
        },
    )
    .unwrap();
    let before = planned_of_bookings(&vault).len();
    let pass = crate::usecases::workflow::runs::run_due_workflows(
        &mut vault,
        secs("2026-02-10T12:00:00Z"),
        "Europe/Berlin",
    )
    .unwrap();
    assert_eq!(pass.created.len(), MAX_CATCH_UP);
    assert_eq!(pass.skipped, 41 - MAX_CATCH_UP);
    assert_eq!(planned_of_bookings(&vault).len() - before, MAX_CATCH_UP);
    let runs = outcomes(&vault, &daily.id);
    assert_eq!(runs.len(), 41);
    assert_eq!(
        runs.iter().filter(|run| **run == "skipped").count(),
        41 - MAX_CATCH_UP
    );
}

/// G8: the first numbers a user sees after G6's rent and a liability card.
#[test]
fn golden_g8_net_worth_and_the_daily_budget_after_the_rent() {
    let (_dir, mut vault) = empty_vault();
    vault.set_base_currency("usd");
    quote_usd(&mut vault);
    berlin_rent(&mut vault, "300");
    let bank = store(&mut vault, "Bank", StoreKind::Bank, None);
    let card = crate::usecases::money::stores::create_store_with(
        &mut vault,
        "Card",
        StoreKind::Liability,
        "",
        "",
        None,
        StoreExtras {
            liability: Some(LiabilityTerms {
                kind: LiabilityKind::CreditCard,
                rate: None,
                limit: Some(amount("1000")),
                due_day: Some(25),
                monthly_payment: None,
            }),
            ..StoreExtras::default()
        },
    )
    .unwrap()
    .id;
    let side = |from: &str, to: &str, sum: &str| {
        transfer_between(
            AccountRef::Store(from.into()),
            AccountRef::Store(to.into()),
            usd(),
            sum,
        )
    };
    crate::usecases::money::transactions::add_transaction(&mut vault, side(&card, &bank, "500"))
        .unwrap();
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        payment(&card, "200", "shop"),
    )
    .unwrap();
    let mut interest = payment(&card, "10", "interest");
    interest.meta_mut().categories = vec!["loan_interest".into()];
    crate::usecases::money::transactions::add_transaction(&mut vault, interest).unwrap();
    crate::usecases::money::transactions::add_transaction(&mut vault, side(&bank, &card, "300"))
        .unwrap();

    for iso in ["2026-01-15T09:00:00Z", "2026-02-20T12:00:00Z"] {
        crate::usecases::workflow::runs::run_due_workflows(&mut vault, secs(iso), "Europe/Berlin")
            .unwrap();
    }
    assert_eq!(planned_of_bookings(&vault).len(), 2);

    let worth = crate::usecases::money::capital::net_worth(&vault.books());
    assert_eq!(worth.financial, amount("600"));
    assert_eq!(worth.liabilities, amount("410"));
    assert_eq!(worth.net, amount("190"));
    let budget = crate::planning::daily_budget::daily_budget(
        &vault.books(),
        secs("2026-02-20T12:00:00Z"),
        "Europe/Berlin",
    );
    // Feb has no income and the 300 rent is mandatory: -300 over 28 days,
    // 20 days elapsed incl. today -> -214.2857... (the code's figure today).
    assert_eq!(budget.fixed, amount("300"));
    assert_eq!(budget.days, 28);
    assert_eq!(budget.spent_today, amount("0"));
    assert_eq!(budget.left_today.round(2), amount("-214.29"));
}

fn bookings(vault: &UnlockedVault) -> usize {
    planned_of_bookings(vault).len()
}

fn pass_at(vault: &mut UnlockedVault, iso: &str, zone: &str) -> usize {
    crate::usecases::workflow::runs::run_due_workflows(vault, secs(iso), zone)
        .unwrap()
        .created
        .len()
}

/// Red before the fix: another device zone on a later tick booked the same date again.
#[test]
fn a_zone_change_does_not_book_a_date_twice() {
    let (_dir, mut vault) = empty_vault();
    berlin_rent(&mut vault, "1");
    // Bangkok 15 Mar 09:00 = 02:00Z; Jan and Feb are caught up with it.
    assert_eq!(
        pass_at(&mut vault, "2026-03-15T03:00:00Z", "Asia/Bangkok"),
        3
    );
    assert_eq!(
        planned_of_bookings(&vault).last().unwrap(),
        "2026-03-15T02:00:00Z"
    );
    assert_eq!(
        pass_at(&mut vault, "2026-03-20T12:00:00Z", "Europe/Moscow"),
        0
    );
    assert_eq!(bookings(&vault), 3);
}

/// The sync-merge path pins another zone after the fire; the date is still booked once.
#[test]
fn a_pinned_zone_changed_by_sync_does_not_book_a_date_twice() {
    let (_dir, mut vault) = empty_vault();
    berlin_rent(&mut vault, "1");
    assert_eq!(
        pass_at(&mut vault, "2026-03-15T03:00:00Z", "Asia/Bangkok"),
        3
    );
    vault.doc.settings.schedule_timezone = Some("Europe/Moscow".into());
    assert_eq!(
        pass_at(&mut vault, "2026-03-20T12:00:00Z", "Asia/Bangkok"),
        0
    );
    assert_eq!(bookings(&vault), 3);
}

/// Red before the fix: the moment's date in the new zone (the 16th) was
/// booked instead of the stored one (the 15th), so the 16th was lost.
#[test]
fn a_zone_moving_east_across_midnight_still_books_the_next_day() {
    let (_dir, mut vault) = empty_vault();
    let account = store(&mut vault, "Cash", StoreKind::Cash, None);
    let mut funding = income(&account, "1000", "float");
    at(&mut funding, "2026-01-01T00:00:00Z");
    crate::usecases::money::transactions::add_transaction(&mut vault, funding).unwrap();
    crate::usecases::planning::automations::create_automation(
        &mut vault,
        AutomationDraft {
            time: Some("23:30".into()),
            starts_at: "2026-03-14T00:00:00Z".into(),
            schedule: Schedule::EveryNDays { days: 1 },
            ..monthly_draft("Late", 1, rent_draft(&account, "1"))
        },
    )
    .unwrap();
    // 23:30 Moscow on the 14th and 15th is 20:30Z.
    assert_eq!(
        pass_at(&mut vault, "2026-03-15T21:00:00Z", "Europe/Moscow"),
        2
    );
    // 20:30Z on the 15th is 03:30 on the 16th in Bangkok.
    vault.doc.settings.schedule_timezone = Some("Asia/Bangkok".into());
    assert_eq!(
        pass_at(&mut vault, "2026-03-16T20:00:00Z", "Asia/Bangkok"),
        1
    );
}

#[test]
fn a_record_from_before_planned_date_falls_back_to_planned_at() {
    let (_dir, mut vault) = empty_vault();
    berlin_rent(&mut vault, "1");
    assert_eq!(
        pass_at(&mut vault, "2026-03-15T03:00:00Z", "Asia/Bangkok"),
        3
    );
    for tx in &mut vault.doc.transactions {
        if let TxOrigin::Workflow { planned_date, .. } = &mut tx.meta_mut().origin {
            *planned_date = None;
        }
    }
    vault.doc.settings.schedule_timezone = Some("Europe/Moscow".into());
    assert_eq!(
        pass_at(&mut vault, "2026-03-20T12:00:00Z", "Asia/Bangkok"),
        0
    );
}

#[test]
fn a_booking_carries_the_planned_local_date() {
    let (_dir, mut vault) = empty_vault();
    berlin_rent(&mut vault, "1");
    pass_at(&mut vault, "2026-01-15T09:00:00Z", "Europe/Berlin");
    let dates: Vec<_> = vault
        .doc
        .transactions
        .iter()
        .filter_map(|tx| match &tx.meta().origin {
            TxOrigin::Workflow { planned_date, .. } => *planned_date,
            _ => None,
        })
        .collect();
    assert_eq!(
        dates,
        [crate::time::LocalDate::from_ymd_opt(2026, 1, 15).unwrap()]
    );
    let runs: Vec<_> = vault
        .doc
        .workflow_runs
        .iter()
        .filter_map(|run| run.planned_date)
        .collect();
    assert_eq!(runs, dates);
}

/// Red before the fix: a time edited later made the same date fresh again.
#[test]
fn a_time_moved_later_does_not_book_the_date_again() {
    let (_dir, mut vault) = empty_vault();
    let rent = berlin_rent(&mut vault, "1");
    let zone = "Europe/Berlin";
    assert_eq!(pass_at(&mut vault, "2026-03-15T08:30:00Z", zone), 3);
    let mut draft = monthly_draft("Rent", 15, rent.template.clone());
    draft.time = Some("18:00".into());
    crate::usecases::planning::automations::update_automation_at(
        &mut vault,
        &rent.id,
        draft,
        secs("2026-03-15T08:31:00Z"),
    )
    .unwrap();
    assert_eq!(pass_at(&mut vault, "2026-03-15T20:00:00Z", zone), 0);
    assert_eq!(bookings(&vault), 3);
    // 15 Apr 18:00 CEST = 16:00Z books normally.
    assert_eq!(pass_at(&mut vault, "2026-04-15T16:30:00Z", zone), 1);
}

#[test]
fn a_start_mark_is_not_a_processed_date() {
    let (_dir, mut vault) = empty_vault();
    let account = store(&mut vault, "Cash", StoreKind::Cash, None);
    let mut draft = monthly_draft("Rent", 15, salary_draft(&account, "1"));
    draft.starts_at = "2026-03-15T09:00:00Z".into(); // 10:00 Berlin
    draft.time = Some("18:00".into());
    crate::usecases::planning::automations::create_automation(&mut vault, draft).unwrap();
    assert_eq!(
        pass_at(&mut vault, "2026-03-15T17:05:00Z", "Europe/Berlin"),
        1
    );
}

/// Open question 4: an edited day books nothing back-dated.
#[test]
fn a_day_moved_later_does_not_book_the_new_day_back() {
    let (_dir, mut vault) = empty_vault();
    let rent = berlin_rent(&mut vault, "1");
    let zone = "Europe/Berlin";
    assert_eq!(pass_at(&mut vault, "2026-03-15T08:30:00Z", zone), 3);
    let draft = monthly_draft("Rent", 18, rent.template.clone());
    crate::usecases::planning::automations::update_automation_at(
        &mut vault,
        &rent.id,
        draft,
        secs("2026-03-20T12:00:00Z"),
    )
    .unwrap();
    assert_eq!(pass_at(&mut vault, "2026-03-20T13:00:00Z", zone), 0);
    assert_eq!(bookings(&vault), 3);
    assert_eq!(pass_at(&mut vault, "2026-04-18T12:00:00Z", zone), 1);
}

fn daily_at_0230(vault: &mut UnlockedVault, starts_at: &str) {
    let card = store(vault, "Card", StoreKind::Bank, None);
    let mut draft = monthly_draft("Daily", 1, salary_draft(&card, "1"));
    draft.schedule = Schedule::EveryNDays { days: 1 };
    draft.starts_at = starts_at.into();
    draft.time = Some("02:30".into());
    crate::usecases::planning::automations::create_automation(vault, draft).unwrap();
}

#[test]
fn dst_gap_books_the_date_once() {
    let (_dir, mut vault) = empty_vault();
    daily_at_0230(&mut vault, "2026-03-28T23:00:00Z");
    assert_eq!(
        pass_at(&mut vault, "2026-03-29T01:35:00Z", "Europe/Berlin"),
        1
    );
    assert_eq!(
        pass_at(&mut vault, "2026-03-29T05:00:00Z", "Europe/Berlin"),
        0
    );
    assert_eq!(bookings(&vault), 1);
}

#[test]
fn dst_overlap_books_the_date_once() {
    let (_dir, mut vault) = empty_vault();
    daily_at_0230(&mut vault, "2026-10-24T23:00:00Z");
    assert_eq!(
        pass_at(&mut vault, "2026-10-25T00:35:00Z", "Europe/Berlin"),
        1
    );
    assert_eq!(
        pass_at(&mut vault, "2026-10-25T01:40:00Z", "Europe/Berlin"),
        0
    );
    assert_eq!(bookings(&vault), 1);
}

#[test]
fn the_clock_form_and_the_wrapper_give_the_same_pass() {
    let tick = "2026-03-15T08:30:00Z";
    let (_dir_a, mut wrapped) = empty_vault();
    berlin_rent(&mut wrapped, "1");
    let (_dir_b, mut clocked) = empty_vault();
    berlin_rent(&mut clocked, "1");
    let left = crate::usecases::workflow::runs::run_due_workflows(
        &mut wrapped,
        secs(tick),
        "Europe/Berlin",
    )
    .unwrap();
    let right = crate::usecases::workflow::runs::run_due_workflows_at(
        &mut clocked,
        &crate::time::FixedClock(secs(tick)),
        "Europe/Berlin",
    )
    .unwrap();
    assert_eq!(left.created.len(), right.created.len());
    assert_eq!(planned_of_bookings(&wrapped), planned_of_bookings(&clocked));
}

// -- the pure functions on a bare document and the delegate pins ------------------

#[test]
fn a_draft_is_checked_without_a_vault() {
    let template = salary_draft("x", "100");
    let mut draft = monthly_draft("Salary", 31, template);
    assert!(check_draft_form(&draft).is_ok());
    assert!(check_draft_moment(&draft).is_ok());
    draft.name = "  ".into();
    assert!(check_draft_form(&draft).is_err());
    draft.name = "Salary".into();
    draft.schedule = Schedule::Monthly { day: 32 };
    assert!(check_draft_form(&draft).is_err());
    draft.starts_at = "soon".into();
    assert!(check_draft_moment(&draft).is_err());
}

#[test]
fn the_funds_check_reads_a_bare_document_and_the_delegates_agree() {
    let (_dir, mut vault) = empty_vault();
    let cash = store(&mut vault, "Cash", StoreKind::Cash, None);
    let over = draft_to_transaction(&rent_draft(&cash, "50"), "a", "", "2026-01-01T09:00:00Z");
    let secs = secs("2026-01-01T09:00:00Z");
    let reason = funds_shortfall(&vault.doc, &over, secs);
    assert!(matches!(
        reason,
        Some(RejectReason::InsufficientFunds { .. })
    ));
    assert_eq!(
        reason,
        crate::planning::automations::funds_shortfall(&vault.doc, &over, secs)
    );
    let key = crate::model::store::AssetRef::fiat("USD", "USD").key();
    assert_eq!(
        balance_at(&vault.doc, &cash, &key, secs),
        vault.balance_at(&cash, &key, secs)
    );
    let text = describe_shortfall(&vault.doc, &reason.unwrap());
    assert!(text.starts_with("Cash:"), "{text}");
}

#[test]
fn the_view_and_the_occurrences_need_no_vault() {
    let (_dir, mut vault) = empty_vault();
    let cash = store(&mut vault, "Cash", StoreKind::Cash, None);
    let automation = monthly(&mut vault, "Rent", 5, rent_draft(&cash, "90"));
    assert_eq!(automation_draft(&automation), automation.template);
    assert_eq!(
        automation_draft(&automation),
        vault.automation_draft(&automation)
    );
    let cal = Calendar::new("UTC");
    let after = secs("2026-01-01T00:00:00Z");
    let moments = automation_occurrences(&automation, &cal, after, after + 70 * 86_400);
    assert_eq!(moments.len(), 3);
    let delegate = vault.automation_occurrences(&automation, &cal, after, after + 70 * 86_400);
    let days =
        |planned: &[(i64, Transaction)]| planned.iter().map(|(day, _)| *day).collect::<Vec<_>>();
    assert_eq!(days(&moments), days(&delegate));
    assert!(automation_runs(&vault.books(), None).is_empty());
}

// -- one test per pure function on a bare document, and the family pins ------------

/// A result as JSON with every id and timestamp blanked, so twin vaults compare.
pub(super) fn without_ids<T: serde::Serialize>(value: &T, known_ids: &[&str]) -> serde_json::Value {
    fn blank(node: &mut serde_json::Value, known_ids: &[&str]) {
        match node {
            serde_json::Value::Object(fields) => {
                for (key, child) in fields.iter_mut() {
                    if key == "id"
                        || key.ends_with("_id")
                        || key.ends_with("_at")
                        || key.contains("timestamp")
                    {
                        *child = serde_json::Value::Null;
                    } else {
                        blank(child, known_ids);
                    }
                }
            }
            serde_json::Value::Array(items) => {
                for item in items {
                    blank(item, known_ids);
                }
            }
            serde_json::Value::String(text) if known_ids.contains(&text.as_str()) => {
                *text = String::new();
            }
            _ => {}
        }
    }
    let mut json = serde_json::to_value(value).unwrap();
    blank(&mut json, known_ids);
    json
}

fn bare_workflow(draft: &AutomationDraft, id: &str) -> crate::workflow::model::Workflow {
    new_automation_workflow(draft, id, "2026-01-01T00:00:00Z").unwrap()
}

fn varying_draft() -> AutomationDraft {
    AutomationDraft {
        amount_mode: AmountMode::Varies,
        ..monthly_draft("Pay", 5, salary_draft("wallet", "100"))
    }
}

#[test]
fn a_new_workflow_is_the_trimmed_draft_with_its_graph() {
    let mut draft = monthly_draft("  Rent ", 5, rent_draft("wallet", "90"));
    draft.active = false;
    let workflow = bare_workflow(&draft, "wf");
    assert_eq!(
        (workflow.id.as_str(), workflow.name.as_str()),
        ("wf", "Rent")
    );
    assert!(!workflow.active);
    assert_eq!(workflow.created_at, "2026-01-01T00:00:00Z");
    assert_eq!(workflow.automation, Some(draft.spec()));
    assert!(workflow.graph.contains("trigger.schedule"));
    assert!(workflow.versions.is_empty());
}

#[test]
fn rewriting_keeps_the_old_graph_as_a_version_only_when_it_changed() {
    let draft = monthly_draft("Rent", 5, rent_draft("wallet", "90"));
    let mut workflow = bare_workflow(&draft, "wf");
    let old_graph = workflow.graph.clone();
    rewrite_automation(&mut workflow, &draft, "2026-02-01T00:00:00Z").unwrap();
    assert!(workflow.versions.is_empty(), "same graph, no version");
    assert_eq!(workflow.updated_at, "2026-02-01T00:00:00Z");
    let moved = AutomationDraft {
        name: " Rent 2 ".into(),
        schedule: Schedule::Monthly { day: 9 },
        ..draft
    };
    rewrite_automation(&mut workflow, &moved, "2026-03-01T00:00:00Z").unwrap();
    assert_eq!(workflow.versions.len(), 1);
    assert_eq!(workflow.versions[0].graph, old_graph);
    assert_eq!(workflow.versions[0].saved_at, "2026-03-01T00:00:00Z");
    assert_eq!(workflow.name, "Rent 2");
    assert_eq!(workflow.automation, Some(moved.spec()));
}

#[test]
fn a_workflow_is_an_automation_and_an_ordinary_one_is_not() {
    let draft = varying_draft();
    let mut workflow = bare_workflow(&draft, "wf");
    let asked = std::cell::Cell::new(0);
    let view = automation_of(
        &workflow,
        |id| {
            asked.set(asked.get() + 1);
            assert_eq!(id, "wf");
            Some(amount("1300"))
        },
        Some("2026-02-05T09:00:00Z".into()),
    )
    .unwrap();
    assert_eq!(view.estimate, Some(amount("1300")));
    assert_eq!(
        view.last_planned_at.as_deref(),
        Some("2026-02-05T09:00:00Z")
    );
    assert_eq!(asked.get(), 1);
    // The start mark is no processed date; a fixed amount asks for no average.
    let fixed = bare_workflow(&monthly_draft("Pay", 5, salary_draft("w", "1")), "fx");
    let mark = crate::workflow::automation::start_mark(fixed.automation.as_ref().unwrap());
    let view = automation_of(&fixed, |_| panic!("not asked"), Some(mark)).unwrap();
    assert_eq!((view.estimate, view.last_planned_at), (None, None));
    workflow.automation = None;
    assert!(automation_of(&workflow, |_| None, None).is_none());
}

#[test]
fn a_manual_run_record_books_one_transaction_by_hand() {
    let record = manual_run_record("run", "wf", "2026-01-05T09:00:00Z", "tx");
    assert_eq!(record.id, "run");
    assert_eq!(record.workflow_id, "wf");
    assert_eq!(record.trigger, "manual 2026-01-05T09:00:00Z");
    assert_eq!(record.created, vec!["tx".to_string()]);
    assert!(matches!(record.outcome, RunOutcome::Done));
    assert!(!record.dry_run);
}

#[test]
fn a_run_reads_as_the_history_shows_it() {
    let mut doc = crate::model::vault::VaultDoc::default();
    let mut booked = income("wallet", "100", "salary");
    booked.meta_mut().id = "tx".into();
    doc.transactions.push(booked);
    let record = manual_run_record("run", "wf", "2026-01-05T09:00:00Z", "tx");
    let run = automation_run_of(&doc, &record).unwrap();
    assert_eq!(run.automation_id, "wf");
    assert_eq!(
        run.outcome,
        AutomationOutcome::Created { tx_id: "tx".into() }
    );
    assert_eq!(run.amount, Some(amount("100")));
    assert_eq!(run.planned_at, "2026-01-05T09:00:00Z");
    let mut skipped = record.clone();
    skipped.outcome = RunOutcome::Dropped {
        reason: "skipped".into(),
    };
    assert_eq!(
        automation_run_of(&doc, &skipped).unwrap().outcome,
        AutomationOutcome::Skipped
    );
    skipped.outcome = RunOutcome::Dropped {
        reason: "other".into(),
    };
    assert!(automation_run_of(&doc, &skipped).is_none());
}

#[test]
fn a_manual_transaction_carries_its_run_and_the_estimate_mark() {
    let (_dir, mut vault) = empty_vault();
    let wallet = store(&mut vault, "Wallet", StoreKind::Bank, None);
    let draft = AutomationDraft {
        amount_mode: AmountMode::Varies,
        ..monthly_draft("Pay", 5, salary_draft(&wallet, "100"))
    };
    let automation =
        crate::usecases::planning::automations::create_automation(&mut vault, draft).unwrap();
    let made = manual_run_transaction(&automation, "2026-01-05T09:00:00Z", "run-1");
    let TxOrigin::Workflow {
        estimated, run_id, ..
    } = made.meta().origin.clone()
    else {
        panic!("not a workflow origin");
    };
    assert!(estimated);
    assert_eq!(run_id, "run-1");
    assert_eq!(made.meta().timestamp, "2026-01-05T09:00:00Z");
}

#[test]
fn an_approximate_operation_is_confirmed_and_its_mark_cleared() {
    let mut doc = crate::model::vault::VaultDoc::default();
    let workflow = bare_workflow(&varying_draft(), "wf");
    let automation = automation_of(&workflow, |_| None, None).unwrap();
    let mut estimated = manual_run_transaction(&automation, "2026-01-05T09:00:00Z", "run");
    estimated.meta_mut().id = "tx".into();
    doc.transactions.push(estimated);
    assert!(confirmed_transaction(&doc, "tx", crate::Amount::ZERO).is_err());
    assert!(confirmed_transaction(&doc, "nope", amount("5")).is_err());
    let mut corrected = confirmed_transaction(&doc, "tx", amount("130")).unwrap();
    assert_eq!(main_amount(&corrected), Some(amount("130")));
    assert!(clear_estimated(&mut corrected));
    assert!(!clear_estimated(&mut corrected), "already clear");
    doc.transactions[0] = corrected;
    assert!(confirmed_transaction(&doc, "tx", amount("1")).is_err());
}

#[test]
fn only_a_change_of_when_it_plans_changes_the_schedule() {
    let draft = monthly_draft("Rent", 5, rent_draft("wallet", "90"));
    let mut doc = crate::model::vault::VaultDoc::default();
    doc.workflows.push(bare_workflow(&draft, "wf"));
    assert!(!schedule_changed(&doc, "wf", &draft).unwrap());
    let renamed = AutomationDraft {
        name: "Other".into(),
        ..draft.clone()
    };
    assert!(!schedule_changed(&doc, "wf", &renamed).unwrap());
    let later = AutomationDraft {
        time: Some("10:30".into()),
        ..draft.clone()
    };
    assert!(schedule_changed(&doc, "wf", &later).unwrap());
    assert!(schedule_changed(&doc, "none", &draft).is_err());
}

#[test]
fn an_emulation_report_lists_only_the_new_runs() {
    let mut doc = crate::model::vault::VaultDoc::default();
    let draft = monthly_draft("Pay", 5, salary_draft("wallet", "100"));
    doc.workflows.push(bare_workflow(&draft, "wf"));
    let mut booked = income("wallet", "100", "salary");
    booked.meta_mut().id = "tx".into();
    doc.transactions.push(booked);
    doc.workflow_runs
        .push(manual_run_record("old", "wf", "2026-01-01T09:00:00Z", "tx"));
    doc.workflow_runs
        .push(manual_run_record("new", "wf", "2026-01-05T09:00:00Z", "tx"));
    let known = HashSet::from(["old".to_string()]);
    let report = simulation_report(&doc, &known, &[], None, secs("2026-01-02T00:00:00Z"), 10);
    assert_eq!((report.created, report.rejected), (1, 0));
    assert_eq!(report.runs.len(), 1);
    assert_eq!(report.runs[0].automation_name, "Pay");
    assert_eq!(report.from, "2026-01-02T00:00:00Z");
    assert_eq!(report.until, "2026-01-12T00:00:00Z");
    let other = simulation_report(&doc, &known, &[], Some("zzz"), 0, 1);
    assert!(other.runs.is_empty());
}

#[test]
fn run_now_books_the_transaction_under_a_recorded_run() {
    let (_dir, mut vault) = empty_vault();
    let wallet = store(&mut vault, "Wallet", StoreKind::Bank, None);
    let pay = monthly(&mut vault, "Pay", 5, salary_draft(&wallet, "100"));
    let now = secs("2026-01-05T10:00:00Z");
    let saved =
        crate::usecases::planning::automations::run_automation(&mut vault, &pay.id, now).unwrap();
    let record = vault
        .doc
        .workflow_runs
        .iter()
        .find(|run| run.trigger.starts_with("manual "))
        .expect("a run record");
    let TxOrigin::Workflow { run_id, .. } = &saved.meta().origin else {
        panic!("not a workflow origin");
    };
    assert_eq!(&record.id, run_id);
    assert_eq!(record.created, vec![saved.meta().id.clone()]);
    assert_eq!(record.trigger, format!("manual {}", calendar::iso(now)));
    assert!(
        vault
            .doc
            .transactions
            .iter()
            .any(|tx| tx.meta().id == saved.meta().id)
    );
}

#[test]
fn run_now_moves_the_holding_at_once() {
    let (_dir, mut vault) = empty_vault();
    let wallet = store(&mut vault, "Wallet", StoreKind::Bank, None);
    let pay = monthly(&mut vault, "Pay", 5, salary_draft(&wallet, "100"));
    let now = secs("2026-01-05T10:00:00Z");
    crate::usecases::planning::automations::run_automation(&mut vault, &pay.id, now).unwrap();
    assert_eq!(holding_of(&vault, &wallet, &usd()), amount("100"));
}

#[test]
fn run_now_writes_the_rebased_corrections_with_its_commit() {
    use crate::model::transaction::{AdjustmentLine, AdjustmentTx, TxMeta};
    use crate::transaction::PriceStamp;
    let (_dir, mut vault) = empty_vault();
    let wallet = store(&mut vault, "Wallet", StoreKind::Bank, None);
    let pay = monthly(&mut vault, "Pay", 5, salary_draft(&wallet, "100"));
    let now = secs("2026-01-05T10:00:00Z");
    let mut meta = TxMeta::new("Reconcile");
    meta.timestamp = calendar::iso(now + 3600);
    let correction = Transaction::Adjustment(AdjustmentTx {
        meta,
        store_id: wallet.clone(),
        lines: vec![AdjustmentLine {
            asset: usd(),
            before: amount("0"),
            after: amount("150"),
            price: PriceStamp::default(),
        }],
        fee: None,
    });
    crate::usecases::money::transactions::add_transaction(&mut vault, correction).unwrap();
    crate::usecases::planning::automations::run_automation(&mut vault, &pay.id, now).unwrap();
    let reopened = UnlockedVault::open(vault.path.clone(), "pw").unwrap();
    let stored = reopened
        .doc
        .transactions
        .iter()
        .find_map(|tx| match tx {
            Transaction::Adjustment(inner) => Some(inner.lines[0].before),
            _ => None,
        })
        .expect("the correction");
    assert_eq!(stored, amount("100"), "rebased before the commit wrote it");
}

#[test]
fn run_now_without_funds_is_refused_and_leaves_no_run() {
    let (_dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    let rent = monthly(&mut vault, "Rent", 5, rent_draft(&card, "800"));
    let err = crate::usecases::planning::automations::run_automation(
        &mut vault,
        &rent.id,
        secs("2026-01-05T10:00:00Z"),
    )
    .unwrap_err();
    assert_eq!(err.code(), "INSUFFICIENT_FUNDS");
    assert!(vault.doc.workflow_runs.is_empty());
    assert!(vault.doc.transactions.is_empty());
}

fn twin_vaults() -> (
    (tempfile::TempDir, UnlockedVault, String),
    (tempfile::TempDir, UnlockedVault, String),
) {
    let build = || {
        let (dir, mut vault) = empty_vault();
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        (dir, vault, card)
    };
    (build(), build())
}

#[test]
fn the_edit_delegates_equal_the_scenarios() {
    use crate::usecases::planning::automations as scenario;
    let ((_a, mut by_method, card_a), (_b, mut by_scenario, card_b)) = twin_vaults();
    let draft = |card: &str| monthly_draft("Pay", 5, salary_draft(card, "100"));
    let method =
        crate::usecases::planning::automations::create_automation(&mut by_method, draft(&card_a))
            .unwrap();
    let made = scenario::create_automation(&mut by_scenario, draft(&card_b)).unwrap();
    let ids = |automation: &Automation| vec![automation.id.clone()];
    assert_eq!(
        without_ids(&method, &[&ids(&method)[0], &card_a]),
        without_ids(&made, &[&ids(&made)[0], &card_b])
    );
    let renamed = |card: &str| AutomationDraft {
        name: "Pay 2".into(),
        ..draft(card)
    };
    let method = crate::usecases::planning::automations::update_automation_at(
        &mut by_method,
        &method.id,
        renamed(&card_a),
        0,
    )
    .unwrap();
    let made =
        scenario::update_automation_at(&mut by_scenario, &made.id, renamed(&card_b), 0).unwrap();
    assert_eq!(
        without_ids(&method, &[&method.id, &card_a]),
        without_ids(&made, &[&made.id, &card_b])
    );
    let method_flow =
        crate::usecases::planning::automations::detach_automation(&mut by_method, &method.id)
            .unwrap();
    let made_flow = scenario::detach_automation(&mut by_scenario, &made.id).unwrap();
    let shape = |flow: &crate::workflow::model::Workflow| {
        (
            flow.name.clone(),
            flow.automation.is_none(),
            flow.versions.len(),
        )
    };
    assert_eq!(shape(&method_flow), shape(&made_flow));
    assert_eq!(
        crate::usecases::planning::automations::automations(&by_method).len(),
        crate::usecases::planning::automations::automations(&by_scenario).len()
    );
}

#[test]
fn the_delete_delegate_equals_the_scenario() {
    use crate::usecases::planning::automations as scenario;
    let ((_a, mut by_method, card_a), (_b, mut by_scenario, card_b)) = twin_vaults();
    let method = crate::usecases::planning::automations::create_automation(
        &mut by_method,
        monthly_draft("Pay", 5, salary_draft(&card_a, "100")),
    )
    .unwrap();
    let made = scenario::create_automation(
        &mut by_scenario,
        monthly_draft("Pay", 5, salary_draft(&card_b, "100")),
    )
    .unwrap();
    crate::usecases::planning::automations::delete_automation(&mut by_method, &method.id).unwrap();
    scenario::delete_automation(&mut by_scenario, &made.id).unwrap();
    assert!(by_method.doc.workflows.is_empty() && by_scenario.doc.workflows.is_empty());
    assert!(
        crate::usecases::planning::automations::delete_automation(&mut by_method, &method.id)
            .is_err()
    );
    assert!(scenario::delete_automation(&mut by_scenario, &made.id).is_err());
}

#[test]
fn the_run_delegate_equals_the_scenario() {
    use crate::usecases::planning::automations as scenario;
    let ((_a, mut by_method, card_a), (_b, mut by_scenario, card_b)) = twin_vaults();
    let method = crate::usecases::planning::automations::create_automation(
        &mut by_method,
        monthly_draft("Pay", 5, salary_draft(&card_a, "100")),
    )
    .unwrap();
    let made = scenario::create_automation(
        &mut by_scenario,
        monthly_draft("Pay", 5, salary_draft(&card_b, "100")),
    )
    .unwrap();
    let now = secs("2026-01-05T10:00:00Z");
    let by_hand =
        crate::usecases::planning::automations::run_automation(&mut by_method, &method.id, now)
            .unwrap();
    let by_case = scenario::run_automation(&mut by_scenario, &made.id, now).unwrap();
    assert_eq!(
        without_ids(&by_hand, &[&method.id, &card_a]),
        without_ids(&by_case, &[&made.id, &card_b])
    );
    assert_eq!(
        by_method.doc.workflow_runs.len(),
        by_scenario.doc.workflow_runs.len()
    );
}

#[test]
fn the_retry_skip_and_confirm_delegates_equal_the_scenarios() {
    use crate::usecases::planning::automations as scenario;
    let ((_a, mut by_method, card_a), (_b, mut by_scenario, card_b)) = twin_vaults();
    let rent = |card: &str| monthly_draft("Rent", 5, rent_draft(card, "800"));
    let method =
        crate::usecases::planning::automations::create_automation(&mut by_method, rent(&card_a))
            .unwrap();
    let made = scenario::create_automation(&mut by_scenario, rent(&card_b)).unwrap();
    let due = secs("2026-02-20T12:00:00Z");
    crate::usecases::workflow::runs::run_due_workflows(&mut by_method, due, "UTC").unwrap();
    crate::usecases::workflow::runs::run_due_workflows(&mut by_scenario, due, "UTC").unwrap();
    let runs_a =
        crate::planning::automation_plan::automation_runs(&by_method.books(), Some(&method.id));
    let runs_b =
        crate::planning::automation_plan::automation_runs(&by_scenario.books(), Some(&made.id));
    assert_eq!(runs_a.len(), runs_b.len());
    let now = secs("2026-02-21T12:00:00Z");
    // Both refuse for lack of funds, then both are funded and retry.
    assert_eq!(
        crate::usecases::planning::automations::retry_automation_run(
            &mut by_method,
            &runs_a[0].id,
            now,
            "UTC"
        )
        .unwrap_err()
        .code(),
        scenario::retry_automation_run(&mut by_scenario, &runs_b[0].id, now, "UTC")
            .unwrap_err()
            .code()
    );
    for (vault, card) in [(&mut by_method, &card_a), (&mut by_scenario, &card_b)] {
        let mut salary = income(card, "1000", "salary");
        at(&mut salary, "2026-02-21T10:00:00Z");
        crate::usecases::money::transactions::add_transaction(vault, salary).unwrap();
    }
    let retried = crate::usecases::planning::automations::retry_automation_run(
        &mut by_method,
        &runs_a[0].id,
        now,
        "UTC",
    )
    .unwrap();
    let retried_by_case =
        scenario::retry_automation_run(&mut by_scenario, &runs_b[0].id, now, "UTC").unwrap();
    assert_eq!(retried.meta().timestamp, retried_by_case.meta().timestamp);
    let skipped =
        crate::usecases::planning::automations::skip_automation_run(&mut by_method, &runs_a[1].id)
            .unwrap();
    let skipped_by_case = scenario::skip_automation_run(&mut by_scenario, &runs_b[1].id).unwrap();
    assert_eq!(skipped.outcome, skipped_by_case.outcome);
}

#[test]
fn the_confirm_delegate_equals_the_scenario() {
    use crate::usecases::planning::automations as scenario;
    let ((_a, mut by_method, card_a), (_b, mut by_scenario, card_b)) = twin_vaults();
    let varying = |card: &str| AutomationDraft {
        amount_mode: AmountMode::Varies,
        workday_shift: WorkdayShift::Back,
        ..monthly_draft("Pay", 20, salary_draft(card, "1000"))
    };
    crate::usecases::planning::automations::create_automation(&mut by_method, varying(&card_a))
        .unwrap();
    scenario::create_automation(&mut by_scenario, varying(&card_b)).unwrap();
    let due = secs("2026-02-20T10:00:00Z");
    let booked = |vault: &mut UnlockedVault| {
        let pass = crate::usecases::workflow::runs::run_due_workflows(vault, due, "UTC").unwrap();
        made(vault, &pass).meta().id.clone()
    };
    let tx_a = booked(&mut by_method);
    let tx_b = booked(&mut by_scenario);
    let by_hand = crate::usecases::planning::automations::confirm_estimated_amount(
        &mut by_method,
        &tx_a,
        amount("1300"),
    )
    .unwrap();
    let by_case =
        scenario::confirm_estimated_amount(&mut by_scenario, &tx_b, amount("1300")).unwrap();
    assert_eq!(main_amount(&by_hand), main_amount(&by_case));
    assert_eq!(
        by_hand.meta().origin.is_estimated(),
        by_case.meta().origin.is_estimated()
    );
    assert!(!by_case.meta().origin.is_estimated());
}
