//! The invariant pass of stage M: after a merge (and when a vault opens) the
//! document is checked against the rules a merge cannot keep by itself, and a
//! violation is fixed by a deterministic edit.
//!
//! - Acts only on a violation, so a consistent document is not stamped or sent.
//! - Deterministic: the fix depends only on the merged state (ids, stamps,
//!   values), so two devices that merged the same records make the same edit.
//! - Stamped as a local edit, so it travels to the other devices, older builds
//!   included; recorded in the batch that caused it, so undoing the batch
//!   undoes the repair; the user is told in a notice.
//! - Never changes money: it moves or drops references and structure only.
//!
//! Rules, in the order they run: R3 (deleted records that money points to are
//! restored, `repair_restore.rs`), R9 (staged imports that are already booked
//! or discarded, `repair_imports.rs`), duplicates by natural key, R1 (one root Cash
//! wallet, `repair_cash.rs`), R2 (account tree), R4 (envelopes of deleted
//! goals), R5 (links to missing asset classes), the shares of the classes
//! (`repair_natural.rs`), the links and money of shared events (R6, R7,
//! `repair_events.rs`), R8 (emergency fund and inflation sources that name a
//! missing record).

use std::collections::{BTreeMap, HashSet};

use crate::error::Result;
use crate::model::notify::{MAX_NOTICE_LOG, NoticeCategory};
use crate::model::store::{Binding, Custody, ExchangeProfile, StoreKind};
use crate::sync::view;
use crate::sync::{BatchEntry, Stamp};

use super::UnlockedVault;
pub(super) use super::repair_report::RepairReport;

impl UnlockedVault {
    /// Check the rules and fix what breaks them. `batch_id` is the sync batch
    /// whose merge is being settled (the repair is recorded in it for undo);
    /// `None` when a vault opens (the change is only stamped).
    pub(in crate::service) fn repair_invariants(
        &mut self,
        batch_id: Option<&str>,
    ) -> Result<RepairReport> {
        let before = self.doc_value()?;
        let mut report = RepairReport::default();
        self.repair_restored_records(&mut report);
        self.repair_pending_imports(&mut report);
        // Before the natural keys: a folded guest contact may leave two debt
        // terms of one contact and asset behind.
        self.repair_guest_contacts(&mut report);
        self.repair_natural_keys(&mut report);
        self.repair_cash_wallets(&mut report);
        self.repair_account_tree(&mut report);
        self.repair_goal_envelopes(&mut report);
        self.repair_class_links(&mut report);
        self.repair_shares(&mut report);
        self.repair_event_links(&mut report);
        self.repair_event_money(&mut report);
        // R8. Things another device deleted may still be named by a record
        // that arrived (an emergency account, an inflation source). Shared
        // events keep their references: a settlement or item that names a
        // removed participant is user data (the next rules of stage M).
        self.prune_emergency_fund();
        self.prune_inflation_sources();
        self.announce_repair(&report);

        let after = self.doc_value()?;
        if after == before {
            return Ok(report);
        }
        let pre_records = view::records(&before);
        let post_records = view::records(&after);
        let touched: Vec<String> = pre_records
            .keys()
            .chain(post_records.keys())
            .filter(|key| pre_records.get(*key) != post_records.get(*key))
            .cloned()
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        // The states the batch has to restore, before the tracker stamps.
        let mut before_states = BTreeMap::new();
        for key in &touched {
            before_states.insert(key.clone(), self.state_of(&pre_records, key));
        }
        let changed = self.track_changes()?;
        let settled = view::records(&self.doc_value()?);
        report.records = changed.clone();
        if let Some(id) = batch_id
            && let Some(batch) = self
                .doc
                .sync
                .batches
                .iter()
                .position(|batch| batch.id == id)
        {
            let mut entries = Vec::new();
            for key in changed {
                let Some(prior) = before_states.remove(&key) else {
                    continue;
                };
                entries.push(BatchEntry {
                    after: self.state_of(&settled, &key).marks(),
                    before: prior,
                    key,
                });
            }
            let kept = &mut self.doc.sync.batches[batch].entries;
            for entry in entries {
                match kept.iter_mut().find(|known| known.key == entry.key) {
                    Some(known) => known.after = entry.after,
                    None => kept.push(entry),
                }
            }
        }
        Ok(report)
    }

