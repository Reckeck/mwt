use tauri::{AppHandle, State};

use mwt_base::Error;
use mwt_model::StoreKind;
use mwt_model::Transaction;
use mwt_model::TxProperty;
use mwt_model::{
    DistributionPart, DistributionRow, DistributionRule, EffectRef, PartStatus, TopupTx,
};
use mwt_planning::distribution::resolve_distribution;
use mwt_usecases::ipc_contract::{
    AddLinkedTransactionsInput, AdjustmentDraftInput, DistributeIncomeInput, DistributedIncome,
    DistributionPreview, EnvelopeMoveRef, IncomePart, IncomeSource, IncomeTree,
    PreviewDistributionInput, PreviewTransactionInput, TransactionInput, TransactionKinds,
    UpdateDistributionInput, UpdateTransactionInput,
};
use mwt_usecases::service::Session;
use mwt_usecases::usecases::planning::distribution;

use mwt_household::property_links::tx_property_links as property_link_reads;
use mwt_money::adjustments::AdjustmentDraft;
use mwt_money::ledger::{TxFilter, TxGraph, TxPage};
use mwt_money::preview::TxPreview;
use mwt_money::{preview, rebase};
use mwt_usecases::usecases::household::ledger;
use mwt_usecases::usecases::money::adjustments::adjustment_draft;
use mwt_usecases::usecases::money::transactions::CreationRules;
use mwt_usecases::usecases::money::{rates, transactions};

use crate::commands::AppState;
use crate::commands::app_settings::current_zone;
use crate::commands::enrichment::enrich_in_background;
use crate::commands::{with_vault, with_vault_mut};
use crate::ipc::IpcError;
use crate::ipc::commands::ipc_commands;

