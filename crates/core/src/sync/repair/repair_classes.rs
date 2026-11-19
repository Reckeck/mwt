//! Stage M, rules of `repair_invariants` for asset classes.
//!
//! - Links to an asset class that is gone are dropped (R5).
//! - Shares that do not add up to 100 are scaled to 100 (decision of
//!   2026-11-04): proportional, rounded to the shares' own precision, the
//!   rounding remainder to the largest share (equal: the smaller class id).
//!   All-zero shares are left alone.

use std::collections::HashSet;

use super::repair_report::RepairReport;
use crate::Amount;
use crate::model::planning::AssetClass;
use crate::model::vault::VaultDoc;

/// Links to a class that is gone point at nothing. `known` are the ids of the
/// classes in force (the saved ones or the preset).
pub(crate) fn repair_class_links(
    doc: &mut VaultDoc,
    known: &HashSet<String>,
    report: &mut RepairReport,
) {
    let links = doc.asset_class_of.len();
    doc.asset_class_of
        .retain(|link| known.contains(&link.class_id));
    let mut dropped = links - doc.asset_class_of.len();
    for store in &mut doc.stores {
        if store
            .asset_class
            .as_ref()
            .is_some_and(|class| !known.contains(class))
        {
            store.asset_class = None;
            dropped += 1;
        }
    }
    report.dropped_class_links = dropped;
}

/// Shares that do not add up to 100 are scaled to 100.
pub(crate) fn repair_shares(doc: &mut VaultDoc, report: &mut RepairReport) {
    let Some(scaled) = scaled_shares(&doc.asset_classes) else {
        return;
    };
    for (class, share) in doc.asset_classes.iter_mut().zip(scaled) {
        class.target_pct = share;
    }
    report.normalized_shares = true;
}

/// The scaled shares, in the order of `classes`; `None` when they already add
/// up to 100 or are all zero.
pub fn scaled_shares(classes: &[AssetClass]) -> Option<Vec<Amount>> {
    let hundred = Amount::from(100);
    let total = classes
        .iter()
        .fold(Amount::ZERO, |sum, class| sum + class.target_pct);
    if total == hundred || classes.iter().all(|class| class.target_pct.is_zero()) {
        return None;
    }
    let precision = classes
        .iter()
        .map(|class| class.target_pct.into_inner().scale())
        .max()
        .unwrap_or(0);
    let mut scaled: Vec<Amount> = classes
        .iter()
        .map(|class| (class.target_pct * hundred / total).round(precision))
        .collect();
    let remainder = hundred - scaled.iter().fold(Amount::ZERO, |sum, share| sum + *share);
    let largest = (0..classes.len()).max_by(|left, right| {
        scaled[*left]
            .cmp(&scaled[*right])
            .then_with(|| classes[*right].id.cmp(&classes[*left].id))
    })?;
    scaled[largest] += remainder;
    Some(scaled)
}
