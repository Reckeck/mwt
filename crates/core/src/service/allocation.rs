//! Shim of the `allocation` area: the rules moved to `crate::planning::allocation`, the
//! scenarios to `crate::usecases::planning::allocation` (spec core-strangler, D1).

pub use crate::planning::allocation::{
    AssetClassTarget, AssetClassesView, PRESET_GLOBAL, PRESET_LOCAL, PRESET_RISKY, TopupAdvice,
    TopupRow, preset_classes,
};

use super::UnlockedVault;
use super::forward::forward;
use crate::Amount;
use crate::error::Result;
use crate::model::planning::AssetClass;

impl UnlockedVault {
    // forward:begin
    forward! {
        pub fn asset_classes(&self) -> AssetClassesView
            => crate::planning::allocation::asset_classes @doc;
        /// Split `amount` (default: last month's savings) over the baskets so
        /// the shares move towards their targets (plan F). Creates nothing.
        pub fn topup_advice(&self, amount: Option<Amount>, now: i64, zone: &str) -> TopupAdvice
            => crate::planning::allocation::topup_advice @books;
        /// Replace the baskets. Shares must add up to 100; a basket that is gone
        /// takes its assignments with it. An empty list brings the preset back.
        pub fn set_asset_classes(&mut self, classes: Vec<AssetClass>) -> Result<AssetClassesView>
            => crate::usecases::planning::allocation::set_asset_classes;
        /// Put an asset or a store in a basket, or take it out with `None`.
        /// Assigning to a preset basket saves the preset first.
        pub fn assign_asset_class(&mut self, target: AssetClassTarget, class_id: Option<String>) -> Result<AssetClassesView>
            => crate::usecases::planning::allocation::assign_asset_class;
    }
    // forward:end

    // Read by the net-worth area and the peer repair until they move.
    forward! {
        pub(crate) fn effective_classes(&self) -> Vec<AssetClass>
            => crate::planning::allocation::effective_classes @doc;
    }
}
