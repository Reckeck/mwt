//! op-period T22: the effects of an automation run's distribution carry the run's identity, so
//! the run made on two devices is one booking after the sync, effects included.

use super::tests::{settle, two_devices};
use super::*;
use crate::model::planning::{AmountMode, Schedule, TransactionDraft};
use crate::model::store::StoreKind;
use crate::model::transaction::Transaction;
use crate::planning::automations::AutomationDraft;
use crate::planning::templates::TemplateDraft;
use crate::service::test_support::{amount, income, store, usd};
use crate::time::unix_secs;
use crate::usecases::planning::automations::create_automation;
use crate::usecases::planning::distribution::replace_distribution;
use crate::usecases::planning::templates::create_operation_template;
use crate::usecases::workflow::runs::run_due_workflows;
use mwt_model::{
    DistKind, DistTarget, DistributionRow, DistributionRule, OnOtherAmount, TemplateKind,
};

fn row(kind: DistKind, target: DistTarget) -> DistributionRow {
    DistributionRow { kind, target }
}

/// The advance of 30 000 on the 20th: 10% to `savings`, 5% to an envelope, the rest stays.
fn advance_with_rows() -> (tempfile::TempDir, Session, Session, String) {
    let (dir, mut left, mut right) = two_devices();
    let card = store(&mut left, "Card", StoreKind::Bank, None);
    let savings = store(&mut left, "Savings", StoreKind::Bank, None);
    let envelope = crate::usecases::household::envelopes::create_envelope(
        &mut left,
        crate::planning::envelopes::EnvelopeDraft {
            store_id: card.clone(),
            name: "Vacation".into(),
            asset: usd(),
            target: None,
            goal_id: None,
        },
    )
    .unwrap()
    .id;
    let template = create_operation_template(
        &mut left,
        TemplateDraft {
            name: "Advance".into(),
            kind: TemplateKind::Income,
            store_id: None,
            asset: Some(usd()),
            amount: Some(amount("30000")),
            categories: Vec::new(),
            comment: String::new(),
            contact_id: None,
            period_rule: None,
            distribution: Some(DistributionRule {
                rows: vec![
                    row(
                        DistKind::Percent { pct: amount("10") },
                        DistTarget::Store { id: savings },
                    ),
                    row(
                        DistKind::Percent { pct: amount("5") },
                        DistTarget::Envelope { id: envelope },
                    ),
                    row(DistKind::Remainder, DistTarget::None),
                ],
                on_other_amount: OnOtherAmount::ShrinkRemainder,
                keep_on_card_min: None,
            }),
        },
    )
    .unwrap()
    .id;
    let Transaction::Topup(inner) = income(&card, "30000", "advance") else {
        unreachable!()
    };
    let made = create_automation(
        &mut left,
        AutomationDraft {
            name: "Advance".into(),
            active: true,
            preset: None,
            schedule: Schedule::Monthly { day: 20 },
            starts_at: "2026-11-01T00:00:00Z".into(),
            template: TransactionDraft::Topup(inner),
            amount_mode: AmountMode::Fixed,
            workday_shift: Default::default(),
            property_id: None,
            time: None,
            asks: false,
            for_month_offset: None,
        },
    )
    .unwrap();
    left.doc
        .workflows
        .iter_mut()
        .find(|workflow| workflow.id == made.id)
        .unwrap()
        .automation
        .as_mut()
        .unwrap()
        .operation_template_id = Some(template);
    left.persist().unwrap();
    settle(&mut left, &mut right);
    (dir, left, right, made.id)
}

fn count(vault: &Session, wanted: fn(&Transaction) -> bool) -> usize {
    vault
        .doc
        .transactions
        .iter()
        .filter(|tx| wanted(tx))
        .count()
}

fn envelope_moves(vault: &Session) -> usize {
    vault
        .doc
        .envelopes
        .iter()
        .map(|envelope| envelope.moves.len())
        .sum()
}

#[test]
fn a_distributing_run_on_both_devices_books_its_effects_once() {
    let (_dir, mut left, mut right, _id) = advance_with_rows();
    let moment = unix_secs("2026-11-20T12:00:00Z").unwrap();
    run_due_workflows(&mut left, moment, "UTC").unwrap();
    run_due_workflows(&mut right, moment, "UTC").unwrap();
    assert_eq!(count(&left, |tx| matches!(tx, Transaction::Transfer(_))), 1);
    settle(&mut left, &mut right);
    for vault in [&left, &right] {
        assert_eq!(count(vault, |tx| matches!(tx, Transaction::Topup(_))), 1);
        assert_eq!(
            count(vault, |tx| matches!(tx, Transaction::Transfer(_))),
            1,
            "the transfer was booked twice"
        );
        assert_eq!(envelope_moves(vault), 1, "the envelope was filled twice");
    }
    let sorted = |vault: &Session| {
        let mut ids: Vec<String> = vault
            .doc
            .transactions
            .iter()
            .map(|tx| tx.meta().id.clone())
            .collect();
        ids.sort();
        ids
    };
    assert_eq!(sorted(&left), sorted(&right));
    assert_eq!(left.doc.envelopes, right.doc.envelopes);
    for vault in [&mut left, &mut right] {
        let (income_id, rows) = vault
            .doc
            .transactions
            .iter()
            .find_map(|tx| match tx {
                Transaction::Topup(topup) => Some((
                    topup.meta.id.clone(),
                    topup
                        .distribution
                        .iter()
                        .flatten()
                        .map(|part| row(part.kind.clone(), part.target.clone()))
                        .collect::<Vec<_>>(),
                )),
                _ => None,
            })
            .unwrap();
        let found = crate::money::distribution::verify_distribution(&vault.books(), &income_id);
        assert!(found.is_empty(), "{found:?}");
        replace_distribution(vault, &income_id, &rows).unwrap();
    }
}
