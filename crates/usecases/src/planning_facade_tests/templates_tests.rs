//! op-period T20: operation templates, their validation (T2, D7), the CRUD usecases (T3) and the
//! repair of dangling references.

use crate::error::Error;
use crate::model::planning::{Schedule, TransactionDraft};
use crate::model::store::StoreKind;
use crate::model::transaction::{TopupTx, Transaction};
use crate::planning::automations::AutomationDraft;
use crate::planning::templates::TemplateDraft;
use crate::service::Session;
use crate::service::test_support::*;
use crate::usecases::planning::automations::create_automation;
use crate::usecases::planning::templates::{
    archive_operation_template, create_operation_template, delete_operation_template,
    list_operation_templates, restore_operation_template, update_operation_template,
};
use crate::usecases::sync::repair::repair_invariants;
use mwt_model::{
    Contact, ContactKind, DistKind, DistTarget, DistributionRow, DistributionRule, OnOtherAmount,
    PeriodRule, TemplateKind,
};

fn draft(name: &str, kind: TemplateKind) -> TemplateDraft {
    TemplateDraft {
        name: name.into(),
        kind,
        store_id: None,
        asset: None,
        amount: None,
        categories: Vec::new(),
        comment: String::new(),
        contact_id: None,
        period_rule: Some(PeriodRule::PrevHalfSecond),
        distribution: None,
    }
}

fn row(kind: DistKind, target: DistTarget) -> DistributionRow {
    DistributionRow { kind, target }
}

fn rule(rows: Vec<DistributionRow>) -> DistributionRule {
    DistributionRule {
        rows,
        on_other_amount: OnOtherAmount::ShrinkRemainder,
        keep_on_card_min: None,
    }
}

fn remainder() -> DistributionRow {
    row(DistKind::Remainder, DistTarget::None)
}

