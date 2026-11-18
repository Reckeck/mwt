//! Shim of the `property_links` area: moved to `crate::household::property_links`
//! (spec core-strangler, D1).

use crate::Result;
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
        /// A link may point only at a live property.
        pub(crate) fn check_property_link(&self, property_id: Option<&str>) -> Result<()>
            => crate::usecases::household::property_links::check_property_link;
        /// Link the operation to a property, or unlink it with `None`. Not persisted.
        pub(crate) fn set_property_link(&mut self, tx_id: &str, property_id: Option<&str>)
            => crate::usecases::household::property_links::set_property_link;
    }
    // forward:end
}
