//! The one usecase behind an income that distributes itself (spec S8, S11): apply, replace and
//! remove a distribution. Rows are resolved and planned in `planning::distribution`, the result is
//! checked by `money::distribution`; this module only books the effects. Everything runs on one
//! document and is committed once: on any error the document is put back as it was, so nothing
//! is written.

use crate::Amount;
use crate::error::{Error, Result};
use crate::model::ids::StoreId;
use crate::model::vault::now_iso;
use crate::money::distribution::{validate_distribution_targets, verify_distribution};
use crate::money::transactions::ensure_fresh_id;
use crate::planning::distribution::{plan_distribution, resolve_rows};
use crate::planning::envelopes::{EnvelopeDraft, free_balance, new_envelope, plan_fund_goal};
use crate::service::Session;
use crate::usecases::money::portfolio::recompute_holdings;
use crate::usecases::money::stores::rebase_and_commit;
use crate::usecases::money::transactions::prepare_transaction;
use mwt_model::{
    AccountRef, AssetRef, DistKind, DistTarget, DistributionPart, DistributionRow, EffectRef,
    TopupTx, Transaction, TransferTx, TxKind, TxMeta, VaultDoc,
};

/// Stable code of a distribution whose rows ask for more than the income.
pub const OVER_ALLOCATED: &str = "DISTRIBUTION_OVER_ALLOCATED";
/// Stable code of an income that already distributes (apply is not replace).
pub const ALREADY_DISTRIBUTED: &str = "DISTRIBUTION_ALREADY_DISTRIBUTED";

/// What the income needs to know to book effects.
struct Income {
    id: String,
    store_id: String,
    asset: AssetRef,
    amount: Amount,
    timestamp: String,
    old_parts: Vec<DistributionPart>,
}

/// Distribute an income that does not distribute yet: one effect per non-remainder row, the list
/// stored on the income (spec S8). Returns the stored parts.
pub fn apply_distribution(
    vault: &mut Session,
    income_id: &str,
    rows: &[DistributionRow],
) -> Result<Vec<DistributionPart>> {
    if income_of(&vault.doc, income_id)?.distribution.is_some() {
        return Err(Error::InvalidInput(format!(
            "{ALREADY_DISTRIBUTED}: replace the distribution instead"
        )));
    }
    rewrite(vault, income_id, Some(rows))
}

/// Edit the distribution (spec S11, A2b): the effects of dropped or changed parts are removed, the
/// new ones created, an unchanged part keeps its effect. Distributes an income that did not.
pub fn replace_distribution(
    vault: &mut Session,
    income_id: &str,
    rows: &[DistributionRow],
) -> Result<Vec<DistributionPart>> {
    rewrite(vault, income_id, Some(rows))
}

/// Undo: every effect is removed and the income stores no distribution.
pub fn remove_distribution(vault: &mut Session, income_id: &str) -> Result<()> {
    rewrite(vault, income_id, None).map(|_| ())
}

fn income_of<'doc>(doc: &'doc VaultDoc, id: &str) -> Result<&'doc TopupTx> {
    match doc.transactions.iter().find(|tx| tx.meta().id == id) {
        Some(Transaction::Topup(topup)) => Ok(topup),
        Some(_) => Err(Error::InvalidInput(
            "only an income can be distributed".into(),
        )),
        None => Err(Error::Missing(format!("income {id}"))),
    }
}

fn read_income(vault: &Session, income_id: &str) -> Result<Income> {
    let topup = income_of(&vault.doc, income_id)?;
    if !topup.parts.is_empty() {
        return Err(Error::InvalidInput(
            "an income cannot arrive in parts and distribute".into(),
        ));
    }
    Ok(Income {
        id: income_id.to_string(),
        store_id: topup.store_id.clone(),
        asset: topup.asset.clone(),
        amount: topup.amount,
        timestamp: topup.meta.timestamp.clone(),
        old_parts: topup.distribution.clone().unwrap_or_default(),
    })
}

