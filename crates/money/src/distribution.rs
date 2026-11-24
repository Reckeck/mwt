//! Integrity of an income that distributes itself (spec S6, S9, S11 V1-V4, S14): the pure checks
//! behind `verify_distribution` and the own-target rules. No vault, no writes: the usecase runs
//! them before it stores, the repair pass runs them after a sync.

use std::collections::HashMap;

use mwt_base::Amount;
use mwt_base::error::{Error, Result};
use mwt_model::{
    AccountRef, DistKind, DistTarget, DistributionPart, EffectRef, Envelope, TopupTx, Transaction,
    TxKind, VaultDoc,
};

use super::books::Books;
use super::validate::live_store;

/// V1: the parts do not add up to the income, or the remainder is not exactly one.
pub const SUM_MISMATCH: &str = "DISTRIBUTION_SUM_MISMATCH";
/// V1: a distribution has no remainder part or more than one.
pub const REMAINDER_COUNT: &str = "DISTRIBUTION_REMAINDER_COUNT";
/// V1: a part other than the remainder has an amount of zero or less.
pub const PART_NOT_POSITIVE: &str = "DISTRIBUTION_PART_NOT_POSITIVE";
/// V2: a part has no effect, or the remainder has one.
pub const EFFECT_SHAPE: &str = "DISTRIBUTION_EFFECT_SHAPE";
/// V2: the effect record does not exist.
pub const EFFECT_MISSING: &str = "DISTRIBUTION_EFFECT_MISSING";
/// V2: the effect exists but disagrees with the part (amount, asset, source, target or back-reference).
pub const EFFECT_MISMATCH: &str = "DISTRIBUTION_EFFECT_MISMATCH";
/// V3: a record points back to an income but no part of it names the record.
pub const EFFECT_ORPHAN: &str = "DISTRIBUTION_EFFECT_ORPHAN";
/// V3: one record is named by more than one part.
pub const EFFECT_SHARED: &str = "DISTRIBUTION_EFFECT_SHARED";
/// V3: records point back to an income that does not exist.
pub const INCOME_MISSING: &str = "DISTRIBUTION_INCOME_MISSING";
/// A distribution on a record that is not an income (spec S14: only a top-up distributes).
pub const NOT_INCOME: &str = "DISTRIBUTION_NOT_INCOME";
/// V4: the income has both `parts` and a `distribution`.
pub const WITH_PARTS: &str = "DISTRIBUTION_WITH_PARTS";
/// A part's target is missing, foreign, archived or not allowed for its kind.
pub const BAD_TARGET: &str = "DISTRIBUTION_BAD_TARGET";

/// One broken invariant, with a stable `code` for callers and tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    pub code: &'static str,
    pub message: String,
}

fn violation(code: &'static str, message: impl Into<String>) -> Violation {
    Violation {
        code,
        message: message.into(),
    }
}

/// Checks V1-V4 of income `income_id` against the vault. An income that is not distributed is
/// clean unless a record still points back to it.
pub fn verify_distribution(books: &Books<'_>, income_id: &str) -> Vec<Violation> {
    check_distribution(books.doc(), income_id)
}

/// [`verify_distribution`] over a bare document.
pub fn check_distribution(doc: &VaultDoc, income_id: &str) -> Vec<Violation> {
    let referenced = referencing_records(doc, income_id);
    let income = doc.transactions.iter().find(|tx| tx.meta().id == income_id);
    let Some(income) = income else {
        return if referenced.is_empty() {
            Vec::new()
        } else {
            vec![violation(
                INCOME_MISSING,
                format!(
                    "{} record(s) point to the missing income {income_id}",
                    referenced.len()
                ),
            )]
        };
    };
    let Transaction::Topup(topup) = income else {
        return vec![violation(
            NOT_INCOME,
            format!("{income_id} is a {}, not an income", income.tag()),
        )];
    };
    let mut found = Vec::new();
    let parts = topup.distribution.as_deref().unwrap_or_default();
    if topup.distribution.is_some() && !topup.parts.is_empty() {
        found.push(violation(
            WITH_PARTS,
            "an income cannot arrive in parts and distribute",
        ));
    }
    if topup.distribution.is_some() {
        found.extend(check_amounts(topup, parts));
        for part in parts {
            found.extend(check_effect(doc, income_id, topup, part));
        }
    }
    found.extend(check_back_references(&referenced, parts));
    found
}