    /// The notice of a repair, added to the (synced) log under a key that the
    /// repair itself decides, so every device that makes it shows it once.
    fn announce_repair(&mut self, report: &RepairReport) {
        let Some(body) = report.notice_body(self.notice_ru) else {
            return;
        };
        let key = report.notice_key();
        if self
            .doc
            .notification_log
            .iter()
            .any(|record| record.key == key)
        {
            return;
        }
        let title = if self.notice_ru {
            "Исправлено после синхронизации"
        } else {
            "Fixed after sync"
        };
        let record = super::super::notifications::notice(
            NoticeCategory::Security,
            key,
            title,
            body,
            Some("/?settings=sync".into()),
            crate::model::vault::now_iso(),
        );
        self.doc.notification_log.push(record);
        let overflow = self
            .doc
            .notification_log
            .len()
            .saturating_sub(MAX_NOTICE_LOG);
        self.doc.notification_log.drain(..overflow);
    }

    // -- R2: the account tree -------------------------------------------------

    /// A parent exists, is a wallet (not an account) of the same kind, and the
    /// parent links make no cycle.
    ///
    /// The choice is by the stamp of `parent_id`: of the members of a cycle the
    /// one whose link is the oldest is cut (equal stamps: the smaller id), so
    /// the more recent decision of the user stands. An account under a missing,
    /// nested or foreign-kind parent is cut itself. A cut account becomes an
    /// archived wallet of its own kind, as when an account is split off.
    fn repair_account_tree(&mut self, report: &mut RepairReport) {
        while let Some(cycle) = self.account_cycle() {
            let victim = cycle
                .into_iter()
                .min_by_key(|id| (self.parent_link_stamp(id), id.clone()))
                .unwrap_or_default();
            self.cut_to_wallet(&victim, report);
        }
        let mut broken: Vec<String> = self
            .doc
            .stores
            .iter()
            .filter(|node| {
                node.parent_id.as_deref().is_some_and(|parent_id| {
                    self.find_store(parent_id)
                        .is_none_or(|parent| parent.parent_id.is_some() || parent.kind != node.kind)
                })
            })
            .map(|node| node.id.clone())
            .collect();
        broken.sort();
        for id in broken {
            self.cut_to_wallet(&id, report);
        }
    }

    /// The members of one cycle of parent links (smallest start id first), if any.
    fn account_cycle(&self) -> Option<Vec<String>> {
        let mut ids: Vec<&str> = self
            .doc
            .stores
            .iter()
            .map(|node| node.id.as_str())
            .collect();
        ids.sort_unstable();
        for start in ids {
            let mut path: Vec<&str> = vec![start];
            let mut seen: HashSet<&str> = HashSet::from([start]);
            let mut next = self
                .find_store(start)
                .and_then(|node| node.parent_id.as_deref());
            while let Some(id) = next {
                if !seen.insert(id) {
                    let from = path.iter().position(|member| *member == id)?;
                    return Some(
                        path[from..]
                            .iter()
                            .map(|member| member.to_string())
                            .collect(),
                    );
                }
                path.push(id);
                next = self
                    .find_store(id)
                    .and_then(|node| node.parent_id.as_deref());
            }
        }
        None
    }

    fn parent_link_stamp(&self, id: &str) -> Stamp {
        self.doc
            .sync
            .stamps
            .get(&format!("stores:{id}"))
            .map(|stamps| stamps.field("parent_id").clone())
            .unwrap_or_default()
    }

    /// The fields of `detach` (a wallet of its own, no account type or
    /// binding), archived, with a name free among the wallets.
    fn cut_to_wallet(&mut self, id: &str, report: &mut RepairReport) {
        let Some(node) = self.find_store(id).cloned() else {
            return;
        };
        let name = self.free_name(id, None, &node.name);
        if let Some(target) = self.doc.stores.iter_mut().find(|store| store.id == id) {
            target.parent_id = None;
            target.sub_type = None;
            target.binding = Binding::Any;
            target.name = name;
            target.archived = true;
            target.custody = (target.kind == StoreKind::Crypto).then_some(Custody::Hot);
            target.exchange = (target.kind == StoreKind::Exchange).then(ExchangeProfile::default);
        }
        report.detached_accounts.push((id.to_string(), node.name));
    }

    // -- R4: envelopes of deleted goals -----------------------------------------

