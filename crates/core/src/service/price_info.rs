//! Shim of the `price_info` area: moved to `crate::money::price_info` (spec core-strangler, D1).

pub use crate::money::price_info::{PriceInfo, PriceOrigin, PriceQuote};

use crate::Amount;

use super::UnlockedVault;
use super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// The current USD price of one unit, and where it came from. A hand-set
        /// price (which only the vault can value) comes first, then the rest.
        pub(crate) fn price_quote_for(&self, asset_key: &str) -> PriceQuote
            => crate::money::price_info::price_quote_for @books;
        /// The last price a transaction recorded for the asset, with its moment.
        pub(crate) fn last_stamp(&self, asset_key: &str) -> Option<(String, Amount)>
            => crate::money::price_info::last_stamp @doc;
    }
    // forward:end
}
