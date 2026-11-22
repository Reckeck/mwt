//! Tests of the stores area that need the vault (`UnlockedVault`): moved from
//! `money/stores_tests.rs` with their assertions unchanged (crate-split T13).

use crate::model::store::StoreKind;
use crate::money::store_change::{StoreChange, StoreChangeInput};
use crate::service::UnlockedVault;
use crate::service::test_support::*;

#[test]
fn the_merge_scenario_does_what_apply_store_change_does() {
    let build = |vault: &mut UnlockedVault| {
        let first = store(vault, "First", StoreKind::Cash, None);
        let second = store(vault, "Second", StoreKind::Cash, None);
        (first, second)
    };
    let (_dir, mut by_scenario) = empty_vault();
    let (first, second) = build(&mut by_scenario);
    let (_other_dir, mut by_apply) = empty_vault();
    let (first_apply, second_apply) = build(&mut by_apply);
    let input = |source: &str, target: &str| StoreChangeInput {
        store_id: source.into(),
        change: StoreChange::MergeInto {
            target_id: target.into(),
        },
    };
    let via_scenario = crate::usecases::money::stores::merge(&mut by_scenario, &first, &second);
    let via_apply = by_apply.apply_store_change(&input(&first_apply, &second_apply));
    assert_eq!(via_scenario.is_ok(), via_apply.is_ok());
    assert_eq!(by_scenario.doc.stores.len(), by_apply.doc.stores.len());
}