    /// An envelope whose goal is gone is deleted again (the earmark only: the
    /// money never left the account). A top-up on another device resurrects the
    /// envelope after the goal went; this puts the deletion back.
    fn repair_goal_envelopes(&mut self, report: &mut RepairReport) {
        let goals: HashSet<&str> = self.doc.goals.iter().map(|goal| goal.id.as_str()).collect();
        let mut gone: Vec<(String, String)> = self
            .doc
            .envelopes
            .iter()
            .filter(|envelope| {
                envelope
                    .goal_id
                    .as_deref()
                    .is_some_and(|goal| !goals.contains(goal))
            })
            .map(|envelope| (envelope.id.clone(), envelope.name.clone()))
            .collect();
        if gone.is_empty() {
            return;
        }
        gone.sort();
        let ids: HashSet<&str> = gone.iter().map(|(id, _)| id.as_str()).collect();
        self.doc
            .envelopes
            .retain(|envelope| !ids.contains(envelope.id.as_str()));
        report.dropped_envelopes = gone;
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{settle, two_devices};
    use super::*;
    use crate::model::store::StoreScope;
    use crate::model::store::{StoreKind, SubType};
    use crate::service::StoreExtras;
    use crate::service::goals::GoalDraft;
    use crate::service::test_support::{amount, holding_of, income, store, usd};

    fn sub_account(vault: &mut UnlockedVault, name: &str, parent: &str) -> String {
        vault
            .create_store_with(
                name,
                StoreKind::Bank,
                "#000",
                "",
                Some(parent.to_string()),
                StoreExtras {
                    sub_type: Some(SubType::Savings),
                    binding: Some(Binding::Asset(usd())),
                    ..StoreExtras::default()
                },
            )
            .unwrap()
            .id
    }

    fn set_parent(vault: &mut UnlockedVault, id: &str, parent: &str) {
        vault
            .doc
            .stores
            .iter_mut()
            .find(|node| node.id == id)
            .unwrap()
            .parent_id = Some(parent.to_string());
        vault.persist().unwrap();
    }

    fn parent_of(vault: &UnlockedVault, id: &str) -> Option<String> {
        vault.find_store(id).and_then(|node| node.parent_id.clone())
    }

    fn goal(vault: &mut UnlockedVault, wallet: &str) -> String {
        vault
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
                    store_ids: vec![wallet.to_string()],
                    include_children: true,
                },
                scenario_id: None,
                expected_return: None,
            })
            .unwrap()
            .id
    }

    #[test]
    fn a_cycle_from_two_concurrent_moves_is_cut_the_same_on_both_devices() {
        let (_dir, mut left, mut right) = two_devices();
        let first = store(&mut left, "bank 1", StoreKind::Bank, None);
        let second = store(&mut left, "bank 2", StoreKind::Bank, None);
        let one = sub_account(&mut left, "one", &first);
        let two = sub_account(&mut left, "two", &first);
        settle(&mut left, &mut right);
        // `one` under `two` on one device, `two` under `one` on the other:
        // the later edit is the right one's.
        set_parent(&mut left, &one, &two);
        std::thread::sleep(std::time::Duration::from_millis(3));
        set_parent(&mut right, &two, &one);
        settle(&mut left, &mut right);
        for device in [&left, &right] {
            assert_eq!(parent_of(device, &one), None, "the older link is cut");
            assert!(device.find_store(&one).unwrap().archived);
            assert_eq!(parent_of(device, &two).as_deref(), Some(one.as_str()));
            assert!(device.account_cycle().is_none());
        }
        let _ = second;
    }

    #[test]
    fn an_account_under_a_deleted_parent_becomes_a_wallet() {
        let (_dir, mut left, mut right) = two_devices();
        let first = store(&mut left, "bank 1", StoreKind::Bank, None);
        let second = store(&mut left, "bank 2", StoreKind::Bank, None);
        let savings = sub_account(&mut left, "sv1", &first);
        settle(&mut left, &mut right);
        left.move_store(&savings, Some(&second)).unwrap();
        right.delete_store(&second).unwrap();
        settle(&mut left, &mut right);
        for device in [&left, &right] {
            let node = device.find_store(&savings).expect("the account stays");
            assert_eq!(node.parent_id, None);
            assert!(node.archived);
        }
    }

    #[test]
    fn an_account_under_an_account_is_cut() {
        let (_dir, mut vault) = crate::service::test_support::empty_vault();
        let bank = store(&mut vault, "bank", StoreKind::Bank, None);
        let one = sub_account(&mut vault, "one", &bank);
        let two = sub_account(&mut vault, "two", &bank);
        set_parent(&mut vault, &two, &one);
        let report = vault.repair_invariants(None).unwrap();
        assert_eq!(report.detached_accounts.len(), 1);
        assert_eq!(parent_of(&vault, &two), None);
        assert_eq!(parent_of(&vault, &one).as_deref(), Some(bank.as_str()));
    }

    #[test]
    fn an_envelope_topped_up_after_its_goal_was_deleted_is_deleted_again() {
        let (_dir, mut left, mut right) = two_devices();
        let wallet = store(&mut left, "Cash", StoreKind::Cash, None);
        left.add_transaction(income(&wallet, "1000", "pay"))
            .unwrap();
        let goal_id = goal(&mut left, &wallet);
        left.fund_goal(&goal_id, &wallet, amount("100")).unwrap();
        settle(&mut left, &mut right);
        let envelope = left.doc.envelopes[0].id.clone();
        left.delete_goal(&goal_id, chrono::Utc::now().timestamp())
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(3));
        right.fund_envelope(&envelope, amount("50")).unwrap();
        settle(&mut left, &mut right);
        for device in [&left, &right] {
            assert!(device.doc.goals.is_empty());
            assert!(
                device.doc.envelopes.is_empty(),
                "the earmark goes with the goal"
            );
            assert_eq!(holding_of(device, &wallet, &usd()), amount("1000"));
        }
    }

    #[test]
    fn a_second_pass_changes_and_stamps_nothing() {
        let (_dir, mut vault) = crate::service::test_support::empty_vault();
        vault.set_device("aaaaaaaa-one");
        let bank = store(&mut vault, "bank", StoreKind::Bank, None);
        let one = sub_account(&mut vault, "one", &bank);
        let two = sub_account(&mut vault, "two", &bank);
        set_parent(&mut vault, &two, &one);
        assert!(!vault.repair_invariants(None).unwrap().is_empty());
        let stamps = vault.doc.sync.stamps.clone();
        let tombstones = vault.doc.sync.tombstones.clone();
        let log = vault.doc.notification_log.len();
        assert!(vault.repair_invariants(None).unwrap().is_empty());
        assert_eq!(vault.doc.sync.stamps, stamps);
        assert_eq!(vault.doc.sync.tombstones, tombstones);
        assert_eq!(vault.doc.notification_log.len(), log);
    }

    #[test]
    fn undoing_the_batch_undoes_the_repair() {
        let (_dir, mut left, mut right) = two_devices();
        let first = store(&mut left, "bank 1", StoreKind::Bank, None);
        let second = store(&mut left, "bank 2", StoreKind::Bank, None);
        let savings = sub_account(&mut left, "sv1", &first);
        settle(&mut left, &mut right);
        left.move_store(&savings, Some(&second)).unwrap();
        right.delete_store(&second).unwrap();
        // The right device takes the left's move and repairs it.
        let (_, applied) = super::super::tests::exchange(&mut left, &mut right);
        assert!(applied.is_some(), "right took something");
        let batch = right
            .doc
            .sync
            .batches
            .iter()
            .rev()
            .find(|batch| {
                batch
                    .entries
                    .iter()
                    .any(|entry| entry.key == format!("stores:{savings}"))
            })
            .map(|batch| batch.id.clone())
            .expect("a batch names the account");
        assert_eq!(parent_of(&right, &savings), None, "repaired");
        let report = right.sync_undo(&batch).unwrap();
        assert!(report.restored > 0);
        assert_ne!(
            parent_of(&right, &savings),
            None,
            "the repair is undone with the batch"
        );
    }

    #[test]
    fn two_devices_repairing_independently_do_not_make_a_new_violation() {
        let (_dir, mut left, mut right) = two_devices();
        let bank = store(&mut left, "bank", StoreKind::Bank, None);
        let one = sub_account(&mut left, "one", &bank);
        let two = sub_account(&mut left, "two", &bank);
        settle(&mut left, &mut right);
        // Both devices repair their own copy of a broken tree before they meet.
        for device in [&mut left, &mut right] {
            set_parent(device, &two, &one);
            device.repair_invariants(None).unwrap();
        }
        settle(&mut left, &mut right);
        for device in [&left, &right] {
            assert!(device.account_cycle().is_none());
            assert!(device.doc.stores.iter().all(|node| {
                node.parent_id.as_deref().is_none_or(|parent| {
                    device
                        .find_store(parent)
                        .is_some_and(|up| up.parent_id.is_none())
                })
            }));
        }
    }
}
