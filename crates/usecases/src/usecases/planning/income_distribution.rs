//! Income distribution on the IPC side (spec op-period I8-I10): preview, book, read and the
//! undo answer. The rows are resolved and booked in `planning::distribution`; this module only
//! joins a draft or stored income with them and shapes the answers the UI reads.

use crate::Amount;
use crate::error::{Error, Result};
use crate::ipc_contract::{
    DistributeIncomeInput, DistributedIncome, DistributionPreview, EnvelopeMoveRef, IncomePart,
    IncomeSource, IncomeTree, PreviewDistributionInput, TransactionInput,
};
use crate::planning::distribution::resolve_distribution;
use crate::service::Session;
use crate::usecases::money::rates;
use crate::usecases::money::transactions::{CreationRules, add_transaction_ruled};
use crate::usecases::planning::distribution::apply_distribution;
use crate::usecases::planning::templates::held_quantity;
use mwt_model::{
    DistributionPart, DistributionRow, DistributionRule, EffectRef, PartStatus, TopupTx,
    Transaction,
};

pub fn income_of<'doc>(vault: &'doc Session, id: &str) -> Result<&'doc TopupTx> {
    match vault.doc.transactions.iter().find(|tx| tx.meta().id == id) {
        Some(Transaction::Topup(topup)) => Ok(topup),
        Some(_) => Err(not_an_income()),
        None => Err(Error::Missing(format!("income {id}"))),
    }
}

fn draft_income(draft: &TransactionInput) -> Result<TopupTx> {
    match draft.to_transaction() {
        Transaction::Topup(topup) => Ok(topup),
        _ => Err(not_an_income()),
    }
}

/// The card as the rows meet it: what it holds, the income included, before any row. A stored
/// income already lost its transfers, so they are added back.
fn card_before_rows(vault: &Session, topup: &TopupTx, stored: bool) -> Amount {
    let with_income = held_quantity(vault, topup);
    if !stored {
        return with_income;
    }
    let moved = topup
        .distribution
        .iter()
        .flatten()
        .filter(|part| matches!(part.effect, Some(EffectRef::Transfer { .. })))
        .fold(Amount::ZERO, |sum, part| sum + part.amount);
    with_income - topup.amount + moved
}

pub fn preview_income_distribution(
    vault: &Session,
    input: PreviewDistributionInput,
) -> Result<DistributionPreview> {
    let (topup, stored) = match &input.income {
        IncomeSource::Draft { transaction } => (draft_income(transaction)?, false),
        IncomeSource::Stored { id } => (income_of(vault, id)?.clone(), true),
    };
    let rule = DistributionRule {
        rows: input.rows,
        on_other_amount: Default::default(),
        keep_on_card_min: input.keep_on_card_min,
    };
    let resolved = resolve_distribution(
        &rule,
        topup.amount,
        topup.amount,
        card_before_rows(vault, &topup, stored),
        topup.asset.minor_places(),
    )?;
    Ok(DistributionPreview::of(topup.amount, None, None, resolved))
}

/// The rows as a list the income carries into `add_transaction_ruled`, which takes the list off
/// and books it with the income; the amounts are placeholders, the usecase resolves them.
fn carried_list(rows: Vec<DistributionRow>) -> Vec<DistributionPart> {
    rows.into_iter()
        .map(|row| DistributionPart {
            kind: row.kind,
            target: row.target,
            amount: Amount::ZERO,
            status: PartStatus::Done,
            effect: None,
        })
        .collect()
}

pub fn distribute_income(
    vault: &mut Session,
    input: DistributeIncomeInput,
) -> Result<DistributedIncome> {
    let income_id = match input.income {
        IncomeSource::Draft { transaction } => {
            let mut draft = *transaction;
            rates::resolve_input_price(vault, &mut draft)?;
            let rules = CreationRules {
                no_period_rule: draft.common().no_period_rule,
                ..CreationRules::default()
            };
            let mut income = draft.to_transaction();
            let Transaction::Topup(topup) = &mut income else {
                return Err(not_an_income());
            };
            topup.distribution = Some(carried_list(input.rows));
            let saved =
                add_transaction_ruled(vault, income, draft.property_id().as_deref(), &rules)?;
            saved.meta().id.clone()
        }
        IncomeSource::Stored { id } => {
            apply_distribution(vault, &id, &input.rows)?;
            id
        }
    };
    distributed_of(vault, &income_id)
}