/// `rows` of `None` removes the distribution.
fn rewrite(
    vault: &mut Session,
    income_id: &str,
    rows: Option<&[DistributionRow]>,
) -> Result<Vec<DistributionPart>> {
    let income = read_income(vault, income_id)?;
    let wanted = match rows {
        Some(rows) => resolve(vault, &income, rows)?,
        None => Vec::new(),
    };
    let plan = plan_distribution(&income.old_parts, &wanted);
    let backup = vault.doc.clone();
    let stored = book(vault, &income, plan.remove, plan.parts, rows.is_some());
    match stored {
        Ok(parts) => {
            rebase_and_commit(vault)?;
            Ok(parts)
        }
        Err(err) => {
            vault.doc = backup;
            Err(err)
        }
    }
}

/// The rows against the income, refused when they ask too much or name a bad target.
fn resolve(
    vault: &Session,
    income: &Income,
    rows: &[DistributionRow],
) -> Result<Vec<DistributionPart>> {
    let resolved = resolve_rows(income.amount, rows, income.asset.minor_places())?;
    if let Some(over) = resolved.over {
        return Err(Error::InvalidInput(format!(
            "{OVER_ALLOCATED}: the rows ask {over} more than the income"
        )));
    }
    if let Some(part) = resolved
        .parts
        .iter()
        .find(|part| !matches!(part.kind, DistKind::Remainder) && !part.amount.is_positive())
    {
        return Err(Error::InvalidInput(format!(
            "{}: a part of {} is not allowed",
            crate::money::distribution::PART_NOT_POSITIVE,
            part.amount
        )));
    }
    validate_distribution_targets(&vault.doc, TxKind::Topup, &resolved.parts)?;
    Ok(resolved.parts)
}

/// Removes the dropped effects, creates the missing ones, stores the list and checks V1-V4.
fn book(
    vault: &mut Session,
    income: &Income,
    remove: Vec<EffectRef>,
    mut parts: Vec<DistributionPart>,
    keep_list: bool,
) -> Result<Vec<DistributionPart>> {
    for effect in &remove {
        drop_effect(vault, effect);
    }
    recompute_holdings(vault);
    for part in parts
        .iter_mut()
        .filter(|part| part.effect.is_none() && !matches!(part.kind, DistKind::Remainder))
    {
        part.effect = Some(create_effect(vault, income, part)?);
    }
    store_list(vault, &income.id, keep_list.then(|| parts.clone()))?;
    let violations = verify_distribution(&vault.books(), &income.id);
    if violations.is_empty() {
        return Ok(parts);
    }
    let codes: Vec<&str> = violations.iter().map(|found| found.code).collect();
    Err(Error::InvalidInput(format!(
        "the distribution breaks its invariants: {}",
        codes.join(", ")
    )))
}

fn drop_effect(vault: &mut Session, effect: &EffectRef) {
    match effect {
        EffectRef::Transfer { tx_id } => vault.doc.transactions.retain(|tx| &tx.meta().id != tx_id),
        EffectRef::EnvelopeMove {
            envelope_id,
            move_id,
        } => {
            if let Some(envelope) = vault
                .doc
                .envelopes
                .iter_mut()
                .find(|envelope| &envelope.id == envelope_id)
            {
                envelope.moves.retain(|entry| &entry.id != move_id);
                envelope.allocated = envelope.moves_sum();
            }
        }
    }
}

fn create_effect(
    vault: &mut Session,
    income: &Income,
    part: &DistributionPart,
) -> Result<EffectRef> {
    match &part.target {
        DistTarget::Store { id } => create_transfer(vault, income, id, part.amount),
        DistTarget::Envelope { id } => {
            let envelope_id = id.clone();
            fill_envelope(vault, income, &envelope_id, part.amount)
        }
        DistTarget::Goal { goal_id, store_id } => {
            let envelope_id = goal_envelope(vault, income, goal_id, store_id, part.amount)?;
            fill_envelope(vault, income, &envelope_id, part.amount)
        }
        DistTarget::None => Err(Error::InvalidInput(
            "only the remainder may stay on the card".into(),
        )),
    }
}