/// V1: sum, exactly one remainder, every other part positive.
fn check_amounts(topup: &TopupTx, parts: &[DistributionPart]) -> Vec<Violation> {
    let mut found = Vec::new();
    let remainders = parts
        .iter()
        .filter(|part| matches!(part.kind, DistKind::Remainder))
        .count();
    if remainders != 1 {
        found.push(violation(
            REMAINDER_COUNT,
            format!("a distribution needs exactly one remainder, got {remainders}"),
        ));
    }
    let total = parts
        .iter()
        .fold(Amount::ZERO, |sum, part| sum + part.amount);
    if total != topup.amount {
        found.push(violation(
            SUM_MISMATCH,
            format!(
                "the parts add up to {total}, the income is {}",
                topup.amount
            ),
        ));
    }
    for part in parts {
        let remainder = matches!(part.kind, DistKind::Remainder);
        if (remainder && part.amount.is_negative()) || (!remainder && !part.amount.is_positive()) {
            found.push(violation(
                PART_NOT_POSITIVE,
                format!("a part of {} is not allowed", part.amount),
            ));
        }
    }
    found
}

/// V2: the effect of one part exists and agrees with it.
fn check_effect(
    doc: &VaultDoc,
    income_id: &str,
    topup: &TopupTx,
    part: &DistributionPart,
) -> Option<Violation> {
    if matches!(part.kind, DistKind::Remainder) {
        return part
            .effect
            .is_some()
            .then(|| violation(EFFECT_SHAPE, "the remainder has no effect"));
    }
    let Some(effect) = &part.effect else {
        return Some(violation(EFFECT_SHAPE, "a part without an effect"));
    };
    let agrees = match effect {
        EffectRef::Transfer { tx_id } => {
            let found = doc.transactions.iter().find(|tx| &tx.meta().id == tx_id);
            let Some(found) = found else {
                return Some(violation(
                    EFFECT_MISSING,
                    format!("transfer {tx_id} is gone"),
                ));
            };
            transfer_agrees(found, income_id, topup, part)
        }
        EffectRef::EnvelopeMove {
            envelope_id,
            move_id,
        } => {
            let envelope = doc
                .envelopes
                .iter()
                .find(|envelope| &envelope.id == envelope_id);
            let entry = envelope.and_then(|envelope| {
                let found = envelope.moves.iter().find(|entry| &entry.id == move_id);
                found.map(|entry| (envelope, entry))
            });
            let Some((envelope, entry)) = entry else {
                return Some(violation(
                    EFFECT_MISSING,
                    format!("envelope move {envelope_id}/{move_id} is gone"),
                ));
            };
            entry.delta == part.amount
                && entry.income_ref.as_deref() == Some(income_id)
                && envelope_agrees(envelope, topup, part)
        }
    };
    (!agrees).then(|| {
        violation(
            EFFECT_MISMATCH,
            format!("the effect of a part of {} disagrees with it", part.amount),
        )
    })
}

fn transfer_agrees(
    found: &Transaction,
    income_id: &str,
    topup: &TopupTx,
    part: &DistributionPart,
) -> bool {
    let Transaction::Transfer(transfer) = found else {
        return false;
    };
    let target_agrees = match &part.target {
        DistTarget::Store { id } => transfer.to == AccountRef::Store(id.clone()),
        _ => false,
    };
    transfer.amount == part.amount
        && transfer.asset.key() == topup.asset.key()
        && transfer.from == AccountRef::Store(topup.store_id.clone())
        && transfer.meta.income_ref.as_deref() == Some(income_id)
        && target_agrees
}

fn envelope_agrees(envelope: &Envelope, topup: &TopupTx, part: &DistributionPart) -> bool {
    let target_agrees = match &part.target {
        DistTarget::Envelope { id } => &envelope.id == id,
        DistTarget::Goal { goal_id, store_id } => {
            envelope.goal_id.as_deref() == Some(goal_id) && &envelope.store_id == store_id
        }
        _ => false,
    };
    envelope.asset.key() == topup.asset.key()
        && envelope.store_id == topup.store_id
        && target_agrees
}

