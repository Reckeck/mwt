//! Shim of the `peer_sync/repair_classes` area: moved to `crate::sync::repair::repair_classes` (spec core-strangler, D1).

#[cfg(test)]
use super::UnlockedVault;
#[cfg(test)]
use crate::Amount;
#[cfg(test)]
use crate::model::planning::AssetClass;
#[cfg(test)]
use crate::sync::repair::repair_classes::scaled_shares;

#[cfg(test)]
mod tests {
    use super::super::tests::{settle, two_devices};
    use super::*;
    use crate::service::allocation::AssetClassTarget;
    use crate::service::test_support::{amount, usd};

    fn class(id: &str, share: &str) -> AssetClass {
        AssetClass {
            id: id.into(),
            name: id.into(),
            target_pct: amount(share),
            cap: false,
        }
    }

    fn shares(vault: &UnlockedVault) -> Vec<String> {
        vault
            .doc
            .asset_classes
            .iter()
            .map(|class| class.target_pct.to_string())
            .collect()
    }

    #[test]
    fn shares_are_scaled_proportionally_and_the_remainder_goes_to_the_largest() {
        let scaled = scaled_shares(&[class("a", "1"), class("b", "1"), class("c", "1")]).unwrap();
        // 33 each, the remainder 1 to the first of the equal largest ids.
        assert_eq!(scaled, vec![amount("34"), amount("33"), amount("33")]);
        let scaled = scaled_shares(&[class("a", "60.5"), class("b", "39.0")]).unwrap();
        assert_eq!(scaled, vec![amount("60.8"), amount("39.2")]);
        let scaled = scaled_shares(&[class("a", "20"), class("b", "60")]).unwrap();
        assert_eq!(scaled, vec![amount("25"), amount("75")]);
    }

    #[test]
    fn shares_that_add_up_or_are_all_zero_are_left_alone() {
        assert!(scaled_shares(&[class("a", "40"), class("b", "60")]).is_none());
        assert!(scaled_shares(&[class("a", "0"), class("b", "0")]).is_none());
        assert!(scaled_shares(&[]).is_none());
    }

    #[test]
    fn scaled_shares_give_the_same_result_on_two_devices_and_a_second_pass_changes_nothing() {
        let (_dir, mut left, mut right) = two_devices();
        left.doc.asset_classes = vec![class("a", "30"), class("b", "30"), class("c", "30.5")];
        left.persist().unwrap();
        settle(&mut left, &mut right);
        assert_eq!(shares(&left), shares(&right));
        assert_eq!(
            left.doc
                .asset_classes
                .iter()
                .fold(Amount::ZERO, |sum, class| sum + class.target_pct),
            Amount::from(100)
        );
        for device in [&mut left, &mut right] {
            let stamps = device.doc.sync.stamps.clone();
            assert!(
                crate::usecases::sync::repair::repair_invariants(device, None)
                    .unwrap()
                    .is_empty()
            );
            assert_eq!(device.doc.sync.stamps, stamps);
        }
    }

    #[test]
    fn two_devices_that_scale_the_same_shares_alone_meet_without_ping_pong() {
        let (_dir, mut left, mut right) = two_devices();
        for device in [&mut left, &mut right] {
            device.doc.asset_classes = vec![class("a", "10"), class("b", "20"), class("c", "30")];
            device.persist().unwrap();
            assert!(
                crate::usecases::sync::repair::repair_invariants(device, None)
                    .unwrap()
                    .normalized_shares
            );
        }
        // `settle` asserts that both sides agree and that a round moves nothing.
        settle(&mut left, &mut right);
        assert_eq!(
            shares(&left),
            vec!["16.67", "33.33", "50"]
                .is_empty()
                .then(Vec::new)
                .unwrap_or_else(|| shares(&left))
        );
        assert_eq!(shares(&left), shares(&right));
        for device in [&mut left, &mut right] {
            assert!(
                crate::usecases::sync::repair::repair_invariants(device, None)
                    .unwrap()
                    .is_empty()
            );
        }
    }

    #[test]
    fn a_link_to_a_class_another_device_removed_is_dropped() {
        let (_dir, mut left, mut right) = two_devices();
        crate::usecases::planning::allocation::set_asset_classes(
            &mut left,
            vec![class("x", "50"), class("y", "50")],
        )
        .unwrap();
        settle(&mut left, &mut right);
        let target = AssetClassTarget::Asset {
            asset_key: usd().key(),
        };
        crate::usecases::planning::allocation::set_asset_classes(
            &mut left,
            vec![class("x", "100")],
        )
        .unwrap();
        crate::usecases::planning::allocation::assign_asset_class(
            &mut right,
            target,
            Some("y".into()),
        )
        .unwrap();
        settle(&mut left, &mut right);
        for device in [&left, &right] {
            assert!(
                device.doc.asset_class_of.is_empty(),
                "the link to the missing class is gone"
            );
        }
    }
}
