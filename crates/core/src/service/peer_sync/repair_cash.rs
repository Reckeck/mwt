//! Stage M, rule R1 of `repair_invariants`: one root Cash wallet.
//!
//! Two vaults made apart each own a Cash wallet, and both are locked against
//! delete and archive, so after the first sync the user could not fix it.
//! Winner: the id `cash` (what `ensure_cash_wallet` makes now), else the
//! earliest creation stamp, else the smaller id, so every device picks the
//! same one from the same merged state. A loser's sub-accounts move under the
//! winner (a clashing name gets a free one), every reference to it (operations,
//! links, goals, exchange targets) is pointed at the winner and the loser is
//! dropped, which the change tracker turns into a tombstone. The user-facing
//! `merge_into` still refuses a root Cash; this is a path of the repair only.

use super::UnlockedVault;
use super::repair::RepairReport;
use crate::model::store::StoreKind;
use crate::service::CASH_WALLET_ID;
use crate::service::store_change::replace_id;

impl UnlockedVault {
    /// Merge the root Cash wallets into one; the ids of the merged-away go to the report.
    pub(super) fn repair_cash_wallets(&mut self, report: &mut RepairReport) {
        let mut roots: Vec<String> = self
            .doc
            .stores
            .iter()
            .filter(|node| node.kind == StoreKind::Cash && node.parent_id.is_none())
            .map(|node| node.id.clone())
            .collect();
        if roots.len() < 2 {
            return;
        }
        let created = |id: &str| {
            self.doc
                .sync
                .stamps
                .get(&format!("stores:{id}"))
                .map(|known| known.created.clone())
                .unwrap_or_default()
        };
        roots.sort_by_key(|id| (id != CASH_WALLET_ID, created(id), id.clone()));
        let winner = roots.remove(0);
        for loser in roots {
            if self.merge_cash_into(&loser, &winner).is_some() {
                report.merged_cash.push(loser);
            }
        }
    }