/// The records that point back to `income_id`, as the effects that would name them.
fn referencing_records(doc: &VaultDoc, income_id: &str) -> Vec<EffectRef> {
    let transfers = doc
        .transactions
        .iter()
        .filter(|tx| tx.meta().income_ref.as_deref() == Some(income_id))
        .map(|tx| EffectRef::Transfer {
            tx_id: tx.meta().id.clone(),
        });
    let moves = doc.envelopes.iter().flat_map(|envelope| {
        envelope
            .moves
            .iter()
            .filter(|entry| entry.income_ref.as_deref() == Some(income_id))
            .map(|entry| EffectRef::EnvelopeMove {
                envelope_id: envelope.id.clone(),
                move_id: entry.id.clone(),
            })
    });
    transfers.chain(moves).collect()
}

/// V3: each referencing record is named by exactly one part.
fn check_back_references(referenced: &[EffectRef], parts: &[DistributionPart]) -> Vec<Violation> {
    let mut named: HashMap<String, usize> = HashMap::new();
    for effect in parts.iter().filter_map(|part| part.effect.as_ref()) {
        *named.entry(effect_key(effect)).or_default() += 1;
    }
    let mut found = Vec::new();
    for effect in referenced {
        match named.get(&effect_key(effect)).copied().unwrap_or(0) {
            1 => {}
            0 => found.push(violation(
                EFFECT_ORPHAN,
                format!(
                    "{} points to the income but no part names it",
                    effect_key(effect)
                ),
            )),
            count => found.push(violation(
                EFFECT_SHARED,
                format!("{} is named by {count} parts", effect_key(effect)),
            )),
        }
    }
    for (key, count) in named {
        let known = referenced.iter().any(|effect| effect_key(effect) == key);
        if count > 1 && !known {
            found.push(violation(
                EFFECT_SHARED,
                format!("{key} is named by {count} parts"),
            ));
        }
    }
    found
}

fn effect_key(effect: &EffectRef) -> String {
    match effect {
        EffectRef::Transfer { tx_id } => format!("transfer {tx_id}"),
        EffectRef::EnvelopeMove {
            envelope_id,
            move_id,
        } => format!("envelope move {envelope_id}/{move_id}"),
    }
}

/// Targets of a distribution on a record of `kind` (spec S6, S14): only an income distributes;
/// store, envelope and goal targets must exist, be own and live; `none` only on the remainder; a
/// goal row's `store_id` is a store of the goal's own scope.
pub fn validate_distribution_targets(
    doc: &VaultDoc,
    kind: TxKind,
    parts: &[DistributionPart],
) -> Result<()> {
    if kind != TxKind::Topup {
        return Err(bad_target("only an income can be distributed"));
    }
    for part in parts {
        let remainder = matches!(part.kind, DistKind::Remainder);
        match &part.target {
            DistTarget::None if remainder => {}
            DistTarget::None => return Err(bad_target("only the remainder may stay on the card")),
            _ if remainder => return Err(bad_target("the remainder stays on the card")),
            DistTarget::Store { id } => {
                live_store(doc, id)?;
            }
            DistTarget::Envelope { id } => {
                let envelope = doc.envelopes.iter().find(|envelope| &envelope.id == id);
                let envelope = envelope.ok_or_else(|| Error::Missing(format!("envelope {id}")))?;
                live_store(doc, &envelope.store_id)?;
            }
            DistTarget::Goal { goal_id, store_id } => check_goal_target(doc, goal_id, store_id)?,
        }
    }
    Ok(())
}

fn check_goal_target(doc: &VaultDoc, goal_id: &str, store_id: &str) -> Result<()> {
    let goal = doc
        .goals
        .iter()
        .find(|goal| goal.id == goal_id)
        .ok_or_else(|| Error::Missing(format!("goal {goal_id}")))?;
    if !goal.active {
        return Err(bad_target("the goal is not active"));
    }
    live_store(doc, store_id)?;
    let own =
        goal.scope.store_ids.is_empty() || goal.scope.store_ids.iter().any(|id| id == store_id);
    if own {
        Ok(())
    } else {
        Err(bad_target("the store is not the goal's own store"))
    }
}

fn bad_target(message: &str) -> Error {
    Error::InvalidInput(format!("{BAD_TARGET}: {message}"))
}
