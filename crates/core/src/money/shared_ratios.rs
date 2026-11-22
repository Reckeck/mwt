//! What shared events do to spending: reports, budgets and "Where it went" count
//! my part of an event, not what left my wallet.
//!
//! * A payment of mine for an event counts for my part of that purchase (the
//!   rest is "Owed to me").
//! * My part of a purchase somebody else made counts from the day of the
//!   purchase, though I have paid nothing yet.
//! * What I wrote off counts as my expense in the event's category.
//! * A return (`Repay`/`Collect`) is a debt transfer, which is never spending,
//!   so it is not counted twice.
//!
//! An event I do not take part in is invisible here. `Books` keeps both results,
//! so a request works them out once.

use std::collections::HashMap;

use super::shared_split::{self as split, FALLBACK_CATEGORY, item_category};
use crate::Amount;
use crate::model::VaultDoc;
use crate::model::shared_event::{ME, SettlementKind};
use crate::time::secs_or_now;

/// One extra spending entry of mine: a part of somebody else's purchase or a
/// write-off, in the event currency.
#[derive(Debug, Clone)]
pub(crate) struct SharedExtra {
    pub(crate) secs: i64,
    pub(crate) event_currency: String,
    pub(crate) category: String,
    pub(crate) value: Amount,
    pub(crate) tag: String,
}

/// For each payment of mine linked to an event: the share of it that is
/// my own expense.
pub(crate) fn shared_payment_ratios(doc: &VaultDoc) -> HashMap<String, Amount> {
    let mut ratios = HashMap::new();
    for event in doc.shared_events.iter().filter(|event| event.i_participate) {
        let computed = split::compute(event);
        for line in computed.purchases.iter().filter(|line| line.buyer == ME) {
            let tx_id = event
                .items
                .iter()
                .flat_map(|item| &item.purchases)
                .find(|purchase| purchase.id == line.purchase_id)
                .and_then(|purchase| purchase.tx_id.clone());
            let Some(tx_id) = tx_id else { continue };
            let ratio = if line.event_amount.is_zero() {
                Amount::ZERO
            } else {
                line.my_part / line.event_amount
            };
            ratios.insert(tx_id, ratio);
        }
    }
    ratios
}

/// Every extra spending entry of mine, in the event currency and not yet
/// valued: it does not depend on the currency, zone or period asked.
pub(crate) fn shared_extra_flows(doc: &VaultDoc) -> Vec<SharedExtra> {
    let mut out = Vec::new();
    for event in doc.shared_events.iter().filter(|event| event.i_participate) {
        let computed = split::compute(event);
        for line in computed
            .purchases
            .iter()
            .filter(|line| line.buyer != ME && line.my_part.is_positive())
        {
            let category = event
                .items
                .iter()
                .find(|item| item.id == line.item_id)
                .map_or_else(
                    || FALLBACK_CATEGORY.to_string(),
                    |item| item_category(event, item),
                );
            out.push(SharedExtra {
                secs: secs_or_now(&line.at),
                event_currency: event.currency.clone(),
                category,
                value: line.my_part,
                tag: format!("shared:{}:{}", event.id, line.purchase_id),
            });
        }
        for settlement in event
            .settlements
            .iter()
            .filter(|settlement| settlement.kind == SettlementKind::WriteOff)
        {
            let category = event
                .category_id
                .clone()
                .unwrap_or_else(|| FALLBACK_CATEGORY.to_string());
            out.push(SharedExtra {
                secs: secs_or_now(&settlement.at),
                event_currency: event.currency.clone(),
                category,
                value: settlement.amount,
                tag: format!("shared:{}:{}", event.id, settlement.id),
            });
        }
    }
    out
}
