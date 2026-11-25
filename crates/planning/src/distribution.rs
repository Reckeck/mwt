//! Pure planning of an income split (spec S6, S7, S11): resolve the asked rows against the payout and
//! diff the result against the parts already stored, so the usecase only touches what changed.

use mwt_base::{Amount, Error, Result};
use mwt_model::{
    DistKind, DistributionPart, DistributionRow, DistributionRule, EffectRef, OnOtherAmount,
    PartStatus,
};
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

/// One rule row with the amount it resolved to.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedRow {
    pub row: DistributionRow,
    pub amount: Amount,
}

/// A rule resolved against an actual payout (spec D8).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ResolvedDistribution {
    /// Every rule row in order (the remainder row included); empty on `over` and `ask`.
    pub rows: Vec<ResolvedRow>,
    /// What the remainder row took.
    pub remainder: Amount,
    /// By how much the fixed and percent rows exceed the payout: the run does not distribute (Q11).
    pub over: Option<Amount>,
    /// The rule asks for a confirmation instead of distributing (D9).
    pub ask: bool,
    /// Rows cut by `keep_on_card_min` as `(row_index, by)` (D10).
    pub cut: Vec<(usize, Amount)>,
}

/// Resolves `rule` for an `actual` payout against the `expected` one (spec D8-D10).
/// `card_balance_before_rows` is the card balance once the payout is booked and no row yet; the rows
/// that take money off the card (every non-remainder row, envelope and goal rows included, as the cut
/// of D10 treats them alike) are subtracted here, so the floor is judged on the balance after them.
pub fn resolve_distribution(
    rule: &DistributionRule,
    actual: Amount,
    expected: Amount,
    card_balance_before_rows: Amount,
    places: u32,
) -> Result<ResolvedDistribution> {
    let differs = actual != expected;
    if differs && rule.on_other_amount == OnOtherAmount::Ask {
        return Ok(ResolvedDistribution {
            ask: true,
            ..ResolvedDistribution::default()
        });
    }
    let scale = differs && rule.on_other_amount == OnOtherAmount::ShrinkAll && !expected.is_zero();
    let asked: Vec<DistributionRow> = rule
        .rows
        .iter()
        .map(|row| match &row.kind {
            DistKind::Sum { amount } if scale => DistributionRow {
                kind: DistKind::Sum {
                    amount: (*amount * actual / expected).round(places),
                },
                target: row.target.clone(),
            },
            _ => row.clone(),
        })
        .collect();
    let resolved = resolve_rows(actual, &asked, places)?;
    if resolved.over.is_some() {
        return Ok(ResolvedDistribution {
            over: resolved.over,
            ..ResolvedDistribution::default()
        });
    }
    let mut rows: Vec<ResolvedRow> = asked
        .into_iter()
        .zip(&resolved.parts)
        .map(|(row, part)| ResolvedRow {
            row,
            amount: part.amount,
        })
        .collect();
    let mut remainder = rows
        .iter()
        .find(|resolved_row| matches!(resolved_row.row.kind, DistKind::Remainder))
        .map_or(Amount::ZERO, |resolved_row| resolved_row.amount);
    let leaving = rows
        .iter()
        .filter(|resolved_row| !matches!(resolved_row.row.kind, DistKind::Remainder))
        .fold(Amount::ZERO, |sum, resolved_row| sum + resolved_row.amount);
    let card_balance_after = card_balance_before_rows - leaving;
    let cut = rule
        .keep_on_card_min
        .map(|floor| cut_to_floor(&mut rows, floor - card_balance_after))
        .unwrap_or_default();
    for (_, by) in &cut {
        remainder += *by;
    }
    for resolved_row in &mut rows {
        if matches!(resolved_row.row.kind, DistKind::Remainder) {
            resolved_row.amount = remainder;
        }
    }
    Ok(ResolvedDistribution {
        rows,
        remainder,
        over: None,
        ask: false,
        cut,
    })
}

/// Cuts `missing` from the last non-remainder row upwards (spec D10, Q13).
fn cut_to_floor(rows: &mut [ResolvedRow], mut missing: Amount) -> Vec<(usize, Amount)> {
    let mut cut = Vec::new();
    for (index, resolved_row) in rows.iter_mut().enumerate().rev() {
        if !missing.is_positive() {
            break;
        }
        if matches!(resolved_row.row.kind, DistKind::Remainder)
            || !resolved_row.amount.is_positive()
        {
            continue;
        }
        let by = resolved_row.amount.min(missing);
        resolved_row.amount -= by;
        missing -= by;
        cut.push((index, by));
    }
    cut
}

#[cfg(test)]
#[path = "distribution_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "distribution_rule_tests.rs"]
mod rule_tests;
