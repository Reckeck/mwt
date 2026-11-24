//! Pure planning of an income split (spec S6, S7, S11): resolve the asked rows against the payout and
//! diff the result against the parts already stored, so the usecase only touches what changed.

use mwt_base::{Amount, Error, Result};
use mwt_model::{DistKind, DistributionPart, DistributionRow, EffectRef, PartStatus};
use rust_decimal::Decimal;

/// The rows resolved against a payout.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedRows {
    /// One part per row, in the rows' order, without effects; the remainder is never negative.
    pub parts: Vec<DistributionPart>,
    /// By how much the sum and percent rows exceed the payout (spec S7); `None` when they fit.
    pub over: Option<Amount>,
}

/// The change a new list of parts needs over the stored one.
#[derive(Debug, Clone, PartialEq)]
pub struct DistributionPlan {
    /// Parts whose effect has to be created (their `effect` is `None` in `parts`).
    pub create: Vec<DistributionPart>,
    /// Effects of dropped or changed parts, to be removed.
    pub remove: Vec<EffectRef>,
    /// The list to store: kept parts carry their old effect.
    pub parts: Vec<DistributionPart>,
}

/// Resolves `rows` against `payout`: `sum` and `percent` rows are rounded to `places` minor-unit
/// digits, the single `remainder` row takes what is left (spec S6). Over-allocation is reported in
/// `over` and leaves the remainder at zero (spec S7).
pub fn resolve_rows(payout: Amount, rows: &[DistributionRow], places: u32) -> Result<ResolvedRows> {
    let remainders = rows
        .iter()
        .filter(|row| matches!(row.kind, DistKind::Remainder))
        .count();
    if remainders != 1 {
        return Err(Error::InvalidInput(format!(
            "a distribution needs exactly one remainder row, got {remainders}"
        )));
    }
    let hundred = Amount::new(Decimal::ONE_HUNDRED);
    let amounts: Vec<Option<Amount>> = rows
        .iter()
        .map(|row| match &row.kind {
            DistKind::Sum { amount } => Some(*amount),
            DistKind::Percent { pct } => Some((payout * *pct / hundred).round(places)),
            DistKind::Remainder => None,
        })
        .collect();
    let asked = amounts
        .iter()
        .flatten()
        .fold(Amount::ZERO, |sum, amount| sum + *amount);
    let left = payout - asked;
    let over = left.is_negative().then(|| -left);
    let remainder = left.max(Amount::ZERO);
    let parts = rows
        .iter()
        .zip(amounts)
        .map(|(row, amount)| DistributionPart {
            kind: row.kind.clone(),
            target: row.target.clone(),
            amount: amount.unwrap_or(remainder),
            status: PartStatus::Done,
            effect: None,
        })
        .collect();
    Ok(ResolvedRows { parts, over })
}

/// Diffs the stored `old_parts` against the resolved `new_parts` (effects `None`): a part with the
/// same target and amount keeps its effect, everything else is removed and created anew (spec S11).
/// Undo is a plan against an empty list.
pub fn plan_distribution(
    old_parts: &[DistributionPart],
    new_parts: &[DistributionPart],
) -> DistributionPlan {
    let mut reusable: Vec<Option<&DistributionPart>> = old_parts
        .iter()
        .map(|part| part.effect.is_some().then_some(part))
        .collect();
    let mut create = Vec::new();
    let mut parts = Vec::with_capacity(new_parts.len());
    for new in new_parts {
        let mut part = new.clone();
        part.effect = None;
        if matches!(part.kind, DistKind::Remainder) {
            parts.push(part);
            continue;
        }
        let kept = reusable.iter_mut().find(|slot| {
            slot.is_some_and(|old| old.target == part.target && old.amount == part.amount)
        });
        match kept.and_then(Option::take) {
            Some(old) => part.effect = old.effect.clone(),
            None => create.push(part.clone()),
        }
        parts.push(part);
    }
    let remove = reusable
        .into_iter()
        .flatten()
        .filter_map(|old| old.effect.clone())
        .collect();
    DistributionPlan {
        create,
        remove,
        parts,
    }
}

#[cfg(test)]
#[path = "distribution_tests.rs"]
mod tests;