fn vault_with_card() -> (tempfile::TempDir, Session, String) {
    let (dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    (dir, vault, card)
}

fn message(error: Error) -> String {
    error.to_string()
}

fn refused(vault: &mut Session, input: TemplateDraft) -> String {
    message(create_operation_template(vault, input).unwrap_err())
}

fn add_contact(vault: &mut Session, id: &str) {
    vault.doc.contacts.push(Contact {
        id: id.into(),
        name: "Employer".into(),
        kind: ContactKind::Company,
        accounts: Vec::new(),
        note: String::new(),
        archived: false,
        created_at: "2026-11-25T00:00:00Z".into(),
        group: String::new(),
    });
}

/// Book an automation that refers to the template (the draft has no field for it yet).
fn automation_using(vault: &mut Session, card: &str, template_id: &str) {
    let Transaction::Topup(inner) = income(card, "100", "salary") else {
        unreachable!()
    };
    let made = create_automation(
        vault,
        AutomationDraft {
            name: "Payday".into(),
            active: true,
            preset: None,
            schedule: Schedule::Monthly { day: 5 },
            starts_at: "2026-01-01T00:00:00Z".into(),
            template: TransactionDraft::Topup(TopupTx { ..inner }),
            amount_mode: Default::default(),
            workday_shift: Default::default(),
            property_id: None,
            time: None,
            asks: false,
            for_month_offset: None,
        },
    )
    .unwrap();
    let workflow = vault
        .doc
        .workflows
        .iter_mut()
        .find(|workflow| workflow.id == made.id)
        .unwrap();
    workflow.automation.as_mut().unwrap().operation_template_id = Some(template_id.into());
}

#[test]
fn t2_distribution_only_on_an_income() {
    let (_dir, mut vault, _card) = vault_with_card();
    let mut input = draft("Rent", TemplateKind::Expense);
    input.distribution = Some(rule(vec![remainder()]));
    assert!(refused(&mut vault, input).contains("only an income"));
}

#[test]
fn d7_exactly_one_remainder_row() {
    let (_dir, mut vault, card) = vault_with_card();
    let own = row(
        DistKind::Percent { pct: amount("5") },
        DistTarget::Store { id: card },
    );
    for rows in [vec![own.clone()], vec![own, remainder(), remainder()]] {
        let mut input = draft("Salary", TemplateKind::Income);
        input.distribution = Some(rule(rows));
        assert!(refused(&mut vault, input).contains("exactly one remainder"));
    }
}

#[test]
fn t2_rule_targets_exist_and_are_own() {
    let (_dir, mut vault, _card) = vault_with_card();
    let mut input = draft("Salary", TemplateKind::Income);
    input.distribution = Some(rule(vec![
        row(
            DistKind::Sum {
                amount: amount("10"),
            },
            DistTarget::Store { id: "ghost".into() },
        ),
        remainder(),
    ]));
    assert!(refused(&mut vault, input).contains("ghost"));
    let mut input = draft("Salary", TemplateKind::Income);
    input.distribution = Some(rule(vec![
        row(
            DistKind::Sum {
                amount: amount("10"),
            },
            DistTarget::Envelope { id: "ghost".into() },
        ),
        remainder(),
    ]));
    assert!(refused(&mut vault, input).contains("ghost"));
    let mut input = draft("Salary", TemplateKind::Income);
    input.distribution = Some(rule(vec![row(
        DistKind::Remainder,
        DistTarget::Store { id: "x".into() },
    )]));
    assert!(refused(&mut vault, input).contains("remainder"));
}

#[test]
fn t2_name_is_required_and_unique_among_live_templates() {
    let (_dir, mut vault, _card) = vault_with_card();
    assert!(refused(&mut vault, draft("  ", TemplateKind::Income)).contains("name"));
    let first =
        create_operation_template(&mut vault, draft("Salary", TemplateKind::Income)).unwrap();
    assert!(
        refused(&mut vault, draft(" salary ", TemplateKind::Expense)).contains("already exists")
    );
    // An archived template frees its name; bringing it back then collides.
    archive_operation_template(&mut vault, &first.id, true).unwrap();
    let second =
        create_operation_template(&mut vault, draft("Salary", TemplateKind::Income)).unwrap();
    let back = archive_operation_template(&mut vault, &first.id, false).unwrap_err();
    assert!(message(back).contains("already exists"));
    // A template is not a duplicate of itself.
    update_operation_template(
        &mut vault,
        &second.id,
        draft("Salary", TemplateKind::Income),
    )
    .unwrap();
}

#[test]
fn t2_references_must_exist() {
    let (_dir, mut vault, _card) = vault_with_card();
    let mut input = draft("A", TemplateKind::Expense);
    input.categories = vec!["no_such_category".into()];
    assert!(refused(&mut vault, input).contains("no_such_category"));
    let mut input = draft("B", TemplateKind::Expense);
    input.store_id = Some("no_store".into());
    assert!(refused(&mut vault, input).contains("no_store"));
    let mut input = draft("C", TemplateKind::Expense);
    input.contact_id = Some("no_contact".into());
    assert!(refused(&mut vault, input).contains("no_contact"));
}

#[test]
fn t3_crud_round_trip() {
    let (_dir, mut vault, card) = vault_with_card();
    add_contact(&mut vault, "boss");
    let mut input = draft("Salary", TemplateKind::Income);
    input.store_id = Some(card.clone());
    input.contact_id = Some("boss".into());
    input.categories = vec!["salary".into()];
    input.distribution = Some(rule(vec![
        row(
            DistKind::Percent { pct: amount("5") },
            DistTarget::Store { id: card.clone() },
        ),
        remainder(),
    ]));
    let created = create_operation_template(&mut vault, input.clone()).unwrap();
    assert_eq!(list_operation_templates(&vault), vec![created.clone()]);

    input.name = "Advance".into();
    let updated = update_operation_template(&mut vault, &created.id, input).unwrap();
    assert_eq!(updated.id, created.id);
    assert_eq!(updated.created_at, created.created_at);
    assert_eq!(list_operation_templates(&vault)[0].name, "Advance");

    let archived = archive_operation_template(&mut vault, &created.id, true).unwrap();
    assert!(archived.archived);

    let deleted = delete_operation_template(&mut vault, &created.id).unwrap();
    assert!(list_operation_templates(&vault).is_empty());
    let back = restore_operation_template(&mut vault, deleted.clone()).unwrap();
    assert_eq!(back, deleted);
    assert_eq!(list_operation_templates(&vault), vec![deleted.clone()]);
    assert!(restore_operation_template(&mut vault, deleted).is_err());
}

#[test]
fn t3_delete_is_refused_while_an_automation_uses_the_template_archive_works() {
    let (_dir, mut vault, card) = vault_with_card();
    let created =
        create_operation_template(&mut vault, draft("Salary", TemplateKind::Income)).unwrap();
    automation_using(&mut vault, &card, &created.id);
    let error = delete_operation_template(&mut vault, &created.id).unwrap_err();
    assert!(matches!(error, Error::InvalidInput(_)), "{error}");
    assert!(message(error).contains("Payday"));
    assert_eq!(list_operation_templates(&vault).len(), 1);
    assert!(
        archive_operation_template(&mut vault, &created.id, true)
            .unwrap()
            .archived
    );
}

#[test]
fn t2_repair_drops_dangling_references() {
    let (_dir, mut vault, card) = vault_with_card();
    add_contact(&mut vault, "boss");
    let mut input = draft("Salary", TemplateKind::Income);
    input.store_id = Some(card.clone());
    input.contact_id = Some("boss".into());
    input.categories = vec!["salary".into()];
    let created = create_operation_template(&mut vault, input).unwrap();

    assert_eq!(
        repair_invariants(&mut vault, None)
            .unwrap()
            .dropped_template_refs,
        0
    );

    vault.doc.stores.retain(|node| node.id != card);
    vault.doc.contacts.clear();
    vault
        .doc
        .categories
        .retain(|category| category.key != "salary");
    assert_eq!(
        repair_invariants(&mut vault, None)
            .unwrap()
            .dropped_template_refs,
        3
    );
    let kept = &vault.doc.operation_templates[0];
    assert_eq!(kept.id, created.id);
    assert_eq!(
        (kept.store_id.clone(), kept.contact_id.clone()),
        (None, None)
    );
    assert!(kept.categories.is_empty());
}
