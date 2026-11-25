//! op-period T22: an automation run that refers to an operation template takes the template's
//! period rule and distribution rule, all in the one write of the run (T5, D5, D12, Q11, A4).

use crate::model::planning::{
    AmountMode, AutomationOutcome, RejectReason, Schedule, TransactionDraft,
};
use crate::model::store::StoreKind;
use crate::model::transaction::Transaction;
use crate::planning::automations::AutomationDraft;
use crate::planning::templates::TemplateDraft;
use crate::service::Session;
use crate::service::test_support::*;
use crate::time::unix_secs;
use crate::usecases::planning::automations::{
    confirm_distribution, create_automation, preview_distribution_rule, run_automation,
    simulate_automations, update_automation,
};
use crate::usecases::planning::templates::create_operation_template;
use crate::usecases::workflow::runs::run_due_workflows;
use mwt_model::{
    DistKind, DistTarget, DistributionRow, DistributionRule, OnOtherAmount, PeriodRule,
    TemplateKind,
};

fn row(kind: DistKind, target: DistTarget) -> DistributionRow {
    DistributionRow { kind, target }
}

fn envelope_on(vault: &mut Session, card: &str, name: &str) -> String {
    let draft = crate::planning::envelopes::EnvelopeDraft {
        store_id: card.into(),
        name: name.into(),
        asset: usd(),
        target: None,
        goal_id: None,
    };
    crate::usecases::household::envelopes::create_envelope(vault, draft)
        .unwrap()
        .id
}

/// A template with the advance rule: the first half of the month, then `rows` and the remainder.
fn template_with(vault: &mut Session, rows: Vec<DistributionRow>) -> String {
    template_on_other(vault, rows, OnOtherAmount::ShrinkRemainder)
}

fn template_on_other(
    vault: &mut Session,
    rows: Vec<DistributionRow>,
    on_other_amount: OnOtherAmount,
) -> String {
    let mut rows = rows;
    rows.push(row(DistKind::Remainder, DistTarget::None));
    create_operation_template(
        vault,
        TemplateDraft {
            name: "Advance".into(),
            kind: TemplateKind::Income,
            // Prefill for quick-add only: a run never takes them over its own draft.
            store_id: None,
            asset: Some(usd()),
            amount: Some(amount("99999")),
            categories: Vec::new(),
            comment: String::new(),
            contact_id: None,
            period_rule: Some(PeriodRule::ThisHalfFirst),
            distribution: Some(DistributionRule {
                rows,
                on_other_amount,
                keep_on_card_min: None,
            }),
        },
    )
    .unwrap()
    .id
}

/// The advance of 30 000 on the 20th, bound to the template when `template_id` is given.
fn advance(vault: &mut Session, card: &str, template_id: Option<&str>) -> String {
    advance_asking(vault, card, template_id, false)
}

fn advance_asking(
    vault: &mut Session,
    card: &str,
    template_id: Option<&str>,
    asks: bool,
) -> String {
    advance_with(vault, card, template_id, asks, Default::default())
}

fn advance_with(
    vault: &mut Session,
    card: &str,
    template_id: Option<&str>,
    asks: bool,
    amount_mode: AmountMode,
) -> String {
    let Transaction::Topup(inner) = income(card, "30000", "advance") else {
        unreachable!()
    };
    let made = create_automation(
        vault,
        AutomationDraft {
            name: "Advance".into(),
            active: true,
            preset: None,
            schedule: Schedule::Monthly { day: 20 },
            starts_at: "2026-11-01T00:00:00Z".into(),
            template: TransactionDraft::Topup(inner),
            amount_mode,
            workday_shift: Default::default(),
            property_id: None,
            time: None,
            asks,
            for_month_offset: Some(0),
        },
    )
    .unwrap();
    let workflow = vault
        .doc
        .workflows
        .iter_mut()
        .find(|workflow| workflow.id == made.id)
        .unwrap();
    workflow.automation.as_mut().unwrap().operation_template_id = template_id.map(Into::into);
    made.id
}

fn run_on_20_nov(vault: &mut Session) -> crate::workflow::service::runs::WorkflowPass {
    let now = unix_secs("2026-11-20T12:00:00Z").unwrap();
    run_due_workflows(vault, now, "UTC").unwrap()
}

