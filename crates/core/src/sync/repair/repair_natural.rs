//! Stage M, rules of `repair_invariants` for natural keys.
//!
//! - Records with a natural key (debt terms of a contact and asset, a meter's
//!   reading on a day, a valuation of an asset on a day, the envelope of a
//!   goal in a wallet and asset) exist once. Records made by an older build
//!   carry random ids and meet the deterministic ones here (see `sync_ids`).
//!   Winner: the deterministic id, else the earliest creation stamp, else the
//!   smaller id. Fields the loser changed later than the winner move into the
//!   winner; an envelope also takes the loser's journal, so no top-up is lost.
//!
//! Asset classes have their own file, `repair_classes.rs`.

use std::collections::BTreeMap;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use super::repair_report::RepairReport;
use crate::model::vault::VaultDoc;
use crate::service::sync_ids;
use crate::sync::state::RecordStamps;

/// How a collection names its natural key and what it must not carry over.
struct Fold<'rules, T> {
    collection: &'static str,
    id_of: fn(&T) -> &str,
    /// `(natural key, deterministic id)`; `None` for a record outside the rule.
    key: fn(&T) -> Option<(String, String)>,
    /// Fields that are part of the key or are merged by `join`.
    pinned: &'rules [&'rules str],
    join: fn(&mut T, &T),
}

/// Fold the duplicates of `records` into one each; the ids of the removed.
fn fold_duplicates<T>(
    stamps: &BTreeMap<String, RecordStamps>,
    records: &mut Vec<T>,
    rule: &Fold<'_, T>,
) -> Vec<String>
where
    T: Serialize + DeserializeOwned,
{
    let mut groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (index, record) in records.iter().enumerate() {
        if let Some((natural, _)) = (rule.key)(record) {
            groups.entry(natural).or_default().push(index);
        }
    }
    let stamps_of = |id: &str| stamps.get(&format!("{}:{id}", rule.collection));
    let mut removed: Vec<String> = Vec::new();
    for members in groups.into_values().filter(|members| members.len() > 1) {
        let rank = |index: usize| {
            let record = &records[index];
            let id = (rule.id_of)(record);
            let deterministic = (rule.key)(record).is_some_and(|(_, wanted)| wanted == id);
            (
                !deterministic,
                stamps_of(id)
                    .map(|known| known.created.clone())
                    .unwrap_or_default(),
                id.to_string(),
            )
        };
        let Some(winner) = members.iter().copied().min_by_key(|index| rank(*index)) else {
            continue;
        };
        let winner_id = (rule.id_of)(&records[winner]).to_string();
        let mut losers: Vec<usize> = members
            .into_iter()
            .filter(|index| *index != winner)
            .collect();
        losers.sort_by_key(|index| (rule.id_of)(&records[*index]).to_string());
        // What a loser changed later than the winner moves into the winner.
        let winner_stamps = stamps_of(&winner_id).cloned().unwrap_or_default();
        let mut kept = match serde_json::to_value(&records[winner]) {
            Ok(Value::Object(map)) => map,
            _ => continue,
        };
        for loser in &losers {
            let loser_id = (rule.id_of)(&records[*loser]).to_string();
            let (Some(loser_stamps), Ok(Value::Object(other))) =
                (stamps_of(&loser_id), serde_json::to_value(&records[*loser]))
            else {
                continue;
            };
            for (name, value) in &other {
                if name != "id"
                    && !rule.pinned.contains(&name.as_str())
                    && loser_stamps.field(name) > winner_stamps.field(name)
                {
                    kept.insert(name.clone(), value.clone());
                }
            }
        }
        if let Ok(mut carried) = serde_json::from_value::<T>(Value::Object(kept)) {
            for loser in &losers {
                (rule.join)(&mut carried, &records[*loser]);
            }
            records[winner] = carried;
        }
        for loser in &losers {
            removed.push((rule.id_of)(&records[*loser]).to_string());
        }
        let mut doomed = losers;
        doomed.sort_unstable_by(|left, right| right.cmp(left));
        for index in doomed {
            records.remove(index);
        }
    }
    removed
}

fn zone_key(zone: &Option<String>) -> String {
    zone.clone().unwrap_or_default()
}

/// One record per natural key (debt terms, readings, valuations, goal
/// envelopes). The ids of the records folded away go to the report.
pub(crate) fn repair_natural_keys(doc: &mut VaultDoc, report: &mut RepairReport) {
    let stamps = &doc.sync.stamps;
    let mut merged = Vec::new();
    merged.extend(fold_duplicates(
        stamps,
        &mut doc.debt_terms,
        &Fold {
            collection: "debt_terms",
            id_of: |terms| &terms.id,
            key: |terms| {
                Some((
                    format!("{}\u{1f}{}", terms.contact_id, terms.asset),
                    sync_ids::debt_terms_id(&terms.contact_id, &terms.asset),
                ))
            },
            pinned: &["contact_id", "asset"],
            join: |_, _| {},
        },
    ));
    merged.extend(fold_duplicates(
        stamps,
        &mut doc.meter_readings,
        &Fold {
            collection: "meter_readings",
            id_of: |reading| &reading.id,
            key: |reading| {
                let when = reading.sort_key();
                Some((
                    format!(
                        "{}\u{1f}{}\u{1f}{}\u{1f}{}",
                        reading.item_id,
                        zone_key(&reading.zone),
                        when.0,
                        when.1
                    ),
                    sync_ids::reading_id(&reading.item_id, reading.zone.as_deref(), &when),
                ))
            },
            pinned: &["item_id", "zone", "period", "day"],
            join: |_, _| {},
        },
    ));
    merged.extend(fold_duplicates(
        stamps,
        &mut doc.valuations,
        &Fold {
            collection: "valuations",
            id_of: |valuation| &valuation.id,
            key: |valuation| {
                Some((
                    format!("{}\u{1f}{}", valuation.asset_key, valuation.date),
                    sync_ids::valuation_id(&valuation.asset_key, &valuation.date),
                ))
            },
            pinned: &["asset_key", "date"],
            join: |_, _| {},
        },
    ));
    merged.extend(fold_duplicates(
        stamps,
        &mut doc.envelopes,
        &Fold {
            collection: "envelopes",
            id_of: |envelope| &envelope.id,
            key: |envelope| {
                let goal = envelope.goal_id.as_deref()?;
                let asset = envelope.asset.key();
                Some((
                    format!("{goal}\u{1f}{}\u{1f}{asset}", envelope.store_id),
                    sync_ids::goal_envelope_id(goal, &envelope.store_id, &asset),
                ))
            },
            pinned: &["store_id", "asset", "goal_id", "moves", "allocated"],
            join: |winner, loser| {
                // The journals add up: every entry of the folded envelope
                // that the kept one lacks moves over.
                let mut other = loser.clone();
                winner.settle_moves();
                other.settle_moves();
                for entry in other.moves {
                    if !winner.moves.iter().any(|known| known.id == entry.id) {
                        winner.moves.push(entry);
                    }
                }
                winner.allocated = winner.moves_sum();
            },
        },
    ));
    report.merged_duplicates = merged;
}
