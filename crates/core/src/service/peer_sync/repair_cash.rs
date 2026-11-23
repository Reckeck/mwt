//! Shim of the `peer_sync/repair_cash` area: moved to `crate::sync::repair::repair_cash` (spec core-strangler, D1).

#[cfg(test)]
use crate::service::CASH_WALLET_ID;
#[cfg(test)]
use crate::service::UnlockedVault;

#[cfg(test)]
mod tests {
    use super::super::tests::{exchange, settle};
    use super::*;
    use crate::model::store::{StoreKind, SubType};
    use crate::money::store_change::{StoreChange, StoreChangeInput};
    use crate::service::StoreExtras;
    use crate::service::test_support::{empty_vault, income, usd};
    use crate::sync::track::Tracker;
    use crate::usecases::sync::peer_sync;

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
        vault.tracker = Some(Tracker::capture(&peer_sync::doc_value(vault).unwrap()));
        vault.persist().unwrap();
        old
    }

    fn device(name: &str) -> (tempfile::TempDir, UnlockedVault) {
        let (dir, mut vault) = empty_vault();
        peer_sync::set_device(&mut vault, name);
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
        crate::usecases::money::stores::create_store_with(
            vault,
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
        crate::usecases::money::transactions::add_transaction(
            &mut left,
            income(&left_cash, "10", "left"),
        )
        .unwrap();
        crate::usecases::money::transactions::add_transaction(
            &mut right,
            income(&right_cash, "20", "right"),
        )
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
        let before = crate::usecases::sync::read::sync_manifest(&mut left, None).unwrap();
        assert_eq!(exchange(&mut left, &mut right), (None, None));
        assert_eq!(
            crate::usecases::sync::read::sync_manifest(&mut left, None).unwrap(),
            before
        );
    }

    #[test]
    fn the_wallet_named_cash_beats_an_old_random_one_on_both_devices() {
        let (_one, mut left) = device("aaaaaaaa-left");
        let (_two, mut right) = device("bbbbbbbb-right");
        let old = old_build(&mut left);
        crate::usecases::money::transactions::add_transaction(
            &mut right,
            income(CASH_WALLET_ID, "5", "right"),
        )
        .unwrap();
        crate::usecases::money::transactions::add_transaction(&mut left, income(&old, "7", "left"))
            .unwrap();
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
        assert_eq!(
            crate::money::tree::find_store(&left.doc, &other)
                .unwrap()
                .name,
            "Wallet"
        );
        assert_eq!(
            crate::money::tree::find_store(&left.doc, &mine)
                .unwrap()
                .name,
            "Wallet (2)"
        );
    }

    #[test]
    fn undoing_the_batch_brings_the_two_wallets_back() {
        let (_one, mut left) = device("aaaaaaaa-left");
        let (_two, mut right) = device("bbbbbbbb-right");
        let left_cash = old_build(&mut left);
        let right_cash = old_build(&mut right);
        crate::usecases::money::transactions::add_transaction(
            &mut left,
            income(&left_cash, "10", "left"),
        )
        .unwrap();
        crate::usecases::money::transactions::add_transaction(
            &mut right,
            income(&right_cash, "20", "right"),
        )
        .unwrap();
        let (_, applied) = exchange(&mut left, &mut right);
        let batch = applied.expect("right applied a batch").id;
        assert_eq!(roots(&right).len(), 1);
        assert!(peer_sync::sync_undo(&mut right, &batch).unwrap().restored > 0);
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
        let refused = crate::usecases::money::store_change::apply_store_change(
            &mut vault,
            &StoreChangeInput {
                store_id: CASH_WALLET_ID.into(),
                change: StoreChange::MergeInto { target_id: bank },
            },
        );
        assert!(refused.is_err());
        assert_eq!(roots(&vault).len(), 1);
    }
}
