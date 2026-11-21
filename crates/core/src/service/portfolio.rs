//! Shim of the `portfolio` area: moved to `crate::money::portfolio` (spec
//! core-strangler, D1). The views and the holdings fold live there; the scenario
//! that stores the holdings lives in `usecases::money::portfolio`.

pub use crate::money::portfolio::{
    AssetBalance, AssetStat, HistoryPoint, NamedValue, PnlLeg, PortfolioSummary, RealizedPnl,
    StoreChildRow, StoreCrumb, StoreOverview, StorePosition, StoreTreeNode, StoreValue,
};

use std::collections::HashSet;

use crate::Amount;
use crate::error::Result;
#[cfg(test)]
use crate::model::store::Store;
use crate::model::store::StoreScope;
use crate::model::transaction::Transaction;

use super::UnlockedVault;
use super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// Re-derive `doc.holdings` from the transactions and lay the crypto
        /// module's balances over them. Saves nothing: the caller persists.
        pub(crate) fn recompute_holdings(&mut self)
            => crate::usecases::money::portfolio::recompute_holdings;
        /// Base currency code (never empty) and its **latest** cached FX rate.
        pub(crate) fn base_and_fx(&self) -> (String, Amount)
            => crate::usecases::money::portfolio::base_and_fx;
        /// Current USD price of one unit; see [`crate::money::price_info::price_quote_for`].
        pub(crate) fn price_usd_for(&self, asset_key: &str) -> Amount
            => crate::money::portfolio::price_usd_for @books;
        /// Base-currency price of one unit (the base's own fiat is exactly 1).
        pub(crate) fn price_base_for(&self, asset_key: &str, fx: Amount) -> Amount
            => crate::money::portfolio::price_base_for @books;
        /// Every store id a scope covers. An empty id list means the whole vault.
        pub fn resolve_scope(&self, scope: &StoreScope) -> HashSet<String>
            => crate::money::tree::resolve_scope @doc;
        /// Transactions with at least one leg on a store in `ids`.
        pub(crate) fn scoped_txs(&self, ids: &HashSet<String>) -> Vec<Transaction>
            => crate::money::portfolio::scoped_txs @doc;
        /// A store's positions, sorted by value descending.
        #[cfg(test)]
        pub(crate) fn store_positions(&self, store_id: &str, fx: Amount) -> Vec<StorePosition>
            => crate::money::portfolio::store_positions @books;
        /// Direct children of a store.
        #[cfg(test)]
        pub(crate) fn children_of(&self, parent_id: &str) -> Vec<&Store>
            => crate::money::tree::children_of @doc;
        /// Every descendant id of `root_id`, without `root_id` itself.
        pub(crate) fn subtree_ids(&self, root_id: &str) -> Vec<String>
            => crate::money::tree::subtree_ids @doc;
        /// The whole wallet tree with own and rolled-up values. Roots (and
        /// orphans whose parent is gone) come first; live before archived,
        /// alphabetical inside each group.
        pub fn store_tree(&self) -> Vec<StoreTreeNode>
            => crate::money::portfolio::store_tree @books;
        /// Single-round-trip view of one store for the wallet page.
        /// `include_children` widens the totals, PnL and transaction stats to the
        /// whole subtree; positions are reported both ways regardless.
        pub fn store_overview(&self, store_id: &str, include_children: bool) -> Result<StoreOverview>
            => crate::money::portfolio::store_overview @books;
        /// Portfolio value of one store (optionally with its subtree) over time.
        pub fn store_history(&self, store_id: &str, include_children: bool, base_currency: &str) -> Vec<HistoryPoint>
            => crate::money::portfolio::store_history @books;
        /// Realized PnL of one store (optionally with its subtree).
        pub fn store_pnl(&self, store_id: &str, include_children: bool) -> RealizedPnl
            => crate::money::portfolio::store_pnl @books;
        /// Timestamp of the freshest cached price or FX point.
        pub fn prices_as_of(&self) -> Option<String>
            => crate::money::portfolio::prices_as_of @doc;
        pub fn portfolio_summary(&self) -> PortfolioSummary
            => crate::usecases::money::capital::portfolio_summary @books;
        /// Portfolio value over time using transaction timestamps as event
        /// boundaries and the nearest cached price (≤ event ts) per asset.
        pub fn history_series(&self, base_currency: &str) -> Vec<HistoryPoint>
            => crate::money::portfolio::history_series @books;
        pub fn realized_pnl(&self) -> RealizedPnl
            => crate::money::portfolio::realized_pnl @books;
        /// Shared PnL replay. `scope` restricts it to the legs booked on those
        /// stores (`None` replays the whole vault).
        pub(crate) fn realized_pnl_filtered(&self, scope: Option<&HashSet<String>>) -> RealizedPnl
            => crate::money::portfolio::realized_pnl_filtered @books;
    }
    // forward:end
}
