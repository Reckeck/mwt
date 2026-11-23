//! Shim of the `peer_sync/repair_natural` area: moved to `crate::sync::repair::repair_natural` (spec core-strangler, D1).

#[cfg(test)]
use super::UnlockedVault;
#[cfg(test)]
use crate::sync::sync_ids;

#[cfg(test)]
mod tests {
    use super::super::convergence_catalog::add_catalog_stores;
    use super::super::convergence_tests::Ledger;
    use super::super::tests::{settle, two_devices};
    use super::*;
    use crate::household::contacts::DebtTermsDraft;
    use crate::model::contact::DebtTerms;
    use crate::model::store::{StoreKind, StoreScope};
    use crate::money::valuation::ValuationInput;
    use crate::planning::goals::GoalDraft;
    use crate::service::test_support::{amount, income, store};

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
        crate::usecases::household::contacts::set_debt_terms(
            &mut devices[0],
            terms(&contact, "2026-05-01", "left"),
        )
        .unwrap();
        pause();
        crate::usecases::household::contacts::set_debt_terms(
            &mut devices[1],
            terms(&contact, "2026-06-01", "right"),
        )
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
        crate::usecases::household::contacts::set_debt_terms(
            &mut devices[0],
            terms(&contact, "2026-05-01", "new build"),
        )
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
        crate::usecases::household::contacts::set_debt_terms(
            &mut devices[0],
            terms(&contact, "2026-05-01", "a"),
        )
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
        let first = crate::usecases::household::utilities::add_meter_reading(
            vault,
            &ledger.utility_id,
            amount("100"),
            "2026-03",
            Some(3),
            None,
        )
        .unwrap();
        assert_eq!(
            first.id,
            sync_ids::reading_id(&ledger.utility_id, None, &("2026-03".into(), 3))
        );
        let moved = crate::usecases::household::utilities::update_meter_reading(
            vault,
            &first.id,
            amount("100"),
            "2026-03",
            Some(5),
        )
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
        crate::usecases::household::utilities::add_meter_reading(
            &mut devices[0],
            &ledger.utility_id,
            amount("100"),
            "2026-03",
            Some(2),
            None,
        )
        .unwrap();
        pause();
        crate::usecases::household::utilities::add_meter_reading(
            &mut devices[1],
            &ledger.utility_id,
            amount("120"),
            "2026-03",
            Some(2),
            None,
        )
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
        crate::usecases::money::valuation::add_valuation(&mut devices[0], input("1000")).unwrap();
        pause();
        crate::usecases::money::valuation::add_valuation(&mut devices[1], input("1100")).unwrap();
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
        crate::usecases::money::transactions::add_transaction(
            &mut left,
            income(&wallet, "1000", "pay"),
        )
        .unwrap();
        let goal = crate::usecases::planning::goals::create_goal(
            &mut left,
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
                    store_ids: vec![wallet.clone()],
                    include_children: true,
                },
                scenario_id: None,
                expected_return: None,
            },
        )
        .unwrap()
        .id;
        settle(&mut left, &mut right);
        crate::usecases::household::envelopes::fund_goal(
            &mut left,
            &goal,
            &mwt_model::StoreId::from(&wallet),
            amount("100"),
        )
        .unwrap();
        crate::usecases::household::envelopes::fund_goal(
            &mut right,
            &goal,
            &mwt_model::StoreId::from(&wallet),
            amount("50"),
        )
        .unwrap();
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
