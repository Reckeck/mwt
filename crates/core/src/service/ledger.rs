//! Shim of the `ledger` area: moved to `crate::money::ledger` (spec core-strangler, D1).

pub use crate::money::ledger::{
    GRAPH_NODE_CAP, GraphEdge, GraphEdgeKind, GraphLane, GraphNode, TxFilter, TxGraph, TxPage,
};

use crate::error::Result;
use crate::model::transaction::Transaction;

use super::UnlockedVault;
use super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// The filtered transactions, newest first.
        pub fn list_transactions(&self, filter: &TxFilter, zone: &str) -> Result<Vec<Transaction>>
            => crate::usecases::household::ledger::list_transactions;
        /// The filtered operations as CSV, oldest first, one line per leg: the
        /// "Export ▾ › Transactions by filter" of the list.
        pub fn transactions_csv(&self, filter: &TxFilter, zone: &str) -> Result<String>
            => crate::usecases::household::ledger::transactions_csv;
        /// `limit` operations from `offset`, newest first: a long journal goes to
        /// the screen a page at a time instead of whole on every refresh.
        pub fn list_transactions_page(&self, filter: &TxFilter, offset: u32, limit: u32, zone: &str) -> Result<TxPage>
            => crate::usecases::household::ledger::list_transactions_page;
        /// The filtered transactions as lanes and edges. `collapsed` lists stores
        /// whose sub-accounts fold into the store's own lane.
        pub fn tx_graph(&self, filter: &TxFilter, collapsed: &[String], zone: &str) -> Result<TxGraph>
            => crate::usecases::household::ledger::tx_graph;
    }
    // forward:end
}
