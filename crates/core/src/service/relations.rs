//! Shim of the `relations` area: moved to `crate::household::relations` (spec core-strangler, D1).

pub use crate::household::relations::{
    EntityCard, EntityKind, EntityRef, RELATION_ITEMS_CAP, RelationGroup, RelationItem,
    RelationKind,
};

use super::UnlockedVault;
use super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// Everything linked to `target`, grouped; empty groups are left out.
        pub fn relations(&self, target: &EntityRef) -> Vec<RelationGroup>
            => crate::household::relations::relations @doc;
        /// The card of `target`: its head and every non-empty group of links.
        pub fn entity_card(&self, target: &EntityRef) -> EntityCard
            => crate::usecases::household::relations::entity_card;
    }
    // forward:end
}
