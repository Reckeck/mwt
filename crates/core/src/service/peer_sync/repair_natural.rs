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

use super::UnlockedVault;
use super::repair::RepairReport;
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

impl UnlockedVault {
    /// One record per natural key (debt terms, readings, valuations, goal
    /// envelopes). The ids of the records folded away go to the report.
    pub(super) fn repair_natural_keys(&mut self, report: &mut RepairReport) {
        let stamps = &self.doc.sync.stamps;
        let mut merged = Vec::new();
        merged.extend(fold_duplicates(
            stamps,
            &mut self.doc.debt_terms,
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
            &mut self.doc.meter_readings,
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
            &mut self.doc.valuations,
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
            &mut self.doc.envelopes,
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
}

#[cfg(test)]
mod tests {
    use super::super::convergence_catalog::add_catalog_stores;
    use super::super::convergence_tests::Ledger;
    use super::super::tests::{settle, two_devices};
    use super::*;
    use crate::model::contact::DebtTerms;
    use crate::model::store::{StoreKind, StoreScope};
    use crate::service::contacts::DebtTermsDraft;
    use crate::service::goals::GoalDraft;
    use crate::service::test_support::{amount, income, store};
    use crate::service::valuation::ValuationInput;

    fn pair() -> (tempfile::TempDir, Vec<UnlockedVault>, Ledger) {
        let (dir, left, right) = two_devices();
        let mut devices = vec![left, right];
        let mut ledger = Ledger::default();
        add_catalog_stores(&mut devices, &mut ledger);
        (dir, devices, ledger)
    }

    fn meet(devices: &mut [UnlockedVault]) {
        let (left, right) = devices.split_at_mut(1);
        settle(&mut left[0], &mut right[0]);
    }

    fn terms(contact: &str, due: &str, note: &str) -> DebtTermsDraft {
        DebtTermsDraft {
            contact_id: contact.into(),
            asset: "USD".into(),
            due_at: Some(due.into()),
            note: note.into(),
        }
    }

    fn pause() {
        std::thread::sleep(std::time::Duration::from_millis(3));
    }

    #[test]
    fn two_concurrent_debt_terms_of_one_contact_make_one_record() {
        let (_dir, mut devices, ledger) = pair();
        let contact = ledger.anchors[0].clone();
        devices[0]
            .set_debt_terms(terms(&contact, "2026-05-01", "left"))
            .unwrap();
        pause();
        devices[1]
            .set_debt_terms(terms(&contact, "2026-06-01", "right"))
            .unwrap();
        meet(&mut devices);
        for device in &devices {
            assert_eq!(device.doc.debt_terms.len(), 1);
            assert_eq!(
                device.doc.debt_terms[0].note, "right",
                "the later edit wins"
            );
            assert_eq!(
                device.doc.debt_terms[0].id,
                sync_ids::debt_terms_id(&contact, "USD")
            );
        }
    }

    #[test]
    fn an_old_random_debt_record_and_a_new_one_merge_into_the_deterministic_one() {
        let (_dir, mut devices, ledger) = pair();
        let contact = ledger.anchors[0].clone();
        devices[0]
            .set_debt_terms(terms(&contact, "2026-05-01", "new build"))
            .unwrap();
        pause();
        // The older build: a random id, edited later than the new record.
        devices[1].doc.debt_terms.push(DebtTerms {
            id: "00000000-old".into(),
            contact_id: contact.clone(),
            asset: "USD".into(),
            due_at: Some("2026-07-01".into()),
            note: "old build".into(),
            created_at: "2026-01-01T00:00:00Z".into(),
        });
        devices[1].persist().unwrap();
        meet(&mut devices);
        for device in &devices {
            assert_eq!(
                device.doc.debt_terms.len(),
                1,
                "one record per contact and asset"
            );
            let kept = &device.doc.debt_terms[0];
            assert_eq!(kept.id, sync_ids::debt_terms_id(&contact, "USD"));
            assert_eq!(
                kept.note, "old build",
                "the later field moved into the winner"
            );
            assert_eq!(kept.due_at.as_deref(), Some("2026-07-01"));
        }
    }

    #[test]
    fn folding_duplicates_is_recorded_in_the_batch_and_undone_with_it() {
        let (_dir, mut devices, ledger) = pair();
        let contact = ledger.anchors[0].clone();
        devices[0]
            .set_debt_terms(terms(&contact, "2026-05-01", "a"))
            .unwrap();
        devices[1].doc.debt_terms.push(DebtTerms {
            id: "00000000-old".into(),
            contact_id: contact,
            asset: "USD".into(),
            due_at: None,
            note: "b".into(),
            created_at: "2026-01-01T00:00:00Z".into(),
        });
        devices[1].persist().unwrap();
        let (left, right) = devices.split_at_mut(1);
        let (_, applied) = super::super::tests::exchange(&mut left[0], &mut right[0]);
        let batch = applied.expect("right applied a batch").id;
        let right = &mut right[0];
        assert_eq!(right.doc.debt_terms.len(), 1);
        assert!(right.sync_undo(&batch).unwrap().restored > 0);
        // Back to the state before the batch: the device's own record only.
        assert_eq!(right.doc.debt_terms.len(), 1);
        assert_eq!(right.doc.debt_terms[0].id, "00000000-old");
    }

    #[test]
    fn a_reading_moved_to_another_day_takes_the_id_of_that_day() {
        let (_dir, mut devices, ledger) = pair();
        let vault = &mut devices[0];
        let first = vault
            .add_meter_reading(&ledger.utility_id, amount("100"), "2026-03", Some(3), None)
            .unwrap();
        assert_eq!(
            first.id,
            sync_ids::reading_id(&ledger.utility_id, None, &("2026-03".into(), 3))
        );
        let moved = vault
            .update_meter_reading(&first.id, amount("100"), "2026-03", Some(5))
            .unwrap();
        assert_ne!(moved.id, first.id);
        assert_eq!(
            moved.id,
            sync_ids::reading_id(&ledger.utility_id, None, &("2026-03".into(), 5))
        );
        assert_eq!(vault.doc.meter_readings.len(), 1);
        assert_eq!(vault.doc.meter_readings[0].id, moved.id);
    }

    #[test]
    fn two_readings_of_one_meter_and_day_make_one_record() {
        let (_dir, mut devices, ledger) = pair();
        devices[0]
            .add_meter_reading(&ledger.utility_id, amount("100"), "2026-03", Some(2), None)
            .unwrap();
        pause();
        devices[1]
            .add_meter_reading(&ledger.utility_id, amount("120"), "2026-03", Some(2), None)
            .unwrap();
        meet(&mut devices);
        for device in &devices {
            assert_eq!(device.doc.meter_readings.len(), 1);
            assert_eq!(device.doc.meter_readings[0].value, amount("120"));
        }
    }

    #[test]
    fn two_valuations_of_one_day_make_one_record() {
        let (_dir, mut devices, ledger) = pair();
        let input = |price: &str| ValuationInput {
            store_id: ledger.property_id.clone(),
            asset_key: ledger.thing_key.clone(),
            date: "2026-03-01".into(),
            unit_price: amount(price),
            currency: "usd".into(),
        };
        devices[0].add_valuation(input("1000")).unwrap();
        pause();
        devices[1].add_valuation(input("1100")).unwrap();
        meet(&mut devices);
        for device in &devices {
            assert_eq!(device.doc.valuations.len(), 1);
            assert_eq!(device.doc.valuations[0].unit_price, amount("1100"));
        }
    }

    #[test]
    fn two_concurrent_goal_top_ups_make_one_envelope_with_both() {
        let (_dir, mut left, mut right) = two_devices();
        let wallet = store(&mut left, "Cash", StoreKind::Cash, None);
        left.add_transaction(income(&wallet, "1000", "pay"))
            .unwrap();
        let goal = left
            .create_goal(GoalDraft {
                name: "trip".into(),
                active: true,
                target_amount: amount("1000"),
                currency: "usd".into(),
                starts_at: "2026-01-01T00:00:00Z".into(),
                deadline: Some("2026-12-31T00:00:00Z".into()),
                priority: 3,
                monthly_commitment: None,
                scope: StoreScope {
                    store_ids: vec![wallet.clone()],
                    include_children: true,
                },
                scenario_id: None,
                expected_return: None,
            })
            .unwrap()
            .id;
        settle(&mut left, &mut right);
        left.fund_goal(&goal, &wallet, amount("100")).unwrap();
        right.fund_goal(&goal, &wallet, amount("50")).unwrap();
        settle(&mut left, &mut right);
        for device in [&left, &right] {
            assert_eq!(
                device.doc.envelopes.len(),
                1,
                "one envelope per goal and wallet"
            );
            assert_eq!(device.doc.envelopes[0].allocated, amount("150"));
            assert_eq!(device.doc.envelopes[0].moves_sum(), amount("150"));
        }
    }
}
