//! Shim of the `losses` area: moved to `crate::household::losses` (spec core-strangler, D1).

pub use crate::household::losses::{
    FlowAmount, FlowLink, FlowSummary, FxRow, FxTable, LossBucket, LossRow, TxLoss,
};
pub use crate::reports::overview::LossReport;

use std::collections::BTreeMap;

use crate::error::Result;

use super::UnlockedVault;
use super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// The loss of one transaction in the base currency.
        pub fn tx_loss(&self, id: &str) -> Result<TxLoss>
            => crate::usecases::household::losses::tx_loss;
        /// Losses of every transaction that has one, in the base currency, by id
        /// — for the loss column of the list.
        pub fn tx_losses(&self) -> BTreeMap<String, TxLoss>
            => crate::usecases::household::losses::tx_losses;
        /// Put transactions into one chain: the given one, else the chain one of
        /// them is already in, else a new one. Returns the chain id.
        pub fn link_transactions(&mut self, ids: &[String], flow_id: Option<&str>) -> Result<String>
            => crate::usecases::household::losses::link_transactions;
        /// Take a transaction out of its chain.
        pub fn unlink_transaction(&mut self, id: &str) -> Result<()>
            => crate::usecases::household::losses::unlink_transaction;
        /// Every chain with a link in `[from, to)` (RFC3339 bounds; empty = open),
        /// newest first, valued in the base currency.
        pub fn list_flows(&self, from: Option<&str>, to: Option<&str>, store_ids: &[String]) -> Vec<FlowSummary>
            => crate::usecases::household::losses::list_flows;
        /// Every cached currency against the base, latest rates (plan F3b, "Rates"),
        /// plus a row per crypto favourite that has a cached price.
        pub fn fx_table(&self, favorites: &[String]) -> FxTable
            => crate::usecases::household::losses::fx_table;
    }
    // forward:end
}