fn distributed_of(vault: &Session, income_id: &str) -> Result<DistributedIncome> {
    let income = income_of(vault, income_id)?;
    let mut transfers = Vec::new();
    let mut envelope_moves = Vec::new();
    for part in income.distribution.iter().flatten() {
        match &part.effect {
            Some(EffectRef::Transfer { tx_id }) => transfers.extend(transfer_of(vault, tx_id)),
            Some(EffectRef::EnvelopeMove {
                envelope_id,
                move_id,
            }) => envelope_moves.push(EnvelopeMoveRef {
                envelope_id: envelope_id.clone(),
                move_id: move_id.clone(),
                amount: part.amount,
            }),
            None => {}
        }
    }
    Ok(DistributedIncome {
        income: Transaction::Topup(income.clone()),
        transfers,
        envelope_moves,
    })
}

pub fn income_tree(vault: &Session, income_id: &str) -> Result<IncomeTree> {
    let income = income_of(vault, income_id)?;
    let parts = income
        .distribution
        .iter()
        .flatten()
        .map(|part| IncomePart {
            part: part.clone(),
            transfer: match &part.effect {
                Some(EffectRef::Transfer { tx_id }) => transfer_of(vault, tx_id),
                _ => None,
            },
            envelope_move: match &part.effect {
                Some(EffectRef::EnvelopeMove {
                    envelope_id,
                    move_id,
                }) => vault
                    .doc
                    .envelopes
                    .iter()
                    .find(|envelope| &envelope.id == envelope_id)
                    .and_then(|envelope| envelope.moves.iter().find(|entry| &entry.id == move_id))
                    .cloned(),
                _ => None,
            },
        })
        .collect();
    Ok(IncomeTree {
        income: Transaction::Topup(income.clone()),
        parts,
    })
}

fn transfer_of(vault: &Session, tx_id: &str) -> Option<Transaction> {
    vault
        .doc
        .transactions
        .iter()
        .find(|tx| tx.meta().id == tx_id)
        .cloned()
}

