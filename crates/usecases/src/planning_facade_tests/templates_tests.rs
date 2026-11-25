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
    PeriodRule, ReceivedPart, TemplateKind,
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

/// V1: a sum or percent row that is not positive is refused on create and on update.
#[test]
fn v1_rows_must_be_positive() {
    let (_dir, mut vault, card) = vault_with_card();
    let bad_kinds = [
        DistKind::Sum {
            amount: amount("0"),
        },
        DistKind::Sum {
            amount: amount("-5000"),
        },
        DistKind::Percent { pct: amount("0") },
    ];
    let mut ok = draft("Salary", TemplateKind::Income);
    ok.distribution = Some(rule(vec![remainder()]));
    let saved = create_operation_template(&mut vault, ok.clone()).unwrap();
    let before = vault.doc.operation_templates.clone();
    for kind in bad_kinds {
        let mut input = ok.clone();
        input.distribution = Some(rule(vec![
            row(kind, DistTarget::Store { id: card.clone() }),
            remainder(),
        ]));
        let created = message(
            create_operation_template(
                &mut vault,
                TemplateDraft {
                    name: "Other".into(),
                    ..input.clone()
                },
            )
            .unwrap_err(),
        );
        assert!(
            created.contains("DISTRIBUTION_PART_NOT_POSITIVE"),
            "{created}"
        );
        let updated = message(update_operation_template(&mut vault, &saved.id, input).unwrap_err());
        assert!(
            updated.contains("DISTRIBUTION_PART_NOT_POSITIVE"),
            "{updated}"
        );
        assert_eq!(vault.doc.operation_templates, before);
    }
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

mod quick_add {
    use super::*;
    use crate::usecases::money::transactions::{
        CreationRules, add_transaction, add_transaction_ruled,
    };
    use crate::usecases::planning::templates::{QuickAdd, create_from_template};
    use mwt_model::{AssetRef, Transaction};

    fn rub() -> AssetRef {
        AssetRef::fiat("RUB", "Russian Ruble")
    }

    fn envelope_on(vault: &mut Session, card: &str, name: &str) -> String {
        let draft = crate::planning::envelopes::EnvelopeDraft {
            store_id: card.into(),
            name: name.into(),
            asset: rub(),
            target: None,
            goal_id: None,
        };
        crate::usecases::household::envelopes::create_envelope(vault, draft)
            .unwrap()
            .id
    }

    /// The A2 rule: 20000 to the deposit, 5% and 3000 to two envelopes, the rest stays.
    fn salary_template(vault: &mut Session, card: &str, deposit: &str) -> String {
        let vacation = envelope_on(vault, card, "Vacation");
        let gifts = envelope_on(vault, card, "Gifts");
        let mut input = draft("Zarplata", TemplateKind::Income);
        input.store_id = Some(card.into());
        input.asset = Some(rub());
        input.amount = Some(amount("120000"));
        input.categories = vec!["salary".into()];
        input.distribution = Some(rule(vec![
            row(
                DistKind::Sum {
                    amount: amount("20000"),
                },
                DistTarget::Store { id: deposit.into() },
            ),
            row(
                DistKind::Percent { pct: amount("5") },
                DistTarget::Envelope { id: vacation },
            ),
            row(
                DistKind::Sum {
                    amount: amount("3000"),
                },
                DistTarget::Envelope { id: gifts },
            ),
            remainder(),
        ]));
        create_operation_template(vault, input).unwrap().id
    }

    fn setup_two() -> (tempfile::TempDir, Session, String, String) {
        let (dir, mut vault, card) = vault_with_card();
        let deposit = store(&mut vault, "Cushion", StoreKind::Bank, None);
        (dir, vault, card, deposit)
    }

    fn on_5_nov() -> QuickAdd {
        QuickAdd {
            timestamp: Some("2026-11-05T10:00:00Z".into()),
            ..QuickAdd::default()
        }
    }

    #[test]
    fn a4b_quick_add_sets_the_period_and_distributes_in_one_write() {
        let (_dir, mut vault, card, deposit) = setup_two();
        let id = salary_template(&mut vault, &card, &deposit);
        let added = create_from_template(&mut vault, &id, on_5_nov()).unwrap();
        assert!(!added.ask);
        let Transaction::Topup(topup) = &added.transaction else {
            panic!("an income")
        };
        let period = topup.meta.period.as_ref().expect("a period");
        assert_eq!(period.start, "2026-10-16");
        assert_eq!(period.end, "2026-10-31");
        let parts = topup.distribution.as_ref().unwrap();
        let amounts: Vec<_> = parts.iter().map(|part| part.amount).collect();
        assert_eq!(
            amounts,
            ["20000", "6000", "3000", "91000"].map(amount).to_vec()
        );
        let transfers = vault
            .doc
            .transactions
            .iter()
            .filter(|tx| matches!(tx, Transaction::Transfer(_)))
            .count();
        assert_eq!(transfers, 1);
        assert_eq!(holding_of(&vault, &deposit, &rub()), amount("20000"));
    }

    #[test]
    fn an_over_allocating_rule_writes_nothing() {
        let (_dir, mut vault, card, deposit) = setup_two();
        let id = salary_template(&mut vault, &card, &deposit);
        let before = vault.doc.transactions.len();
        let input = QuickAdd {
            amount: Some(amount("10000")),
            ..on_5_nov()
        };
        // ShrinkRemainder keeps the 20000 sum: it exceeds the 10000 payout.
        assert!(create_from_template(&mut vault, &id, input).is_err());
        assert_eq!(vault.doc.transactions.len(), before);
    }

    #[test]
    fn an_edited_input_keeps_its_own_period() {
        let (_dir, mut vault, card, deposit) = setup_two();
        let _ = salary_template(&mut vault, &card, &deposit);
        let mut tx = crate::service::test_support::income_of(&card, rub(), "5", "0.01", "x");
        at(&mut tx, "2026-11-05T10:00:00Z");
        let period = mwt_model::OpPeriod {
            kind: mwt_model::PeriodKind::Month,
            start: "2026-09-01".into(),
            end: "2026-09-30".into(),
        };
        tx.meta_mut().period = Some(period.clone());
        let saved = add_transaction(&mut vault, tx).unwrap();
        assert_eq!(saved.meta().period, Some(period));
    }

    /// A one-row rule «20000 to the deposit, the rest stays» with a floor on the card.
    fn floor_template(vault: &mut Session, card: &str, deposit: &str, floor: &str) -> String {
        let mut input = draft("Floor", TemplateKind::Income);
        input.categories = vec!["salary".into()];
        input.store_id = Some(card.into());
        input.asset = Some(rub());
        input.amount = Some(amount("120000"));
        input.distribution = Some(DistributionRule {
            rows: vec![
                row(
                    DistKind::Sum {
                        amount: amount("20000"),
                    },
                    DistTarget::Store { id: deposit.into() },
                ),
                remainder(),
            ],
            on_other_amount: OnOtherAmount::ShrinkRemainder,
            keep_on_card_min: Some(amount(floor)),
        });
        create_operation_template(vault, input).unwrap().id
    }

    fn transfers(vault: &Session) -> usize {
        vault
            .doc
            .transactions
            .iter()
            .filter(|tx| matches!(tx, Transaction::Transfer(_)))
            .count()
    }

    /// D10: the floor is judged on the balance after the rows leave the card, so the store row is
    /// cut by what the card lacks (120000 - 20000 = 100000 against a floor of 105000).
    #[test]
    fn keep_on_card_min_cuts_the_store_row() {
        let (_dir, mut vault, card, deposit) = setup_two();
        let id = floor_template(&mut vault, &card, &deposit, "105000");
        create_from_template(&mut vault, &id, on_5_nov()).unwrap();
        assert_eq!(holding_of(&vault, &deposit, &rub()), amount("15000"));
        assert_eq!(holding_of(&vault, &card, &rub()), amount("105000"));
    }

    /// D10 with a balance already on the card: 50000 held + 120000 - 20000 = 150000 clears 140000.
    #[test]
    fn keep_on_card_min_counts_what_the_card_already_holds() {
        let (_dir, mut vault, card, deposit) = setup_two();
        let id = floor_template(&mut vault, &card, &deposit, "140000");
        let mut held = crate::service::test_support::income_of(&card, rub(), "50000", "0.01", "x");
        at(&mut held, "2026-11-01T10:00:00Z");
        add_transaction(&mut vault, held).unwrap();
        create_from_template(&mut vault, &id, on_5_nov()).unwrap();
        assert_eq!(holding_of(&vault, &deposit, &rub()), amount("20000"));
    }

    /// D10, Q13: a row cut to nothing is left out; the money stays on the card.
    #[test]
    fn a_row_cut_to_zero_is_dropped_and_the_rest_applies() {
        let (_dir, mut vault, card, deposit) = setup_two();
        let id = salary_template(&mut vault, &card, &deposit);
        let mut template = vault.doc.operation_templates[0].clone();
        template.distribution.as_mut().unwrap().keep_on_card_min = Some(amount("100000"));
        vault.doc.operation_templates[0] = template;
        // Rows leave 29000, the card keeps 91000: the floor of 100000 cuts 9000 from the bottom
        // up, the gifts (3000) vanish and the vacation row (6000) is cut to nothing as well.
        let added = create_from_template(&mut vault, &id, on_5_nov()).unwrap();
        let Transaction::Topup(topup) = &added.transaction else {
            panic!("an income")
        };
        let amounts: Vec<_> = topup
            .distribution
            .as_ref()
            .unwrap()
            .iter()
            .map(|part| part.amount)
            .collect();
        assert_eq!(amounts, ["20000", "100000"].map(amount).to_vec());
        assert_eq!(holding_of(&vault, &deposit, &rub()), amount("20000"));
    }

    /// An unreachable floor leaves only the remainder: the income is saved undistributed.
    #[test]
    fn an_unreachable_floor_saves_the_income_undistributed() {
        let (_dir, mut vault, card, deposit) = setup_two();
        let id = floor_template(&mut vault, &card, &deposit, "1000000");
        let added = create_from_template(&mut vault, &id, on_5_nov()).unwrap();
        assert!(!added.ask);
        let Transaction::Topup(topup) = &added.transaction else {
            panic!("an income")
        };
        assert!(topup.distribution.is_none());
        assert_eq!(transfers(&vault), 0);
        assert_eq!(holding_of(&vault, &card, &rub()), amount("120000"));
    }

    /// D9: another amount under `ask` saves the income with its period and distributes nothing.
    #[test]
    fn ask_saves_the_income_and_reports_the_flag() {
        let (_dir, mut vault, card, deposit) = setup_two();
        let id = salary_template(&mut vault, &card, &deposit);
        vault.doc.operation_templates[0]
            .distribution
            .as_mut()
            .unwrap()
            .on_other_amount = OnOtherAmount::Ask;
        let input = QuickAdd {
            amount: Some(amount("118500")),
            ..on_5_nov()
        };
        let added = create_from_template(&mut vault, &id, input).unwrap();
        assert!(added.ask);
        let Transaction::Topup(topup) = &added.transaction else {
            panic!("an income")
        };
        assert!(topup.distribution.is_none());
        assert_eq!(topup.meta.period.as_ref().unwrap().start, "2026-10-16");
        assert_eq!(transfers(&vault), 0);
        assert_eq!(holding_of(&vault, &card, &rub()), amount("118500"));
    }

    /// S15: a failure in the middle of the write puts the whole vault back.
    #[test]
    fn a_failed_distribution_leaves_the_vault_as_it_was() {
        let (_dir, mut vault, card, deposit) = setup_two();
        let id = salary_template(&mut vault, &card, &deposit);
        // The deposit goes away after the template was made: the first row cannot be applied.
        vault.doc.stores.retain(|node| node.id != deposit);
        let before = serde_json::to_value(&vault.doc).unwrap();
        let revision = vault.session.revision();
        assert!(create_from_template(&mut vault, &id, on_5_nov()).is_err());
        assert_eq!(serde_json::to_value(&vault.doc).unwrap(), before);
        assert_eq!(vault.session.revision(), revision);
    }

    /// A2 in full, and in one write: the income and its distribution cost the vault no more
    /// revisions than a plain income does.
    #[test]
    fn a2_is_one_write_with_every_effect() {
        let (_dir, mut vault, card, deposit) = setup_two();
        let id = salary_template(&mut vault, &card, &deposit);
        let start = vault.session.revision();
        create_from_template(&mut vault, &id, on_5_nov()).unwrap();
        let quick_add_cost = vault.session.revision() - start;
        assert_eq!(holding_of(&vault, &card, &rub()), amount("100000"));
        assert_eq!(holding_of(&vault, &deposit, &rub()), amount("20000"));
        let start = vault.session.revision();
        let mut plain = crate::service::test_support::income_of(&card, rub(), "5", "0.01", "x");
        at(&mut plain, "2026-11-05T10:00:00Z");
        add_transaction(&mut vault, plain).unwrap();
        assert_eq!(vault.session.revision() - start, quick_add_cost);
        let reserved: Vec<_> = vault
            .doc
            .envelopes
            .iter()
            .map(|envelope| (envelope.name.clone(), envelope.allocated))
            .collect();
        assert!(
            reserved.contains(&("Vacation".to_string(), amount("6000"))),
            "{reserved:?}"
        );
        assert!(
            reserved.contains(&("Gifts".to_string(), amount("3000"))),
            "{reserved:?}"
        );
    }

    /// An edited form result saves through the ordinary path: its period and its own parts win
    /// over the template's rule.
    #[test]
    fn explicit_period_and_parts_win_over_the_template() {
        let (_dir, mut vault, card, deposit) = setup_two();
        let mut tx = crate::service::test_support::income_of(&card, rub(), "1000", "0.01", "x");
        at(&mut tx, "2026-11-05T10:00:00Z");
        let period = mwt_model::OpPeriod {
            kind: mwt_model::PeriodKind::Month,
            start: "2026-09-01".into(),
            end: "2026-09-30".into(),
        };
        tx.meta_mut().period = Some(period.clone());
        if let Transaction::Topup(topup) = &mut tx {
            topup.distribution = Some(vec![
                mwt_model::DistributionPart {
                    kind: DistKind::Sum {
                        amount: amount("100"),
                    },
                    target: DistTarget::Store {
                        id: deposit.clone(),
                    },
                    amount: amount("100"),
                    status: mwt_model::PartStatus::Done,
                    effect: None,
                },
                mwt_model::DistributionPart {
                    kind: DistKind::Remainder,
                    target: DistTarget::None,
                    amount: amount("900"),
                    status: mwt_model::PartStatus::Done,
                    effect: None,
                },
            ]);
        }
        let rules = CreationRules {
            template_rule: Some(PeriodRule::PrevHalfSecond),
            no_period_rule: false,
        };
        let saved = add_transaction_ruled(&mut vault, tx, None, &rules).unwrap();
        assert_eq!(saved.meta().period, Some(period));
        assert_eq!(holding_of(&vault, &deposit, &rub()), amount("100"));
    }

    /// S1: parts set on an input saved under a template period rule are stored; the period stays.
    #[test]
    fn parts_on_an_input_under_a_template_rule_are_saved() {
        let (_dir, mut vault, card, deposit) = setup_two();
        let mut tx = crate::service::test_support::income_of(&card, rub(), "1000", "0.01", "x");
        at(&mut tx, "2026-11-05T10:00:00Z");
        let period = mwt_model::OpPeriod {
            kind: mwt_model::PeriodKind::Month,
            start: "2026-09-01".into(),
            end: "2026-09-30".into(),
        };
        tx.meta_mut().period = Some(period.clone());
        let parts = vec![
            ReceivedPart {
                store_id: card.clone(),
                amount: amount("400"),
                timestamp: None,
            },
            ReceivedPart {
                store_id: deposit.clone(),
                amount: amount("600"),
                timestamp: None,
            },
        ];
        if let Transaction::Topup(topup) = &mut tx {
            topup.parts = parts.clone();
        }
        let rules = CreationRules {
            template_rule: Some(PeriodRule::PrevHalfSecond),
            no_period_rule: false,
        };
        let saved = add_transaction_ruled(&mut vault, tx, None, &rules).unwrap();
        let Transaction::Topup(topup) = &saved else {
            panic!("an income");
        };
        assert_eq!(topup.parts, parts);
        assert_eq!(saved.meta().period, Some(period));
        assert_eq!(holding_of(&vault, &card, &rub()), amount("400"));
        assert_eq!(holding_of(&vault, &deposit, &rub()), amount("600"));
        let Transaction::Topup(stored) = &vault.doc.transactions[0] else {
            panic!("an income");
        };
        assert_eq!(stored.parts, parts);
    }

    #[test]
    fn explicit_fields_win_and_no_period_rule_means_none() {
        let (_dir, mut vault, card, deposit) = setup_two();
        let id = salary_template(&mut vault, &card, &deposit);
        vault.doc.operation_templates[0].distribution = None;
        let input = QuickAdd {
            store_id: Some(deposit.clone()),
            comment: Some("advance".into()),
            amount: Some(amount("500")),
            no_period_rule: true,
            ..on_5_nov()
        };
        let added = create_from_template(&mut vault, &id, input).unwrap();
        let Transaction::Topup(topup) = &added.transaction else {
            panic!("an income")
        };
        assert_eq!(topup.store_id, deposit);
        assert_eq!(topup.meta.comment, "advance");
        assert_eq!(topup.amount, amount("500"));
        assert!(topup.meta.period.is_none());
    }

    #[test]
    fn an_expense_template_makes_a_payment_with_its_period() {
        let (_dir, mut vault, card) = vault_with_card();
        let mut input = draft("Rent", TemplateKind::Expense);
        input.store_id = Some(card.clone());
        input.asset = Some(rub());
        input.amount = Some(amount("30000"));
        input.period_rule = Some(PeriodRule::PrevMonth);
        input.categories = vec!["groceries".into()];
        let id = create_operation_template(&mut vault, input).unwrap().id;
        let added = create_from_template(&mut vault, &id, on_5_nov()).unwrap();
        assert!(!added.ask);
        let Transaction::Payment(payment) = &added.transaction else {
            panic!("a payment")
        };
        assert_eq!(payment.amount, amount("30000"));
        let period = payment.meta.period.as_ref().unwrap();
        assert_eq!(
            (period.start.as_str(), period.end.as_str()),
            ("2026-10-01", "2026-10-31")
        );
    }
}
