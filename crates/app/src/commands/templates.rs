//! What the operation-template commands of `planning` run (spec op-period I13): the quick-add
//! translation between the DTOs and the usecase, and the end-to-end tests of the six commands.

use mwt_usecases::ipc_contract::{CreateFromTemplateInput, TemplateOperation};
use mwt_usecases::service::Session;
use mwt_usecases::usecases::planning::templates::{QuickAdd, create_from_template};

/// Quick-add: the template's operation, explicit fields winning; a distribution rule applies in
/// the same write, an `ask` rule only reports it. The booked operation is the first field.
pub(crate) fn quick_add(
    vault: &mut Session,
    input: CreateFromTemplateInput,
) -> mwt_base::Result<TemplateOperation> {
    let added = create_from_template(
        vault,
        &input.template_id,
        QuickAdd {
            timestamp: input.timestamp,
            amount: input.amount,
            store_id: input.store_id,
            comment: input.comment,
            no_period_rule: input.no_period_rule,
        },
    )?;
    Ok(TemplateOperation {
        transaction: added.transaction,
        ask: added.ask,
    })
}

/// The id of a booked operation, for the background enrichment.
pub(crate) fn operation_id(operation: &TemplateOperation) -> String {
    operation.transaction.meta().id.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mwt_base::Amount;
    use mwt_base::Error;
    use mwt_model::Transaction;
    use mwt_model::{
        AssetRef, DistKind, DistTarget, DistributionRow, DistributionRule, OnOtherAmount,
        PeriodRule, StoreKind, TemplateKind,
    };
    use mwt_planning::envelopes::EnvelopeDraft;
    use mwt_usecases::ipc_contract::OperationTemplateInput;
    use mwt_usecases::usecases::household::envelopes::create_envelope;
    use mwt_usecases::usecases::money::stores::create_store;
    use mwt_usecases::usecases::planning::templates::{
        archive_operation_template, create_operation_template, delete_operation_template,
        list_operation_templates, update_operation_template,
    };

    fn amount(raw: &str) -> Amount {
        Amount::parse(raw).unwrap()
    }

    fn rub() -> AssetRef {
        AssetRef::fiat("RUB", "Russian Ruble")
    }

    fn row(kind: DistKind, target: DistTarget) -> DistributionRow {
        DistributionRow { kind, target }
    }

    struct World {
        _dir: tempfile::TempDir,
        vault: Session,
        card: String,
        deposit: String,
        vacation: String,
        gifts: String,
    }

    fn world() -> World {
        let dir = tempfile::tempdir().unwrap();
        let mut vault = Session::create(dir.path().join("test.wallet"), "pw").unwrap();
        let mut bank = |name: &str| {
            create_store(&mut vault, name, StoreKind::Bank, "#000", "", None)
                .unwrap()
                .id
        };
        let card = bank("Card");
        let deposit = bank("Cushion");
        let mut envelope = |name: &str| {
            let draft = EnvelopeDraft {
                store_id: card.clone(),
                name: name.into(),
                asset: rub(),
                target: None,
                goal_id: None,
            };
            create_envelope(&mut vault, draft).unwrap().id
        };
        let vacation = envelope("Vacation");
        let gifts = envelope("Gifts");
        World {
            _dir: dir,
            vault,
            card,
            deposit,
            vacation,
            gifts,
        }
    }

    /// The A2 rule on the «Зарплата» template with the rule `prev_half_second`.
    fn salary(world: &World) -> OperationTemplateInput {
        OperationTemplateInput {
            name: "Zarplata".into(),
            kind: TemplateKind::Income,
            store_id: Some(world.card.clone()),
            asset: Some(rub()),
            amount: Some(amount("120000")),
            categories: vec!["salary".into()],
            comment: String::new(),
            contact_id: None,
            period_rule: Some(PeriodRule::PrevHalfSecond),
            distribution: Some(DistributionRule {
                rows: vec![
                    row(
                        DistKind::Sum {
                            amount: amount("20000"),
                        },
                        DistTarget::Store {
                            id: world.deposit.clone(),
                        },
                    ),
                    row(
                        DistKind::Percent { pct: amount("5") },
                        DistTarget::Envelope {
                            id: world.vacation.clone(),
                        },
                    ),
                    row(
                        DistKind::Sum {
                            amount: amount("3000"),
                        },
                        DistTarget::Envelope {
                            id: world.gifts.clone(),
                        },
                    ),
                    row(DistKind::Remainder, DistTarget::None),
                ],
                on_other_amount: OnOtherAmount::ShrinkRemainder,
                keep_on_card_min: None,
            }),
        }
    }

    fn on_5_nov(template_id: &str) -> CreateFromTemplateInput {
        CreateFromTemplateInput {
            template_id: template_id.into(),
            timestamp: Some("2026-11-05T10:00:00Z".into()),
            ..CreateFromTemplateInput::default()
        }
    }

    fn held(world: &World, store_id: &str) -> Amount {
        world
            .vault
            .doc
            .holdings
            .iter()
            .find(|holding| holding.store_id == store_id)
            .map_or(Amount::ZERO, |holding| holding.quantity)
    }

    #[test]
    fn a4b_quick_add_through_the_command_gives_the_period_and_the_a2_result() {
        let mut world = world();
        let input = salary(&world);
        let template = create_operation_template(&mut world.vault, input.to_draft()).unwrap();
        let done = quick_add(&mut world.vault, on_5_nov(&template.id)).unwrap();
        assert!(!done.ask);
        let Transaction::Topup(topup) = &done.transaction else {
            panic!("an income")
        };
        let period = topup.meta.period.as_ref().expect("a period");
        assert_eq!(
            (period.start.as_str(), period.end.as_str()),
            ("2026-10-16", "2026-10-31")
        );
        let amounts: Vec<Amount> = topup
            .distribution
            .iter()
            .flatten()
            .map(|part| part.amount)
            .collect();
        assert_eq!(
            amounts,
            ["20000", "6000", "3000", "91000"].map(amount).to_vec()
        );
        assert_eq!(held(&world, &world.deposit), amount("20000"));
        assert_eq!(held(&world, &world.card), amount("100000"));
        assert_eq!(operation_id(&done), topup.meta.id);
    }

    #[test]
    fn quick_add_of_an_unknown_template_is_refused() {
        let mut world = world();
        let error = quick_add(&mut world.vault, on_5_nov("missing")).unwrap_err();
        assert!(matches!(error, Error::Missing(_)), "{error}");
    }

    #[test]
    fn crud_through_the_usecases_the_commands_call() {
        let mut world = world();
        let input = salary(&world);
        let created = create_operation_template(&mut world.vault, input.to_draft()).unwrap();
        let mut edited = input;
        edited.name = "Avans".into();
        let updated =
            update_operation_template(&mut world.vault, &created.id, edited.to_draft()).unwrap();
        assert_eq!(updated.name, "Avans");
        assert!(
            archive_operation_template(&mut world.vault, &created.id, true)
                .unwrap()
                .archived
        );
        assert_eq!(list_operation_templates(&world.vault).len(), 1);
        delete_operation_template(&mut world.vault, &created.id).unwrap();
        assert!(list_operation_templates(&world.vault).is_empty());
    }

    #[test]
    fn delete_is_refused_while_an_automation_refers_to_the_template() {
        use mwt_model::Schedule;
        use mwt_usecases::ipc_contract::{AutomationInput, TransactionInput, TxCommonInput};
        use mwt_usecases::usecases::planning::automations::create_automation;

        let mut world = world();
        let input = salary(&world);
        let template = create_operation_template(&mut world.vault, input.to_draft()).unwrap();
        let automation = AutomationInput {
            name: "Payday".into(),
            active: true,
            preset: None,
            schedule: Schedule::Monthly { day: 5 },
            starts_at: "2026-01-01T00:00:00Z".into(),
            template: TransactionInput::Topup {
                common: TxCommonInput {
                    categories: vec!["salary".into()],
                    ..TxCommonInput::default()
                },
                store_id: world.card.clone(),
                asset: mwt_usecases::ipc_contract::AssetIdent {
                    symbol: "RUB".into(),
                    chain: Some(mwt_model::Chain::Fiat),
                    ..Default::default()
                },
                amount: amount("100"),
                fee: None,
                from: None,
                details: Default::default(),
                parts: Vec::new(),
            },
            amount_mode: Default::default(),
            workday_shift: Default::default(),
            time: None,
            asks: false,
            for_month_offset: None,
        };
        let made = create_automation(&mut world.vault, automation.to_draft().unwrap()).unwrap();
        let workflow = world
            .vault
            .doc
            .workflows
            .iter_mut()
            .find(|workflow| workflow.id == made.id)
            .unwrap();
        workflow.automation.as_mut().unwrap().operation_template_id = Some(template.id.clone());
        let error = delete_operation_template(&mut world.vault, &template.id).unwrap_err();
        let wire = crate::ipc::IpcError::from(error).to_string();
        assert!(wire.starts_with("INVALID_INPUT: "), "{wire}");
        assert!(wire.contains("Payday"), "{wire}");
        assert_eq!(list_operation_templates(&world.vault).len(), 1);
    }
}
