//! Shim of the `price_info` area: moved to `crate::money::price_info` (spec core-strangler, D1).

#[cfg(test)]
pub use crate::money::price_info::PriceQuote;
pub use crate::money::price_info::{PriceInfo, PriceOrigin};

use super::UnlockedVault;
use super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// The current USD price of one unit, and where it came from. A hand-set
        /// price (which only the vault can value) comes first, then the rest.
        #[cfg(test)]
        pub(crate) fn price_quote_for(&self, asset_key: &str) -> PriceQuote
            => crate::money::price_info::price_quote_for @books;
    }
    // forward:end
}
