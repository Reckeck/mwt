//! Shim of the `peer_sync/repair` area: moved to `crate::sync::repair::repair` (spec core-strangler, D1).

#[cfg(test)]
use super::UnlockedVault;
#[cfg(test)]
use crate::model::store::Binding;

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
        crate::usecases::money::stores::create_store_with(
            vault,
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
        crate::money::tree::find_store(&vault.doc, id).and_then(|node| node.parent_id.clone())
    }

    fn goal(vault: &mut UnlockedVault, wallet: &str) -> String {
        crate::usecases::planning::goals::create_goal(
            vault,
            GoalDraft {
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
            },
        )
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
            assert!(
                crate::money::tree::find_store(&device.doc, &one)
                    .unwrap()
                    .archived
            );
            assert_eq!(parent_of(device, &two).as_deref(), Some(one.as_str()));
            assert!(crate::sync::repair::repair::account_cycle(&device.doc).is_none());
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
        crate::usecases::money::stores::move_store(&mut left, &savings, Some(&second)).unwrap();
        crate::usecases::money::stores::delete_store(&mut right, &second).unwrap();
        settle(&mut left, &mut right);
        for device in [&left, &right] {
            let node =
                crate::money::tree::find_store(&device.doc, &savings).expect("the account stays");
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
        let report = crate::usecases::sync::repair::repair_invariants(&mut vault, None).unwrap();
        assert_eq!(report.detached_accounts.len(), 1);
        assert_eq!(parent_of(&vault, &two), None);
        assert_eq!(parent_of(&vault, &one).as_deref(), Some(bank.as_str()));
    }

    #[test]
    fn an_envelope_topped_up_after_its_goal_was_deleted_is_deleted_again() {
        let (_dir, mut left, mut right) = two_devices();
        let wallet = store(&mut left, "Cash", StoreKind::Cash, None);
        crate::usecases::money::transactions::add_transaction(
            &mut left,
            income(&wallet, "1000", "pay"),
        )
        .unwrap();
        let goal_id = goal(&mut left, &wallet);
        crate::usecases::household::envelopes::fund_goal(
            &mut left,
            &goal_id,
            &mwt_model::StoreId::from(&wallet),
            amount("100"),
        )
        .unwrap();
        settle(&mut left, &mut right);
        let envelope = left.doc.envelopes[0].id.clone();
        crate::usecases::planning::goals::delete_goal(
            &mut left,
            &goal_id,
            chrono::Utc::now().timestamp(),
        )
        .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(3));
        crate::usecases::household::envelopes::fund_envelope(&mut right, &envelope, amount("50"))
            .unwrap();
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
        assert!(
            !crate::usecases::sync::repair::repair_invariants(&mut vault, None)
                .unwrap()
                .is_empty()
        );
        let stamps = vault.doc.sync.stamps.clone();
        let tombstones = vault.doc.sync.tombstones.clone();
        let log = vault.doc.notification_log.len();
        assert!(
            crate::usecases::sync::repair::repair_invariants(&mut vault, None)
                .unwrap()
                .is_empty()
        );
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
        crate::usecases::money::stores::move_store(&mut left, &savings, Some(&second)).unwrap();
        crate::usecases::money::stores::delete_store(&mut right, &second).unwrap();
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
            crate::usecases::sync::repair::repair_invariants(device, None).unwrap();
        }
        settle(&mut left, &mut right);
        for device in [&left, &right] {
            assert!(crate::sync::repair::repair::account_cycle(&device.doc).is_none());
            assert!(device.doc.stores.iter().all(|node| {
                node.parent_id.as_deref().is_none_or(|parent| {
                    crate::money::tree::find_store(&device.doc, parent)
                        .is_some_and(|up| up.parent_id.is_none())
                })
            }));
        }
    }
}
