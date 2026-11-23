//! Scenarios of the relations area: reads only. What a wallet is worth is the
//! portfolio area's (`store_overview`, called by method: OD6), so the scenario
//! hands it to the pure card.

use crate::Amount;
use crate::household::relations::{self, EntityCard, EntityRef};
use crate::service::Session;

/// A wallet's base value with its sub-accounts and the currency, for a card.
pub fn store_worth(vault: &Session) -> impl Fn(&str) -> Option<(Amount, String)> {
    |id| {
        crate::money::portfolio::store_overview(&vault.books(), id, true)
            .ok()
            .map(|overview| (overview.subtree_base, overview.base_currency))
    }
}

/// The card of `target`: its head and every non-empty group of links.
pub fn entity_card(vault: &Session, target: &EntityRef) -> EntityCard {
    relations::card_of(&vault.doc, target, &store_worth(vault))
}