/// The operations list: every filter field is optional, `None` = everything.
#[tauri::command]
pub async fn list_transactions(
    filter: Option<TxFilter>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<Transaction>, IpcError> {
    let zone = current_zone(&app);
    with_vault(&state, |vault| {
        ledger::list_transactions(vault, &filter.unwrap_or_default(), &zone)
    })
}

/// The filtered operations as CSV through the native save dialog; `None`
/// when the user closed it.
#[tauri::command]
pub async fn export_transactions_csv(
    filter: Option<TxFilter>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<String>, IpcError> {
    let zone = current_zone(&app);
    let csv = {
        with_vault::<_, IpcError>(&state, |vault| {
            ledger::transactions_csv(vault, &filter.unwrap_or_default(), &zone)
        })?
    };
    crate::platform_fs::save_bytes_dialog(&app, "transactions.csv", "CSV", &["csv"], csv.as_bytes())
        .map_err(IpcError::from)
}

/// One page of the operations list (newest first) and the total that match.
#[tauri::command]
pub async fn list_transactions_page(
    filter: Option<TxFilter>,
    offset: u32,
    limit: u32,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<TxPage, IpcError> {
    let zone = current_zone(&app);
    with_vault(&state, |vault| {
        ledger::list_transactions_page(vault, &filter.unwrap_or_default(), offset, limit, &zone)
    })
}

/// The same operations as lanes and edges; `collapsed` folds a store's
/// sub-accounts into its own lane. More than 2000 nodes is `TOO_LARGE`.
#[tauri::command]
pub async fn get_tx_graph(
    filter: Option<TxFilter>,
    collapsed: Option<Vec<String>>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<TxGraph, IpcError> {
    let zone = current_zone(&app);
    with_vault(&state, |vault| {
        ledger::tx_graph(
            vault,
            &filter.unwrap_or_default(),
            &collapsed.unwrap_or_default(),
            &zone,
        )
    })
}

/// Every kind of transaction goes through here: build, auto-categorize,
/// give a period without one by the rules (`no_period_rule` opts out), validate, append, re-derive holdings, persist.
#[tauri::command]
pub async fn add_transaction(
    input: TransactionInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Transaction, IpcError> {
    let saved = {
        let mut input = input;
        with_vault_mut::<_, IpcError>(&state, |vault| {
            rates::resolve_input_price(vault, &mut input)?;
            let rules = CreationRules {
                no_period_rule: input.common().no_period_rule,
                ..CreationRules::default()
            };
            transactions::add_transaction_ruled(
                vault,
                input.to_transaction(),
                input.property_id().as_deref(),
                &rules,
            )
        })?
    };
    enrich_in_background(&app, saved.meta().id.clone());
    Ok(saved)
}

/// What a draft would do to every touched position; writes nothing. A draft
/// that would be refused still previews, with the refusal in `error`.
#[tauri::command]
pub async fn preview_transaction(
    input: PreviewTransactionInput,
    state: State<'_, AppState>,
) -> Result<TxPreview, IpcError> {
    with_vault(&state, |vault| {
        let mut draft = input.transaction;
        // A missing rate is not a preview error: the draft previews unpriced.
        if rates::resolve_input_price(vault, &mut draft).is_err() {
            draft.common_mut().price = None;
        }
        Ok(preview::preview_transaction(
            &vault.books(),
            draft.to_transaction(),
            input.replaces.as_deref(),
        ))
    })
}

/// Replace a transaction with an edited version; its id and origin are kept.
#[tauri::command]
pub async fn update_transaction(
    input: UpdateTransactionInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Transaction, IpcError> {
    let saved = {
        let mut draft = input.transaction;
        with_vault_mut::<_, IpcError>(&state, |vault| {
            rates::resolve_input_price(vault, &mut draft)?;
            let edited = rebase::rebase_transaction(
                &vault.doc,
                &input.id,
                input.base.as_ref(),
                draft.to_transaction(),
            )?;
            transactions::update_transaction_for(
                vault,
                &input.id,
                edited,
                draft.property_id().as_deref(),
            )
        })?
    };
    enrich_in_background(&app, saved.meta().id.clone());
    Ok(saved)
}

/// Several operations as one chain (an exchange split between recipients,
/// a P2P deal): all saved, or none.
#[tauri::command]
pub async fn add_linked_transactions(
    input: AddLinkedTransactionsInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<Transaction>, IpcError> {
    let saved = {
        with_vault_mut::<_, IpcError>(&state, |vault| {
            let mut txs = Vec::with_capacity(input.transactions.len());
            for mut draft in input.transactions {
                rates::resolve_input_price(vault, &mut draft)?;
                txs.push(draft.to_transaction());
            }
            transactions::add_linked_transactions(vault, txs, input.flow_id.as_deref())
        })?
    };
    for tx in &saved {
        enrich_in_background(&app, tx.meta().id.clone());
    }
    Ok(saved)
}

/// The correction screen of one account: its assets and their ledger
/// balances at the moment asked.
#[tauri::command]
pub async fn get_adjustment_draft(
    input: AdjustmentDraftInput,
    state: State<'_, AppState>,
) -> Result<AdjustmentDraft, IpcError> {
    with_vault(&state, |vault| adjustment_draft(&vault.doc, &input))
}

/// Which operation belongs to which property (plan G4).
#[tauri::command]
pub async fn tx_property_links(state: State<'_, AppState>) -> Result<Vec<TxProperty>, IpcError> {
    with_vault(&state, |vault| Ok(property_link_reads(&vault.doc)))
}

#[tauri::command]
pub async fn delete_transaction(id: String, state: State<'_, AppState>) -> Result<(), IpcError> {
    with_vault_mut(&state, |vault| transactions::delete_transaction(vault, &id))
}

/// Which transaction kinds a wallet of this kind accepts.
///
/// Pure: the binding is static domain data, so this needs no vault and takes no
/// lock. The frontend calls it with the kind of the wallet it is showing, which
/// keeps "what may a bank record" in exactly one place.
#[tauri::command]
pub async fn transaction_kinds_for_store_kind(
    kind: StoreKind,
) -> Result<TransactionKinds, IpcError> {
    Ok(TransactionKinds::for_store_kind(kind))
}

// -- income distribution (spec op-period I8-I10) -------------------------------------

/// The rows of a distribution against an income, the way a save would book them; writes nothing.
#[tauri::command]
pub async fn preview_distribution(
    input: PreviewDistributionInput,
    state: State<'_, AppState>,
) -> Result<DistributionPreview, IpcError> {
    with_vault(&state, |vault| preview_distribution_of(vault, input))
}

/// An income (a draft is saved first) and its distribution: the income, its transfers and its
/// envelope moves are written together or not at all.
#[tauri::command]
pub async fn distribute_income(
    input: DistributeIncomeInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DistributedIncome, IpcError> {
    let distributed = with_vault_mut::<_, IpcError>(&state, |vault| distribute(vault, input))?;
    enrich_in_background(&app, distributed.income.meta().id.clone());
    Ok(distributed)
}

/// The stored parts of an income, top to bottom, each with the record it produced; the marker
/// «разложено на N» is the parts without the remainder.
#[tauri::command]
pub async fn get_income_distribution(
    income_id: String,
    state: State<'_, AppState>,
) -> Result<IncomeTree, IpcError> {
    with_vault(&state, |vault| income_tree(vault, &income_id))
}

/// «Изменить раскладку»: the effects of dropped or changed parts go, the new ones are booked.
#[tauri::command]
pub async fn update_distribution(
    input: UpdateDistributionInput,
    state: State<'_, AppState>,
) -> Result<IncomeTree, IpcError> {
    with_vault_mut(&state, |vault| {
        distribution::replace_distribution(vault, &input.income_id, &input.rows)?;
        income_tree(vault, &input.income_id)
    })
}

/// Undo: every effect is removed and the income stores no distribution. Returns the income.
#[tauri::command]
pub async fn undo_distribution(
    income_id: String,
    state: State<'_, AppState>,
) -> Result<Transaction, IpcError> {
    with_vault_mut(&state, |vault| {
        distribution::remove_distribution(vault, &income_id)?;
        income_of(vault, &income_id).map(|topup| Transaction::Topup(topup.clone()))
    })
}

fn income_of<'doc>(vault: &'doc Session, id: &str) -> mwt_base::Result<&'doc TopupTx> {
    match vault.doc.transactions.iter().find(|tx| tx.meta().id == id) {
        Some(Transaction::Topup(topup)) => Ok(topup),
        Some(_) => Err(Error::InvalidInput("only an income distributes".into())),
        None => Err(Error::Missing(format!("income {id}"))),
    }
}

fn draft_income(draft: &TransactionInput) -> mwt_base::Result<TopupTx> {
    match draft.to_transaction() {
        Transaction::Topup(topup) => Ok(topup),
        _ => Err(Error::InvalidInput("only an income distributes".into())),
    }
}

/// The card as the rows meet it: what it holds, the income included, before any row. A stored
/// income already lost its transfers, so they are added back.
fn card_before_rows(
    vault: &Session,
    topup: &TopupTx,
    stored: bool,
) -> mwt_base::Result<mwt_base::Amount> {
    let held = vault
        .doc
        .holdings
        .iter()
        .find(|holding| {
            holding.store_id == topup.store_id && holding.asset.key() == topup.asset.key()
        })
        .map_or(mwt_base::Amount::ZERO, |holding| holding.quantity);
    if !stored {
        return Ok(held + topup.amount);
    }
    let moved = topup
        .distribution
        .iter()
        .flatten()
        .filter(|part| matches!(part.effect, Some(EffectRef::Transfer { .. })))
        .fold(mwt_base::Amount::ZERO, |sum, part| sum + part.amount);
    Ok(held + moved)
}

fn preview_distribution_of(
    vault: &Session,
    input: PreviewDistributionInput,
) -> mwt_base::Result<DistributionPreview> {
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
        card_before_rows(vault, &topup, stored)?,
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
            amount: mwt_base::Amount::ZERO,
            status: PartStatus::Done,
            effect: None,
        })
        .collect()
}

fn distribute(
    vault: &mut Session,
    input: DistributeIncomeInput,
) -> mwt_base::Result<DistributedIncome> {
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
                return Err(Error::InvalidInput("only an income distributes".into()));
            };
            topup.distribution = Some(carried_list(input.rows));
            let saved = transactions::add_transaction_ruled(
                vault,
                income,
                draft.property_id().as_deref(),
                &rules,
            )?;
            saved.meta().id.clone()
        }
        IncomeSource::Stored { id } => {
            distribution::apply_distribution(vault, &id, &input.rows)?;
            id
        }
    };
    distributed_of(vault, &income_id)
}

fn distributed_of(vault: &Session, income_id: &str) -> mwt_base::Result<DistributedIncome> {
    let income = income_of(vault, income_id)?;
    let mut transfers = Vec::new();
    let mut envelope_moves = Vec::new();
    for part in income.distribution.iter().flatten() {
        match &part.effect {
            Some(EffectRef::Transfer { tx_id }) => transfers.extend(
                vault
                    .doc
                    .transactions
                    .iter()
                    .find(|tx| &tx.meta().id == tx_id)
                    .cloned(),
            ),
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

fn income_tree(vault: &Session, income_id: &str) -> mwt_base::Result<IncomeTree> {
    let income = income_of(vault, income_id)?;
    let parts = income
        .distribution
        .iter()
        .flatten()
        .map(|part| IncomePart {
            part: part.clone(),
            transfer: match &part.effect {
                Some(EffectRef::Transfer { tx_id }) => vault
                    .doc
                    .transactions
                    .iter()
                    .find(|tx| &tx.meta().id == tx_id)
                    .cloned(),
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

ipc_commands!(transactions_commands, TRANSACTIONS_NAMES, commands::transactions: [list_transactions, export_transactions_csv, list_transactions_page, get_tx_graph, add_transaction, preview_transaction, update_transaction, add_linked_transactions, preview_distribution, distribute_income, get_income_distribution, update_distribution, undo_distribution, get_adjustment_draft, tx_property_links, delete_transaction, transaction_kinds_for_store_kind]);

#[cfg(test)]
mod distribution_tests {
    use super::*;
    use mwt_base::Amount;
    use mwt_model::{DistKind, DistTarget, KindDetails};
    use mwt_planning::envelopes::EnvelopeDraft;
    use mwt_usecases::ipc_contract::{AssetIdent, TxCommonInput};
    use mwt_usecases::usecases::household::envelopes::create_envelope;
    use mwt_usecases::usecases::money::stores::create_store;

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
        distribute(&mut world.vault, input).unwrap()
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
        distribution::remove_distribution(&mut world.vault, &id).unwrap();
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
        let preview = preview_distribution_of(
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
        let again = preview_distribution_of(
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
        assert!(distribute(&mut world.vault, input).is_err());
    }
}