    fn merge_cash_into(&mut self, loser: &str, winner: &str) -> Option<()> {
        let mut children: Vec<String> = self
            .children_of(loser)
            .into_iter()
            .map(|child| child.id.clone())
            .collect();
        children.sort();
        for child in children {
            let name = self.find_store(&child)?.name.clone();
            let free = self.free_name(&child, Some(winner), &name);
            let node = self.doc.stores.iter_mut().find(|node| node.id == child)?;
            node.parent_id = Some(winner.to_string());
            node.name = free;
        }
        self.doc.stores.retain(|node| node.id != loser);
        self.doc
            .exchanges
            .bybit
            .retain(|keys| keys.store_id != loser);
        // Sync bookkeeping keeps its own keys; only the document is repointed.
        let sync = std::mem::take(&mut self.doc.sync);
        let mut value = serde_json::to_value(&self.doc).ok()?;
        replace_id(&mut value, loser, winner);
        let mut doc = serde_json::from_value(value).ok()?;
        std::mem::swap(&mut self.doc, &mut doc);
        self.doc.sync = sync;
        Some(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{exchange, settle};
    use super::*;
    use crate::model::store::{StoreKind, SubType};
    use crate::service::StoreExtras;
    use crate::service::store_change::{StoreChange, StoreChangeInput};
    use crate::service::test_support::{empty_vault, income, usd};
    use crate::sync::track::Tracker;

    /// A vault as an older build made it: the Cash wallet has a random id.
    fn old_build(vault: &mut UnlockedVault) -> String {
        let old = uuid::Uuid::new_v4().to_string();
        // Nothing refers to the wallet yet, so renaming the record is enough.
        for node in &mut vault.doc.stores {
            if node.id == CASH_WALLET_ID {
                node.id = old.clone();
            }
        }
        let stamps = vault
            .doc
            .sync
            .stamps
            .remove(&format!("stores:{CASH_WALLET_ID}"));
        if let Some(stamps) = stamps {
            vault
                .doc
                .sync
                .stamps
                .insert(format!("stores:{old}"), stamps);
        }
        vault.tracker = Some(Tracker::capture(&vault.doc_value().unwrap()));
        vault.persist().unwrap();
        old
    }

    fn device(name: &str) -> (tempfile::TempDir, UnlockedVault) {
        let (dir, mut vault) = empty_vault();
        vault.set_device(name);
        vault.persist().unwrap();
        (dir, vault)
    }

    fn roots(vault: &UnlockedVault) -> Vec<String> {
        vault
            .doc
            .stores
            .iter()
            .filter(|node| node.kind == StoreKind::Cash && node.parent_id.is_none())
            .map(|node| node.id.clone())
            .collect()
    }

    fn pocket(vault: &mut UnlockedVault, parent: &str, name: &str) -> String {
        vault
            .create_store_with(
                name,
                StoreKind::Cash,
                "#000",
                "",
                Some(parent.to_string()),
                StoreExtras {
                    sub_type: Some(SubType::Pocket),
                    binding: Some(crate::model::store::Binding::Asset(usd())),
                    ..StoreExtras::default()
                },
            )
            .unwrap()
            .id
    }

    #[test]
    fn a_new_vault_gets_a_cash_wallet_with_the_fixed_id() {
        let (_dir, vault) = empty_vault();
        assert_eq!(roots(&vault), vec![CASH_WALLET_ID.to_string()]);
    }

    #[test]
    fn two_vaults_made_apart_end_with_one_cash_wallet_holding_both_operations() {
        let (_one, mut left) = device("aaaaaaaa-left");
        let (_two, mut right) = device("bbbbbbbb-right");
        let left_cash = old_build(&mut left);
        let right_cash = old_build(&mut right);
        left.add_transaction(income(&left_cash, "10", "left"))
            .unwrap();
        right
            .add_transaction(income(&right_cash, "20", "right"))
            .unwrap();
        settle(&mut left, &mut right);
        for device in [&left, &right] {
            let cash = roots(device);
            assert_eq!(cash.len(), 1, "one root Cash wallet");
            assert_eq!(device.doc.transactions.len(), 2);
            for tx in &device.doc.transactions {
                assert!(tx.touches(&cash[0]), "both operations are on the survivor");
            }
        }
        assert_eq!(roots(&left), roots(&right));
        let before = left.sync_manifest(None).unwrap();
        assert_eq!(exchange(&mut left, &mut right), (None, None));
        assert_eq!(left.sync_manifest(None).unwrap(), before);
    }

    #[test]
    fn the_wallet_named_cash_beats_an_old_random_one_on_both_devices() {
        let (_one, mut left) = device("aaaaaaaa-left");
        let (_two, mut right) = device("bbbbbbbb-right");
        let old = old_build(&mut left);
        right
            .add_transaction(income(CASH_WALLET_ID, "5", "right"))
            .unwrap();
        left.add_transaction(income(&old, "7", "left")).unwrap();
        settle(&mut left, &mut right);
        assert_eq!(roots(&left), vec![CASH_WALLET_ID.to_string()]);
        assert_eq!(roots(&right), vec![CASH_WALLET_ID.to_string()]);
        assert_eq!(left.doc.transactions.len(), 2);
    }

    #[test]
    fn the_sub_accounts_of_the_loser_move_under_the_winner_with_free_names() {
        let (_one, mut left) = device("aaaaaaaa-left");
        let (_two, mut right) = device("bbbbbbbb-right");
        let old = old_build(&mut left);
        let mine = pocket(&mut left, &old, "Wallet");
        let other = pocket(&mut right, CASH_WALLET_ID, "Wallet");
        pocket(&mut left, &old, "Jar");
        settle(&mut left, &mut right);
        for device in [&left, &right] {
            assert_eq!(roots(device), vec![CASH_WALLET_ID.to_string()]);
            let mut names: Vec<String> = device
                .doc
                .stores
                .iter()
                .filter(|node| node.parent_id.as_deref() == Some(CASH_WALLET_ID))
                .map(|node| node.name.clone())
                .collect();
            names.sort();
            assert_eq!(names, vec!["Jar", "Wallet", "Wallet (2)"]);
        }
        assert_eq!(left.find_store(&other).unwrap().name, "Wallet");
        assert_eq!(left.find_store(&mine).unwrap().name, "Wallet (2)");
    }

    #[test]
    fn undoing_the_batch_brings_the_two_wallets_back() {
        let (_one, mut left) = device("aaaaaaaa-left");
        let (_two, mut right) = device("bbbbbbbb-right");
        let left_cash = old_build(&mut left);
        let right_cash = old_build(&mut right);
        left.add_transaction(income(&left_cash, "10", "left"))
            .unwrap();
        right
            .add_transaction(income(&right_cash, "20", "right"))
            .unwrap();
        let (_, applied) = exchange(&mut left, &mut right);
        let batch = applied.expect("right applied a batch").id;
        assert_eq!(roots(&right).len(), 1);
        assert!(right.sync_undo(&batch).unwrap().restored > 0);
        assert_eq!(roots(&right), vec![right_cash.clone()]);
        assert!(
            right
                .doc
                .transactions
                .iter()
                .all(|tx| tx.touches(&right_cash))
        );
    }

    #[test]
    fn the_cash_wallet_still_cannot_be_merged_away_by_the_user() {
        let (_dir, mut vault) = empty_vault();
        let bank = crate::service::test_support::store(&mut vault, "bank", StoreKind::Bank, None);
        let refused = vault.apply_store_change(&StoreChangeInput {
            store_id: CASH_WALLET_ID.into(),
            change: StoreChange::MergeInto { target_id: bank },
        });
        assert!(refused.is_err());
        assert_eq!(roots(&vault).len(), 1);
    }
}
