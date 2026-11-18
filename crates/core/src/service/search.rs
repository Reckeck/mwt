//! Shim of the `search` area: moved to `crate::money::search` (spec core-strangler, D1).

pub use crate::money::search::{PINNED_CAP, PinnedView, SEARCH_TX_CAP, SearchEntry};

use crate::error::Result;
use crate::model::entity_ref::Pinned;
use crate::model::transaction::Transaction;

use super::UnlockedVault;
use super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// Every entity by name, for the palette; wallets are left to the store tree.
        pub fn search_index(&self) -> Vec<SearchEntry>
            => crate::money::search::search_index @doc;
        /// Operations whose comment, wallet, contact or amount match `query`, or
        /// that carry one of `categories` (the frontend matches the translated
        /// names of built-in categories itself). Newest first.
        pub fn search_transactions(&self, query: &str, categories: &[String], limit: usize) -> Vec<Transaction>
            => crate::money::search::search_transactions @doc;
        /// The pins with their short values.
        pub fn pinned(&self, now: i64, zone: &str) -> Vec<PinnedView>
            => crate::usecases::household::search::pinned;
        /// Replaces the pins: blanks and repeats are dropped, a filter without an
        /// id gets one, the list is capped.
        pub fn set_pinned(&mut self, pins: Vec<Pinned>) -> Result<()>
            => crate::usecases::household::search::set_pinned;
    }
    // forward:end
}