fn setup() -> (tempfile::TempDir, Session, String, String) {
    let (dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    let vacation = envelope_on(&mut vault, &card, "Vacation");
    (dir, vault, card, vacation)
}

fn five_percent(vacation: &str) -> DistributionRow {
    row(
        DistKind::Percent { pct: amount("5") },
        DistTarget::Envelope {
            id: vacation.into(),
        },
    )
}

#[test]
fn a4_the_advance_takes_the_period_and_the_distribution_in_one_commit() {
    let (_dir, mut vault, card, vacation) = setup();
    let template = template_with(&mut vault, vec![five_percent(&vacation)]);
    let id = advance(&mut vault, &card, Some(&template));
    let revision = vault.session.revision();
    let pass = run_on_20_nov(&mut vault);
    let delta = vault.session.revision() - revision;
    assert_eq!(pass.created.len(), 1);
    let income = vault
        .doc
        .transactions
        .iter()
        .find(|tx| tx.meta().id == pass.created[0])
        .unwrap();
    let Transaction::Topup(topup) = income else {
        panic!("an income")
    };
    // The embedded draft books, whatever the template prefill says.
    assert_eq!(topup.amount, amount("30000"));
    assert_eq!(topup.store_id, card);
    let period = topup.meta.period.as_ref().expect("a period");
    assert_eq!(
        (period.start.as_str(), period.end.as_str()),
        ("2026-11-01", "2026-11-15")
    );
    let parts: Vec<_> = topup
        .distribution
        .as_ref()
        .unwrap()
        .iter()
        .map(|part| part.amount)
        .collect();
    assert_eq!(parts, vec![amount("1500"), amount("28500")]);
    let envelope = vault
        .doc
        .envelopes
        .iter()
        .find(|envelope| envelope.id == vacation)
        .unwrap();
    assert_eq!(envelope.allocated, amount("1500"));
    assert_eq!(holding_of(&vault, &card, &usd()), amount("30000"));
    // The old label is still written, never read.
    assert_eq!(topup.details.for_month.as_deref(), Some("2026-11"));
    assert!(matches!(
        crate::planning::automation_plan::automation_runs(&vault.books(), Some(&id))[0].outcome,
        AutomationOutcome::Created { .. }
    ));
    // The run, the income and the distribution are one write of the pass.
    let (_other, mut plain, plain_card, _) = setup();
    advance(&mut plain, &plain_card, None);
    let before = plain.session.revision();
    run_on_20_nov(&mut plain);
    assert_eq!(delta, plain.session.revision() - before);
}

#[test]
fn without_a_template_the_run_books_the_income_alone() {
    let (_dir, mut vault, card, _vacation) = setup();
    advance(&mut vault, &card, None);
    let pass = run_on_20_nov(&mut vault);
    let income = vault
        .doc
        .transactions
        .iter()
        .find(|tx| tx.meta().id == pass.created[0])
        .unwrap();
    assert!(income.meta().period.is_none());
    assert!(
        vault
            .doc
            .envelopes
            .iter()
            .all(|envelope| envelope.allocated == amount("0"))
    );
}

#[test]
fn missing_free_balance_rejects_the_whole_run() {
    let (_dir, mut vault, card, vacation) = setup();
    // The card holds 8 000 but 28 000 of it is reserved: after the advance only 10 000 is free,
    // and the 25 000 row cannot be funded.
    add_transaction_plain(&mut vault, &card, "8000");
    let reserve = envelope_on(&mut vault, &card, "Reserve");
    vault
        .doc
        .envelopes
        .iter_mut()
        .find(|envelope| envelope.id == reserve)
        .unwrap()
        .record_move(amount("28000"), "2026-11-02T00:00:00Z");
    let rows = vec![row(
        DistKind::Sum {
            amount: amount("25000"),
        },
        DistTarget::Envelope { id: vacation },
    )];
    let template = template_with(&mut vault, rows);
    let id = advance(&mut vault, &card, Some(&template));
    let before = vault.doc.transactions.len();
    run_on_20_nov(&mut vault);
    assert_eq!(vault.doc.transactions.len(), before);
    let runs = crate::planning::automation_plan::automation_runs(&vault.books(), Some(&id));
    assert!(matches!(
        runs[0].outcome,
        AutomationOutcome::Rejected {
            reason: RejectReason::Invalid { .. }
        }
    ));
    assert_eq!(holding_of(&vault, &card, &usd()), amount("8000"));
}

fn add_transaction_plain(vault: &mut Session, card: &str, raw: &str) {
    let mut tx = income(card, raw, "seed");
    at(&mut tx, "2026-11-02T00:00:00Z");
    crate::usecases::money::transactions::add_transaction(vault, tx).unwrap();
}

#[test]
fn over_books_the_income_alone_and_raises_the_notice() {
    let (_dir, mut vault, card, vacation) = setup();
    let rows = vec![row(
        DistKind::Sum {
            amount: amount("40000"),
        },
        DistTarget::Envelope { id: vacation },
    )];
    let template = template_with(&mut vault, rows);
    let id = advance(&mut vault, &card, Some(&template));
    let pass = run_on_20_nov(&mut vault);
    assert_eq!(pass.created.len(), 1);
    let income = vault
        .doc
        .transactions
        .iter()
        .find(|tx| tx.meta().id == pass.created[0])
        .unwrap();
    let Transaction::Topup(topup) = income else {
        panic!("an income")
    };
    assert!(topup.distribution.is_none());
    assert!(topup.meta.period.is_some());
    assert!(
        vault
            .doc
            .envelopes
            .iter()
            .all(|envelope| envelope.allocated == amount("0"))
    );
    let keys: Vec<&str> = pass
        .notices
        .iter()
        .map(|record| record.key.as_str())
        .collect();
    assert_eq!(keys.len(), 1, "{keys:?}");
    assert!(keys[0].starts_with("automation-undistributed:"), "{keys:?}");
    assert!(
        vault
            .doc
            .notification_log
            .iter()
            .all(|record| !record.key.starts_with("automation-rejected:")),
        "{:?}",
        vault.doc.notification_log
    );
    // The moment is done: a later pass says nothing again.
    let again = run_on(&mut vault, "2026-11-20T18:00:00Z");
    assert!(again.notices.is_empty(), "{:?}", again.notices);
    assert!(!id.is_empty());
}

fn day_before() -> i64 {
    unix_secs("2026-11-19T12:00:00Z").unwrap()
}

#[test]
fn a4_the_preview_of_the_next_advance_equals_the_booked_result() {
    let (_dir, mut vault, card, vacation) = setup();
    let template = template_with(&mut vault, vec![five_percent(&vacation)]);
    let id = advance(&mut vault, &card, Some(&template));
    let preview = preview_distribution_rule(&vault, &id, None, day_before()).unwrap();
    assert_eq!(preview.amount, amount("30000"));
    let period = preview.period.as_ref().expect("a period");
    assert_eq!(
        (period.start.as_str(), period.end.as_str()),
        ("2026-11-01", "2026-11-15")
    );
    let previewed: Vec<_> = preview.resolved.rows.iter().map(|row| row.amount).collect();
    assert_eq!(previewed, vec![amount("1500"), amount("28500")]);
    let pass = run_on_20_nov(&mut vault);
    let Some(Transaction::Topup(topup)) = vault
        .doc
        .transactions
        .iter()
        .find(|tx| tx.meta().id == pass.created[0])
    else {
        panic!("an income")
    };
    let booked: Vec<_> = topup
        .distribution
        .as_ref()
        .unwrap()
        .iter()
        .map(|part| part.amount)
        .collect();
    assert_eq!(previewed, booked);
    assert_eq!(topup.meta.period, preview.period);
}

#[test]
fn the_preview_resolves_another_amount_and_writes_nothing() {
    let (_dir, mut vault, card, vacation) = setup();
    let template = template_with(&mut vault, vec![five_percent(&vacation)]);
    let id = advance(&mut vault, &card, Some(&template));
    let before = vault.doc.clone();
    let revision = vault.session.revision();
    let preview =
        preview_distribution_rule(&vault, &id, Some(amount("31000")), day_before()).unwrap();
    let rows: Vec<_> = preview.resolved.rows.iter().map(|row| row.amount).collect();
    assert_eq!(rows, vec![amount("1550"), amount("29450")]);
    assert!(vault.doc == before);
    assert_eq!(vault.session.revision(), revision);
}

#[test]
fn a3_ask_books_nothing_until_confirmed_and_confirmation_distributes() {
    let (_dir, mut vault, card, vacation) = setup();
    let template = template_on_other(
        &mut vault,
        vec![five_percent(&vacation)],
        OnOtherAmount::Ask,
    );
    let id = advance_asking(&mut vault, &card, Some(&template), true);
    let preview =
        preview_distribution_rule(&vault, &id, Some(amount("31000")), day_before()).unwrap();
    assert!(preview.resolved.ask);
    assert!(preview.resolved.rows.is_empty());
    let pass = run_on_20_nov(&mut vault);
    assert!(pass.created.is_empty());
    assert!(vault.doc.transactions.is_empty());
    assert_eq!(vault.doc.workflow_waits.len(), 1);
    let run_id = vault.doc.workflow_waits[0].run_id.clone();
    let now = unix_secs("2026-11-20T13:00:00Z").unwrap();
    let income = confirm_distribution(
        &mut vault,
        &run_id,
        amount("31000"),
        Some(OnOtherAmount::ShrinkRemainder),
        now,
        "UTC",
    )
    .unwrap();
    let Transaction::Topup(topup) = income else {
        panic!("an income")
    };
    assert_eq!(topup.amount, amount("31000"));
    let parts: Vec<_> = topup
        .distribution
        .as_ref()
        .unwrap()
        .iter()
        .map(|part| part.amount)
        .collect();
    assert_eq!(parts, vec![amount("1550"), amount("29450")]);
    assert_eq!(holding_of(&vault, &card, &usd()), amount("31000"));
    assert!(vault.doc.workflow_waits.is_empty());
}

/// An advance whose rule asks; the 20 Nov run is done and waits on the question. Its run id.
fn waiting(vault: &mut Session, card: &str, rows: Vec<DistributionRow>) -> String {
    let template = template_on_other(vault, rows, OnOtherAmount::Ask);
    advance_asking(vault, card, Some(&template), true);
    run_on_20_nov(vault);
    assert_eq!(vault.doc.workflow_waits.len(), 1);
    vault.doc.workflow_waits[0].run_id.clone()
}

fn confirm_at_13(
    vault: &mut Session,
    run_id: &str,
    raw: &str,
    mode: Option<OnOtherAmount>,
) -> crate::error::Result<Transaction> {
    let now = unix_secs("2026-11-20T13:00:00Z").unwrap();
    confirm_distribution(vault, run_id, amount(raw), mode, now, "UTC")
}

/// A refused confirmation changes nothing: the question stays, the document is as it was.
fn assert_refused(vault: &mut Session, run_id: &str, raw: &str, mode: Option<OnOtherAmount>) {
    let before = vault.doc.clone();
    let revision = vault.session.revision();
    assert!(confirm_at_13(vault, run_id, raw, mode).is_err());
    assert!(vault.doc == before);
    assert_eq!(vault.doc.workflow_waits.len(), 1);
    assert_eq!(vault.session.revision(), revision);
}

#[test]
fn a_confirmation_that_still_asks_changes_nothing() {
    let (_dir, mut vault, card, vacation) = setup();
    let run_id = waiting(&mut vault, &card, vec![five_percent(&vacation)]);
    assert_refused(&mut vault, &run_id, "31000", None);
    let refusal = confirm_at_13(&mut vault, &run_id, "31000", None).unwrap_err();
    assert!(
        refusal.to_string().contains("the rule still asks"),
        "{refusal}"
    );
    assert_refused(&mut vault, &run_id, "31000", Some(OnOtherAmount::Ask));
    assert_refused(
        &mut vault,
        &run_id,
        "0",
        Some(OnOtherAmount::ShrinkRemainder),
    );
    // The same amount as expected resolves without a mode.
    let income = confirm_at_13(&mut vault, &run_id, "30000", None).unwrap();
    assert!(matches!(income, Transaction::Topup(ref topup) if topup.distribution.is_some()));
    assert!(vault.doc.workflow_waits.is_empty());
}

#[test]
fn a_confirmation_that_asks_too_much_changes_nothing() {
    let (_dir, mut vault, card, vacation) = setup();
    let rows = vec![row(
        DistKind::Sum {
            amount: amount("40000"),
        },
        DistTarget::Envelope { id: vacation },
    )];
    let run_id = waiting(&mut vault, &card, rows);
    assert_refused(
        &mut vault,
        &run_id,
        "31000",
        Some(OnOtherAmount::ShrinkRemainder),
    );
    let refusal = confirm_at_13(
        &mut vault,
        &run_id,
        "31000",
        Some(OnOtherAmount::ShrinkRemainder),
    )
    .unwrap_err();
    assert!(
        refusal
            .to_string()
            .contains(crate::usecases::planning::distribution::OVER_ALLOCATED),
        "{refusal}"
    );
    assert!(
        vault
            .doc
            .envelopes
            .iter()
            .all(|envelope| envelope.allocated == amount("0"))
    );
}

#[test]
fn a_confirmation_without_free_balance_changes_nothing() {
    let (_dir, mut vault, card, vacation) = setup();
    add_transaction_plain(&mut vault, &card, "8000");
    let reserve = envelope_on(&mut vault, &card, "Reserve");
    vault
        .doc
        .envelopes
        .iter_mut()
        .find(|envelope| envelope.id == reserve)
        .unwrap()
        .record_move(amount("28000"), "2026-11-02T00:00:00Z");
    let rows = vec![row(
        DistKind::Sum {
            amount: amount("25000"),
        },
        DistTarget::Envelope { id: vacation },
    )];
    let run_id = waiting(&mut vault, &card, rows);
    assert_refused(
        &mut vault,
        &run_id,
        "31000",
        Some(OnOtherAmount::ShrinkRemainder),
    );
    assert_eq!(holding_of(&vault, &card, &usd()), amount("8000"));
}

#[test]
fn the_confirmation_is_one_answer_with_no_write_after_it() {
    let (_dir, mut plain, card, vacation) = setup();
    let plain_run = waiting(&mut plain, &card, vec![five_percent(&vacation)]);
    let (_other, mut confirmed, card, vacation) = setup();
    let confirmed_run = waiting(&mut confirmed, &card, vec![five_percent(&vacation)]);
    let (before_plain, before_confirmed) = (plain.session.revision(), confirmed.session.revision());
    let now = unix_secs("2026-11-20T13:00:00Z").unwrap();
    let wait = plain.doc.workflow_waits[0].id.clone();
    assert_eq!(plain.doc.workflow_waits[0].run_id, plain_run);
    crate::usecases::workflow::runs::answer_workflow_wait(&mut plain, &wait, true, now, "UTC")
        .unwrap();
    confirm_at_13(
        &mut confirmed,
        &confirmed_run,
        "31000",
        Some(OnOtherAmount::ShrinkRemainder),
    )
    .unwrap();
    // Booking the answer takes the writes of an answer, not one more.
    assert_eq!(
        confirmed.session.revision() - before_confirmed,
        plain.session.revision() - before_plain
    );
}

#[test]
fn an_ask_rule_makes_the_run_wait_though_the_form_did_not_ask() {
    let (_dir, mut vault, card, vacation) = setup();
    let template = template_on_other(
        &mut vault,
        vec![five_percent(&vacation)],
        OnOtherAmount::Ask,
    );
    advance_asking(&mut vault, &card, Some(&template), false);
    let pass = run_on_20_nov(&mut vault);
    assert!(pass.created.is_empty());
    assert!(vault.doc.transactions.is_empty());
    assert_eq!(vault.doc.workflow_waits.len(), 1);
}

#[test]
fn editing_the_automation_keeps_its_template_link() {
    let (_dir, mut vault, card, vacation) = setup();
    let template = template_with(&mut vault, vec![five_percent(&vacation)]);
    let id = advance(&mut vault, &card, Some(&template));
    let stored = crate::usecases::planning::automations::automation(&vault, &id).unwrap();
    let edited = AutomationDraft {
        name: "Advance, renamed".into(),
        active: true,
        preset: stored.preset,
        schedule: stored.schedule,
        starts_at: stored.starts_at.clone(),
        template: stored.template.clone(),
        amount_mode: stored.amount_mode,
        workday_shift: stored.workday_shift,
        property_id: None,
        time: None,
        asks: false,
        for_month_offset: stored.for_month_offset,
    };
    let updated = update_automation(&mut vault, &id, edited).unwrap();
    assert_eq!(updated.name, "Advance, renamed");
    assert_eq!(
        updated.operation_template_id.as_deref(),
        Some(template.as_str())
    );
}

#[test]
fn create_now_books_a_linked_automation_by_the_template() {
    let (_dir, mut vault, card, vacation) = setup();
    let template = template_with(&mut vault, vec![five_percent(&vacation)]);
    let id = advance(&mut vault, &card, Some(&template));
    let now = unix_secs("2026-11-20T12:00:00Z").unwrap();
    let income = run_automation(&mut vault, &id, now).unwrap();
    let Transaction::Topup(topup) = income else {
        panic!("an income")
    };
    let period = topup.meta.period.as_ref().expect("a period");
    assert_eq!(
        (period.start.as_str(), period.end.as_str()),
        ("2026-11-01", "2026-11-15")
    );
    let parts: Vec<_> = topup
        .distribution
        .as_ref()
        .unwrap()
        .iter()
        .map(|part| part.amount)
        .collect();
    assert_eq!(parts, vec![amount("1500"), amount("28500")]);
    assert_eq!(vault.doc.workflow_runs.len(), 1);
    assert_eq!(holding_of(&vault, &card, &usd()), amount("30000"));
}

#[test]
fn a_retry_across_the_month_takes_the_period_of_the_planned_day() {
    let (_dir, mut vault, card, _vacation) = setup();
    add_transaction_plain(&mut vault, &card, "2000");
    let template = create_operation_template(
        &mut vault,
        TemplateDraft {
            name: "Rent".into(),
            kind: TemplateKind::Expense,
            store_id: None,
            asset: Some(usd()),
            amount: None,
            categories: Vec::new(),
            comment: String::new(),
            contact_id: None,
            period_rule: Some(PeriodRule::ThisHalfFirst),
            distribution: None,
        },
    )
    .unwrap()
    .id;
    let Transaction::Payment(inner) = payment(&card, "5000", "rent") else {
        unreachable!()
    };
    let made = create_automation(
        &mut vault,
        AutomationDraft {
            name: "Rent".into(),
            active: true,
            preset: None,
            schedule: Schedule::Monthly { day: 20 },
            starts_at: "2026-11-01T00:00:00Z".into(),
            template: TransactionDraft::Payment(inner),
            amount_mode: Default::default(),
            workday_shift: Default::default(),
            property_id: None,
            time: None,
            asks: false,
            for_month_offset: None,
        },
    )
    .unwrap();
    vault
        .doc
        .workflows
        .iter_mut()
        .find(|workflow| workflow.id == made.id)
        .unwrap()
        .automation
        .as_mut()
        .unwrap()
        .operation_template_id = Some(template);
    run_on_20_nov(&mut vault);
    let refused = vault
        .doc
        .workflow_runs
        .iter()
        .find(|run| {
            matches!(
                run.outcome,
                crate::workflow::model::RunOutcome::Rejected { .. }
            )
        })
        .unwrap_or_else(|| {
            panic!(
                "a refused run in {:?}",
                vault
                    .doc
                    .workflow_runs
                    .iter()
                    .map(|run| &run.outcome)
                    .collect::<Vec<_>>()
            )
        })
        .id
        .clone();
    // Money arrives; the retry is made in December.
    let mut topup = income(&card, "20000", "more");
    at(&mut topup, "2026-11-25T00:00:00Z");
    crate::usecases::money::transactions::add_transaction(&mut vault, topup).unwrap();
    let december = unix_secs("2026-12-02T12:00:00Z").unwrap();
    let pass =
        crate::usecases::workflow::runs::retry_workflow_run(&mut vault, &refused, december, "UTC")
            .unwrap();
    let booked = vault
        .doc
        .transactions
        .iter()
        .find(|tx| tx.meta().id == pass.created[0])
        .unwrap();
    assert!(booked.meta().timestamp.starts_with("2026-12-02"));
    let period = booked.meta().period.as_ref().expect("a period");
    assert_eq!(
        (period.start.as_str(), period.end.as_str()),
        ("2026-11-01", "2026-11-15")
    );
}

#[test]
fn a_late_yes_takes_the_period_of_the_planned_day() {
    let (_dir, mut vault, card, vacation) = setup();
    let run_id = waiting(&mut vault, &card, vec![five_percent(&vacation)]);
    let december = unix_secs("2026-12-02T12:00:00Z").unwrap();
    let income =
        confirm_distribution(&mut vault, &run_id, amount("30000"), None, december, "UTC").unwrap();
    let period = income.meta().period.as_ref().expect("a period");
    assert_eq!(
        (period.start.as_str(), period.end.as_str()),
        ("2026-11-01", "2026-11-15")
    );
}

#[test]
fn the_expected_payout_of_a_varying_automation_is_its_estimate() {
    let (_dir, mut vault, card, _vacation) = setup();
    let goal = envelope_on(&mut vault, &card, "Goal");
    let sum = DistKind::Sum {
        amount: amount("5000"),
    };
    let template = template_on_other(
        &mut vault,
        vec![row(sum, DistTarget::Envelope { id: goal })],
        OnOtherAmount::ShrinkAll,
    );
    let id = advance_with(
        &mut vault,
        &card,
        Some(&template),
        false,
        AmountMode::Varies,
    );
    // Past corrected advances of 20 000: the estimate the next one is booked at.
    let mut past = income(&card, "20000", "past");
    at(&mut past, "2026-10-20T12:00:00Z");
    past.meta_mut().origin = mwt_model::TxOrigin::Workflow {
        workflow_id: id.clone(),
        run_id: "old".into(),
        planned_at: None,
        planned_date: None,
        estimated: false,
        run_key: None,
    };
    vault.doc.transactions.push(past);
    crate::usecases::money::portfolio::recompute_holdings(&mut vault);
    let preview = preview_distribution_rule(&vault, &id, None, day_before()).unwrap();
    assert_eq!(preview.amount, amount("20000"));
    let rows: Vec<_> = preview.resolved.rows.iter().map(|row| row.amount).collect();
    assert_eq!(rows, vec![amount("5000"), amount("15000")]);
    let pass = run_on_20_nov(&mut vault);
    let Some(Transaction::Topup(topup)) = vault
        .doc
        .transactions
        .iter()
        .find(|tx| tx.meta().id == pass.created[0])
    else {
        panic!("an income")
    };
    assert_eq!(topup.amount, amount("20000"));
    let booked: Vec<_> = topup
        .distribution
        .as_ref()
        .unwrap()
        .iter()
        .map(|part| part.amount)
        .collect();
    assert_eq!(booked, vec![amount("5000"), amount("15000")]);
}

#[test]
fn a_store_row_is_emulated_and_booked_by_a_run() {
    let (_dir, mut vault, card, _vacation) = setup();
    let savings = store(&mut vault, "Savings", StoreKind::Bank, None);
    let template = template_with(
        &mut vault,
        vec![row(
            DistKind::Percent { pct: amount("10") },
            DistTarget::Store {
                id: savings.clone(),
            },
        )],
    );
    let id = advance(&mut vault, &card, Some(&template));
    let before = unix_secs("2026-11-19T12:00:00Z").unwrap();
    let emulated = simulate_automations(&vault, Some(&id), 3, before, "UTC").unwrap();
    assert_eq!(
        (emulated.created, emulated.rejected),
        (1, 0),
        "{emulated:?}"
    );
    assert!(vault.doc.transactions.is_empty());
    let pass = run_on_20_nov(&mut vault);
    assert_eq!(pass.created.len(), 1);
    let transfers: Vec<_> = vault
        .doc
        .transactions
        .iter()
        .filter(|tx| matches!(tx, Transaction::Transfer(_)))
        .collect();
    assert_eq!(transfers.len(), 1);
    assert_eq!(
        transfers[0].meta().income_ref.as_deref(),
        Some(pass.created[0].as_str())
    );
    assert_eq!(holding_of(&vault, &savings, &usd()), amount("3000"));
    assert_eq!(holding_of(&vault, &card, &usd()), amount("27000"));
}

#[test]
fn a_run_links_only_its_income_to_the_property_not_the_transfers() {
    let (_dir, mut vault, card, _vacation) = setup();
    let savings = store(&mut vault, "Savings", StoreKind::Bank, None);
    let flat = crate::usecases::money::stores::create_store_with(
        &mut vault,
        "Flat",
        StoreKind::Property,
        "#555",
        "",
        None,
        crate::service::StoreExtras {
            property: Some(mwt_model::PropertyInfo::default()),
            ..crate::service::StoreExtras::default()
        },
    )
    .unwrap()
    .id;
    let template = template_with(
        &mut vault,
        vec![row(
            DistKind::Percent { pct: amount("10") },
            DistTarget::Store { id: savings },
        )],
    );
    let id = advance(&mut vault, &card, Some(&template));
    vault
        .doc
        .workflows
        .iter_mut()
        .find(|workflow| workflow.id == id)
        .unwrap()
        .automation
        .as_mut()
        .unwrap()
        .property_id = Some(flat.clone());
    let pass = run_on_20_nov(&mut vault);
    assert_eq!(pass.created.len(), 1);
    let linked = |tx_id: &str| crate::household::property_links::property_of_tx(&vault.doc, tx_id);
    assert_eq!(linked(&pass.created[0]), Some(flat.as_str()));
    let transfers: Vec<_> = vault
        .doc
        .transactions
        .iter()
        .filter(|tx| matches!(tx, Transaction::Transfer(_)))
        .collect();
    assert_eq!(transfers.len(), 1);
    assert_eq!(linked(&transfers[0].meta().id), None);
}

#[test]
fn create_now_keeps_the_error_of_a_refused_preparation() {
    let (_dir, mut vault, card, _vacation) = setup();
    let id = advance(&mut vault, &card, None);
    vault.doc.stores.retain(|known| known.id != card);
    let now = unix_secs("2026-11-20T12:00:00Z").unwrap();
    let refusal = run_automation(&mut vault, &id, now).unwrap_err();
    // The kind (hence the stable code) of the validation error is the one `add_transaction` gives.
    let plain = crate::usecases::money::transactions::add_transaction(
        &mut vault,
        income(&card, "30000", "advance"),
    )
    .unwrap_err();
    assert_eq!(
        std::mem::discriminant(&refusal),
        std::mem::discriminant(&plain),
        "{refusal} / {plain}"
    );
    assert_eq!(refusal.to_string(), plain.to_string());
}

#[test]
fn a_distribution_transfer_never_counts_toward_the_estimate_of_the_next_run() {
    let (_dir, mut vault, card, _vacation) = setup();
    let savings = store(&mut vault, "Savings", StoreKind::Bank, None);
    let template = template_on_other(
        &mut vault,
        vec![row(
            DistKind::Sum {
                amount: amount("20000"),
            },
            DistTarget::Store { id: savings },
        )],
        OnOtherAmount::ShrinkAll,
    );
    let id = advance_with(
        &mut vault,
        &card,
        Some(&template),
        false,
        AmountMode::Varies,
    );
    let mut past = income(&card, "120000", "past");
    at(&mut past, "2026-10-20T12:00:00Z");
    past.meta_mut().origin = mwt_model::TxOrigin::Workflow {
        workflow_id: id.clone(),
        run_id: "old".into(),
        planned_at: None,
        planned_date: None,
        estimated: false,
        run_key: None,
    };
    vault.doc.transactions.push(past);
    crate::usecases::money::portfolio::recompute_holdings(&mut vault);
    let first = run_on_20_nov(&mut vault);
    assert_eq!(first.created.len(), 1);
    let transfer = vault
        .doc
        .transactions
        .iter()
        .find(|tx| matches!(tx, Transaction::Transfer(_)))
        .expect("the store row made a transfer");
    assert!(transfer.meta().income_ref.is_some());
    let december = unix_secs("2026-12-19T12:00:00Z").unwrap();
    let preview = |vault: &Session| {
        preview_distribution_rule(vault, &id, None, december)
            .unwrap()
            .amount
    };
    // Still approximate: the estimate is the past corrected 120 000, not the 20 000 transfer.
    assert_eq!(preview(&vault), amount("120000"));
    crate::usecases::planning::automations::confirm_estimated_amount(
        &mut vault,
        &first.created[0],
        amount("120000"),
    )
    .unwrap();
    assert_eq!(preview(&vault), amount("120000"));
    let second = run_on(&mut vault, "2026-12-20T12:00:00Z");
    let Some(Transaction::Topup(topup)) = vault
        .doc
        .transactions
        .iter()
        .find(|tx| second.created.first() == Some(&tx.meta().id))
    else {
        panic!("the second income")
    };
    assert_eq!(topup.amount, amount("120000"));
}

fn run_on(vault: &mut Session, moment: &str) -> crate::workflow::service::runs::WorkflowPass {
    run_due_workflows(vault, unix_secs(moment).unwrap(), "UTC").unwrap()
}

/// What a path left of an automation whose rule asks, to compare two ways of setting the question.
#[derive(Debug, PartialEq)]
struct Left {
    incomes: usize,
    undistributed: usize,
    waits: usize,
    created: usize,
}

fn left(vault: &Session, created: usize) -> Left {
    Left {
        incomes: vault.doc.transactions.len(),
        undistributed: vault
            .doc
            .transactions
            .iter()
            .filter(|tx| matches!(tx, Transaction::Topup(topup) if topup.distribution.is_none()))
            .count(),
        waits: vault.doc.workflow_waits.len(),
        created,
    }
}

/// An advance whose rule asks, with the form's `asks` as given, and one path run on it.
fn through(asks: bool, path: fn(&mut Session, &str) -> usize) -> Left {
    let (_dir, mut vault, card, vacation) = setup();
    let template = template_on_other(
        &mut vault,
        vec![five_percent(&vacation)],
        OnOtherAmount::Ask,
    );
    let id = advance_asking(&mut vault, &card, Some(&template), asks);
    let created = path(&mut vault, &id);
    left(&vault, created)
}

fn run_now(vault: &mut Session, id: &str) -> usize {
    let now = unix_secs("2026-11-20T12:00:00Z").unwrap();
    crate::usecases::workflow::runs::run_workflow_now(vault, id, None, now, "UTC")
        .unwrap()
        .run
        .created
        .len()
}

fn create_now(vault: &mut Session, id: &str) -> usize {
    let now = unix_secs("2026-11-20T12:00:00Z").unwrap();
    run_automation(vault, id, now).unwrap();
    1
}

fn retry(vault: &mut Session, id: &str) -> usize {
    let refused = crate::workflow::model::WorkflowRun {
        id: "refused".into(),
        workflow_id: id.into(),
        trigger: "schedule 2026-11-20".into(),
        started_at: "2026-11-20T00:00:00Z".into(),
        outcome: crate::workflow::model::RunOutcome::Rejected {
            reason: RejectReason::Invalid {
                message: "no free balance".into(),
            },
            trigger_node: crate::workflow::automation::TRIGGER.into(),
            planned_at: Some("2026-11-20T00:00:00Z".into()),
        },
        created: Vec::new(),
        queued: 0,
        trace: Vec::new(),
        dry_run: false,
        planned_date: None,
    };
    vault.doc.workflow_runs.push(refused);
    let now = unix_secs("2026-11-20T13:00:00Z").unwrap();
    crate::usecases::workflow::runs::retry_workflow_run(vault, "refused", now, "UTC")
        .unwrap()
        .created
        .len()
}

#[test]
fn run_now_follows_an_ask_rule_like_an_automation_that_asks() {
    let reference = through(true, run_now);
    assert_eq!(reference.waits, 1);
    assert_eq!(reference.incomes, 0);
    assert_eq!(through(false, run_now), reference);
}

#[test]
fn a_retry_follows_an_ask_rule_like_an_automation_that_asks() {
    let reference = through(true, retry);
    assert_eq!(reference.waits, 1);
    assert_eq!(reference.incomes, 0);
    assert_eq!(through(false, retry), reference);
}

#[test]
fn create_now_follows_an_ask_rule_like_an_automation_that_asks() {
    let reference = through(true, create_now);
    assert_eq!(reference.incomes, 1);
    assert_eq!(through(false, create_now), reference);
}