fn create_transfer(
    vault: &mut Session,
    income: &Income,
    target_store: &str,
    amount: Amount,
) -> Result<EffectRef> {
    let mut meta = TxMeta::new("");
    meta.timestamp = income.timestamp.clone();
    meta.income_ref = Some(income.id.clone());
    let mut transfer = Transaction::Transfer(TransferTx {
        meta,
        from: AccountRef::Store(income.store_id.clone()),
        to: AccountRef::Store(target_store.to_string()),
        asset: income.asset.clone(),
        amount,
        fee: None,
        network: None,
        tx_hash: None,
        debt: None,
        price: Default::default(),
    });
    prepare_transaction(vault, &mut transfer)?;
    ensure_fresh_id(&vault.doc, &mut transfer);
    let tx_id = transfer.meta().id.clone();
    vault.doc.transactions.push(transfer);
    recompute_holdings(vault);
    Ok(EffectRef::Transfer { tx_id })
}

/// The envelope of a goal on the income's card, made when missing (as `fund_goal`), without a save.
fn goal_envelope(
    vault: &mut Session,
    income: &Income,
    goal_id: &str,
    store_id: &str,
    amount: Amount,
) -> Result<String> {
    if store_id != income.store_id {
        return Err(Error::InvalidInput(
            "a goal row can only be funded on the card the income arrived on".into(),
        ));
    }
    let funding = plan_fund_goal(&vault.doc, goal_id, &StoreId::from(store_id), amount)?;
    if funding.take < amount {
        return Err(not_enough_free(funding.take, &funding.asset.symbol));
    }
    if let Some(id) = funding.envelope_id {
        return Ok(id);
    }
    let id = crate::sync::sync_ids::goal_envelope_id(goal_id, store_id, &funding.asset.key());
    let draft = EnvelopeDraft {
        store_id: store_id.to_string(),
        name: funding.goal_name,
        asset: funding.asset,
        target: None,
        goal_id: Some(goal_id.to_string()),
    };
    vault
        .doc
        .envelopes
        .push(new_envelope(draft, id.clone(), now_iso()));
    Ok(id)
}

fn fill_envelope(
    vault: &mut Session,
    income: &Income,
    envelope_id: &str,
    amount: Amount,
) -> Result<EffectRef> {
    let envelope = vault
        .doc
        .envelopes
        .iter()
        .find(|envelope| envelope.id == envelope_id)
        .ok_or_else(|| Error::Missing(format!("envelope {envelope_id}")))?;
    if envelope.store_id != income.store_id || envelope.asset.key() != income.asset.key() {
        return Err(Error::InvalidInput(
            "the envelope must sit on the card and the asset of the income".into(),
        ));
    }
    let free = free_balance(&vault.doc, &StoreId::from(&income.store_id), &income.asset);
    if free < amount {
        return Err(not_enough_free(free, &income.asset.symbol));
    }
    let envelope = vault
        .doc
        .envelopes
        .iter_mut()
        .find(|envelope| envelope.id == envelope_id)
        .ok_or_else(|| Error::Missing(format!("envelope {envelope_id}")))?;
    envelope.record_move(amount, &income.timestamp);
    let entry = envelope
        .moves
        .last_mut()
        .ok_or_else(|| Error::InvalidInput("the envelope kept no entry for the move".into()))?;
    entry.income_ref = Some(income.id.clone());
    Ok(EffectRef::EnvelopeMove {
        envelope_id: envelope_id.to_string(),
        move_id: entry.id.clone(),
    })
}

fn not_enough_free(free: Amount, symbol: &str) -> Error {
    Error::InsufficientFunds(format!("only {free} {symbol} is free on the card"))
}

fn store_list(
    vault: &mut Session,
    income_id: &str,
    list: Option<Vec<DistributionPart>>,
) -> Result<()> {
    let topup = vault
        .doc
        .transactions
        .iter_mut()
        .find_map(|tx| match tx {
            Transaction::Topup(topup) if topup.meta.id == income_id => Some(topup),
            _ => None,
        })
        .ok_or_else(|| Error::Missing(format!("income {income_id}")))?;
    topup.distribution = list;
    Ok(())
}
