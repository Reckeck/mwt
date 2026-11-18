//! Shim of the `envelopes` area: moved to `crate::planning::envelopes` (spec core-strangler, D1).

pub use crate::planning::envelopes::{EnvelopeDraft, EnvelopeView};

use crate::Amount;
use crate::error::Result;
use crate::model::ids::StoreId;
use crate::model::planning::Envelope;
use crate::model::store::AssetRef;

use super::UnlockedVault;
use super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// Sum earmarked by every envelope of one asset in one store.
        pub fn earmarked(&self, store_id: &str, asset_key: &str) -> Amount
            => crate::planning::envelopes::earmarked @doc with (store_id => &StoreId::from(store_id));
        /// Balance of the asset in the store that no envelope claims.
        pub fn free_balance(&self, store_id: &str, asset: &AssetRef) -> Amount
            => crate::planning::envelopes::free_balance @doc with (store_id => &StoreId::from(store_id));
        /// Envelopes of one store, or of every store with `None`.
        pub fn envelopes(&self, store_id: Option<&str>) -> Vec<EnvelopeView>
            => crate::planning::envelopes::envelopes @doc with (store_id => store_id.map(StoreId::from));
        pub fn create_envelope(&mut self, draft: EnvelopeDraft) -> Result<Envelope>
            => crate::usecases::household::envelopes::create_envelope;
        /// Rename, retarget or relink an envelope. Its store and asset are fixed
        /// once money is earmarked.
        pub fn update_envelope(&mut self, id: &str, draft: EnvelopeDraft) -> Result<Envelope>
            => crate::usecases::household::envelopes::update_envelope;
        /// Earmark more (`amount > 0`, taken from the free balance) or release
        /// (`amount < 0`, back to free). Releasing below zero is refused.
        pub fn fund_envelope(&mut self, id: &str, amount: Amount) -> Result<Envelope>
            => crate::usecases::household::envelopes::fund_envelope;
        /// Move earmarked money between two envelopes of the same store and asset.
        pub fn move_between_envelopes(&mut self, from: &str, to: &str, amount: Amount) -> Result<()>
            => crate::usecases::household::envelopes::move_between_envelopes;
        /// Delete an envelope; whatever it held simply becomes free again.
        pub fn delete_envelope(&mut self, id: &str) -> Result<()>
            => crate::usecases::household::envelopes::delete_envelope;
        /// Put aside up to `amount` of the goal's currency in `store_id` for the
        /// goal: its envelope there is made if missing, and only the free balance
        /// is taken. Returns what was earmarked; the goal counts its envelopes.
        pub fn fund_goal(&mut self, goal_id: &str, store_id: &str, amount: Amount) -> Result<Amount>
            => crate::usecases::household::envelopes::fund_goal with (store_id => &StoreId::from(store_id));
    }
    // forward:end

    // Not part of the baseline (crate-internal): the callers outside the area keep these names.
    forward! {
        /// Value of the envelopes linked to a goal, in `currency` at `now`.
        pub(crate) fn envelope_saved_for_goal(&self, goal_id: &str, currency: &str, now: i64) -> Option<Amount>
            => crate::planning::envelopes::envelope_saved_for_goal @books;
        /// Give every envelope its opening entry and re-derive `allocated` from
        /// the journal (on open, and after a sync brought records in). True when
        /// something changed.
        pub(crate) fn settle_envelopes(&mut self) -> bool
            => crate::usecases::household::envelopes::settle_envelopes;
    }

    /// The id of the envelope that holds a goal's money in one wallet and asset;
    /// the sync area's, so two devices make one envelope (OD6).
    pub(crate) fn goal_envelope_id(goal_id: &str, store_id: &str, asset_key: &str) -> String {
        super::sync_ids::goal_envelope_id(goal_id, store_id, asset_key)
    }
}
