//! Shim of the `stores` area: moved to `crate::money::stores` (spec
//! core-strangler, D1). The tree rules live there; the scenarios that create,
//! edit, move, archive and delete wallets live in `usecases::money::stores`.

pub use crate::usecases::money::stores::StoreMoveTargets;

use crate::error::Result;
use crate::model::store::{Store, StoreKind};

use super::forward::forward;
use super::{FirstCard, StoreExtras, UnlockedVault};

impl UnlockedVault {
    // forward:begin
    forward! {
        #[cfg(test)]
        pub(crate) fn find_store(&self, id: &str) -> Option<&Store>
            => crate::money::tree::find_store @doc;
        /// 0 for a top-level wallet.
        pub fn depth(&self, id: &str) -> usize
            => crate::money::tree::depth @doc;
        /// Top-level wallet that `id` belongs to (itself for a root).
        pub fn root_of(&self, id: &str) -> String
            => crate::money::tree::root_of @doc;
        pub fn create_store(&mut self, name: &str, kind: StoreKind, color: &str, note: &str, parent_id: Option<String>) -> Result<Store>
            => crate::usecases::money::stores::create_store;
        /// The vault always has exactly one cash wallet; called on create and
        /// open. Returns whether one had to be added.
        pub fn ensure_cash_wallet(&mut self, name: &str) -> Result<bool>
            => crate::usecases::money::stores::ensure_cash_wallet;
        pub fn create_store_with(&mut self, name: &str, kind: StoreKind, color: &str, note: &str, parent_id: Option<String>, extras: StoreExtras) -> Result<Store>
            => crate::usecases::money::stores::create_store_with;
        /// A new bank together with its first card: a card is a bank account, so
        /// a bank is never left without one. The opening balance goes on the
        /// card. All or nothing.
        pub fn create_bank_with_card(&mut self, name: &str, color: &str, note: &str, mut extras: StoreExtras, card: FirstCard) -> Result<Store>
            => crate::usecases::money::stores::create_bank_with_card;
        /// Edit a store's own fields. Its place in the tree changes only through
        /// [`Self::move_store`].
        pub fn update_store(&mut self, id: &str, name: &str, color: &str, note: &str, extras: StoreExtras) -> Result<Store>
            => crate::usecases::money::stores::update_store;
        /// Re-hang a sub-account under another node, or make it a top-level
        /// wallet with `parent_id = None`. A top-level wallet never moves (R6).
        pub fn move_store(&mut self, id: &str, parent_id: Option<&str>) -> Result<Store>
            => crate::usecases::money::stores::move_store;
        /// Where `id` may be moved: every allowed parent (flat, without the
        /// current one) and whether it may become a top-level wallet.
        pub fn store_move_targets(&self, id: &str) -> Result<StoreMoveTargets>
            => crate::usecases::money::stores::store_move_targets;
        /// Archive (or restore) a store together with its whole subtree. A node
        /// cannot be restored while an ancestor is still archived (R8).
        pub fn archive_store(&mut self, id: &str, archived: bool) -> Result<()>
            => crate::usecases::money::stores::archive_store;
        pub fn delete_store(&mut self, id: &str) -> Result<()>
            => crate::usecases::money::stores::delete_store;
    }
    // forward:end
}
