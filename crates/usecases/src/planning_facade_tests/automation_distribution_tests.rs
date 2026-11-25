//! op-period T22: an automation run that refers to an operation template takes the template's
//! period rule and distribution rule, all in the one write of the run (T5, D5, D12, Q11, A4).

use crate::model::planning::{AutomationOutcome, RejectReason, Schedule, TransactionDraft};
use crate::model::store::StoreKind;
use crate::model::transaction::Transaction;
use crate::planning::automations::AutomationDraft;
use crate::planning::templates::TemplateDraft;
use crate::service::Session;
use crate::service::test_support::*;
use crate::time::unix_secs;
use crate::usecases::planning::automations::{
    confirm_distribution, create_automation, preview_distribution_rule,
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
            amount_mode: Default::default(),
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
    assert!(vault.session.revision() > revision);
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
    assert!(
        vault
            .doc
            .notification_log
            .iter()
            .any(|record| record.key.starts_with("automation-rejected:")),
        "{:?}",
        vault.doc.notification_log
    );
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
