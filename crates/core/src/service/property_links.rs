//! Shim of the `property_links` area: moved to `crate::household::property_links`
//! (spec core-strangler, D1).

use crate::model::vault::TxProperty;

use super::UnlockedVault;
use super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// Every operation's property, for the operation form.
        pub fn tx_property_links(&self) -> Vec<TxProperty>
            => crate::household::property_links::tx_property_links @doc;
        pub fn property_of_tx(&self, tx_id: &str) -> Option<&str>
            => crate::household::property_links::property_of_tx @doc;
    }
    // forward:end
}
