//! Shim of the `worth` area: moved to `crate::money::worth` (spec core-strangler, D1).
//! The assembly of `NetWorth` and the scenarios that write hand-set prices live
//! in `usecases::money::capital`.

pub use crate::money::worth::{ClassShare, FlowClass, LiabilityShare, NetWorth};

use crate::Amount;
use crate::error::Result;
use crate::model::transaction::Transaction;
use crate::model::vault::ManualPrice;

use super::UnlockedVault;
use super::forward::forward;

impl UnlockedVault {
    /// A spending flow by its category, `Daily` when it has none.
    pub(crate) fn spending(group: Option<crate::model::category::CategoryGroup>) -> FlowClass {
        crate::money::worth::spending(group)
    }

    // forward:begin
    forward! {
        /// Units of `currency` per USD from the FX cache; USD itself is 1.
        pub(crate) fn per_usd(&self, currency: &str) -> Option<Amount>
            => crate::money::rates::per_usd @doc;
        /// USD value of one unit of a hand-priced asset, if it has a price and the
        /// rate of its currency is known.
        #[cfg(test)]
        pub(crate) fn manual_price_usd(&self, asset_key: &str) -> Option<Amount>
            => crate::usecases::money::capital::manual_price_usd @books;
        /// Set the value of one unit of a property asset. Applies to its whole
        /// history (plan D24), so revaluing does not make the chart jump.
        pub fn set_manual_price(&mut self, asset_key: &str, unit_price: Amount, currency: &str) -> Result<ManualPrice>
            => crate::usecases::money::capital::set_manual_price;
        pub fn clear_manual_price(&mut self, asset_key: &str) -> Result<()>
            => crate::usecases::money::capital::clear_manual_price;
        pub fn net_worth(&self) -> NetWorth
            => crate::usecases::money::capital::net_worth @books;
        /// The one classification budgets, cash flow, month totals and the
        /// calendar share (plan §7.3).
        pub fn classify_flow(&self, tx: &Transaction) -> FlowClass
            => crate::money::worth::classify_flow @doc;
    }
    // forward:end
}
