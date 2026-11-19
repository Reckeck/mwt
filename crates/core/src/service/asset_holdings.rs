//! Shim of the `asset_holdings` area: moved to `crate::money::asset_holdings`
//! (spec core-strangler, D1).

pub use crate::money::asset_holdings::{AssetHolding, AssetHoldings};

use crate::model::store::AssetRef;

use super::UnlockedVault;
use super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// The same asset (`AssetRef::same_as`: one key, or one ticker on one
        /// network) on every wallet. Derivatives are never joined to their coin.
        pub fn asset_holdings(&self, asset: &AssetRef) -> AssetHoldings
            => crate::money::asset_holdings::asset_holdings @books;
    }
    // forward:end
}