fn not_an_income() -> Error {
    Error::InvalidInput("only an income distributes".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc_contract::{AssetIdent, TxCommonInput};
    use crate::usecases::household::envelopes::create_envelope;
    use crate::usecases::money::stores::create_store;
    use mwt_model::StoreKind;
    use mwt_model::{DistKind, DistTarget, KindDetails};
    use mwt_planning::envelopes::EnvelopeDraft;

    fn rub() -> AssetIdent {
        AssetIdent {
            symbol: "RUB".into(),
            chain: Some(mwt_model::Chain::Fiat),
            ..AssetIdent::default()
        }
    }

    fn amount(raw: &str) -> Amount {
        Amount::parse(raw).unwrap()
    }

    struct World {
        _dir: tempfile::TempDir,
        vault: Session,
        card: String,
        deposit: String,
        vacation: String,
        gifts: String,
    }

    fn envelope_on(vault: &mut Session, card: &str, name: &str) -> String {
        let draft = EnvelopeDraft {
            store_id: card.into(),
            name: name.into(),
            asset: rub().to_asset(),
            target: None,
            goal_id: None,
        };
        create_envelope(vault, draft).unwrap().id
    }

    /// A card, a deposit and two envelopes on the card; no income yet.
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
        let vacation = envelope_on(&mut vault, &card, "Vacation");
        let gifts = envelope_on(&mut vault, &card, "Gifts");
        World {
            _dir: dir,
            vault,
            card,
            deposit,
            vacation,
            gifts,
        }
    }

    fn salary(world: &World) -> TransactionInput {
        TransactionInput::Topup {
            common: TxCommonInput {
                categories: vec!["salary".into()],
                price_usd: Some(amount("0.01")),
                ..TxCommonInput::default()
            },
            store_id: world.card.clone(),
            asset: rub(),
            amount: amount("120000"),
            fee: None,
            from: None,
            details: KindDetails::default(),
            parts: Vec::new(),
        }
    }

    fn row(kind: DistKind, target: DistTarget) -> DistributionRow {
        DistributionRow { kind, target }
    }

    /// A2: 20 000 to the deposit, 5 % and 3 000 to two envelopes, the rest stays.
    fn a2_rows(world: &World) -> Vec<DistributionRow> {
        vec![
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
        ]
    }

    fn distribute_draft(world: &mut World) -> DistributedIncome {
        let input = DistributeIncomeInput {
            income: IncomeSource::Draft {
                transaction: Box::new(salary(world)),
            },
            rows: a2_rows(world),
        };
        distribute_income(&mut world.vault, input).unwrap()
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
    fn a2_through_distribute_income_books_the_income_and_every_effect() {
        let mut world = world();
        let done = distribute_draft(&mut world);
        assert_eq!(done.transfers.len(), 1);
        assert_eq!(done.envelope_moves.len(), 2);
        assert_eq!(held(&world, &world.card), amount("100000"));
        assert_eq!(held(&world, &world.deposit), amount("20000"));
        let tree = income_tree(&world.vault, &done.income.meta().id).unwrap();
        let amounts: Vec<Amount> = tree.parts.iter().map(|entry| entry.part.amount).collect();
        assert_eq!(
            amounts,
            ["20000", "6000", "3000", "91000"].map(amount).to_vec()
        );
    }

    #[test]
    fn get_income_distribution_returns_four_parts_with_their_records() {
        let mut world = world();
        let done = distribute_draft(&mut world);
        let tree = income_tree(&world.vault, &done.income.meta().id).unwrap();
        assert_eq!(tree.parts.len(), 4);
        assert_eq!(tree.distributed_count(), 3);
        assert!(tree.parts[0].transfer.is_some());
        assert_eq!(
            tree.parts[1].envelope_move.as_ref().unwrap().delta,
            amount("6000")
        );
        assert!(tree.parts[3].transfer.is_none() && tree.parts[3].envelope_move.is_none());
    }

    #[test]
    fn undo_distribution_leaves_no_effect() {
        let mut world = world();
        let done = distribute_draft(&mut world);
        let id = done.income.meta().id.clone();
        crate::usecases::planning::distribution::remove_distribution(&mut world.vault, &id)
            .unwrap();
        assert_eq!(income_tree(&world.vault, &id).unwrap().parts.len(), 0);
        let transfers = world
            .vault
            .doc
            .transactions
            .iter()
            .filter(|tx| matches!(tx, Transaction::Transfer(_)))
            .count();
        assert_eq!(transfers, 0);
        assert!(world.vault.doc.envelopes.iter().all(|envelope| {
            envelope
                .moves
                .iter()
                .all(|entry| entry.income_ref.is_none())
        }));
        assert_eq!(held(&world, &world.card), amount("120000"));
        assert_eq!(held(&world, &world.deposit), Amount::ZERO);
    }

    #[test]
    fn preview_equals_the_booked_result() {
        let mut world = world();
        let preview = preview_income_distribution(
            &world.vault,
            PreviewDistributionInput {
                income: IncomeSource::Draft {
                    transaction: Box::new(salary(&world)),
                },
                rows: a2_rows(&world),
                keep_on_card_min: None,
            },
        )
        .unwrap();
        let done = distribute_draft(&mut world);
        let tree = income_tree(&world.vault, &done.income.meta().id).unwrap();
        let previewed: Vec<Amount> = preview.rows.iter().map(|row| row.amount).collect();
        let booked: Vec<Amount> = tree.parts.iter().map(|entry| entry.part.amount).collect();
        assert_eq!(previewed, booked);
        assert_eq!(preview.remainder, amount("91000"));
        assert!(preview.over.is_none() && preview.cut.is_empty());
        let again = preview_income_distribution(
            &world.vault,
            PreviewDistributionInput {
                income: IncomeSource::Stored {
                    id: done.income.meta().id.clone(),
                },
                rows: a2_rows(&world),
                keep_on_card_min: None,
            },
        )
        .unwrap();
        assert_eq!(again, preview);
    }

    #[test]
    fn distributing_a_stored_income_twice_is_refused() {
        let mut world = world();
        let done = distribute_draft(&mut world);
        let input = DistributeIncomeInput {
            income: IncomeSource::Stored {
                id: done.income.meta().id.clone(),
            },
            rows: a2_rows(&world),
        };
        assert!(distribute_income(&mut world.vault, input).is_err());
    }
}
