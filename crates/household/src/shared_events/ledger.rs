//! When the ledger changes an operation an event made or adopted, the event
//! follows: deleting it removes the purchase or the settlement, editing its
//! amount or date edits them. What the event cannot follow (another currency,
//! another kind of operation, more than is owed) is refused.

use mwt_base::Amount;
use mwt_base::error::{Error, Result};
use mwt_model::Transaction;
use mwt_model::VaultDoc;
use mwt_model::{
    Purchase, RemovedWithPayment, SettleDirection, Settlement, SettlementKind, SharedEvent,
};

use super::split;

pub const CHANGE_IN_EVENT: &str =
    "this operation belongs to a shared event: change it in the event";

/// Drop the purchases and settlements of every event that point at an
/// operation about to be deleted. Reopens an archived event. What went is
/// kept in the event: see [`SharedEvent::removed_with_payment`].
pub fn cascade_delete(events: &mut [SharedEvent], tx_id: &str) {
    let is_gone = |id: &Option<String>| id.as_deref() == Some(tx_id);
    for event in events {
        let mut removed = Vec::new();
        for item in &mut event.items {
            let (gone, kept) = std::mem::take(&mut item.purchases)
                .into_iter()
                .partition(|purchase| is_gone(&purchase.tx_id));
            item.purchases = kept;
            removed.extend(
                gone.into_iter()
                    .map(|purchase: Purchase| RemovedWithPayment {
                        id: purchase.id.clone(),
                        tx_id: tx_id.to_string(),
                        item_id: Some(item.id.clone()),
                        purchase: Some(purchase),
                        settlement: None,
                    }),
            );
        }
        let (gone, kept) = std::mem::take(&mut event.settlements)
            .into_iter()
            .partition(|settlement| is_gone(&settlement.tx_id));
        event.settlements = kept;
        removed.extend(
            gone.into_iter()
                .map(|settlement: Settlement| RemovedWithPayment {
                    id: settlement.id.clone(),
                    tx_id: tx_id.to_string(),
                    item_id: None,
                    purchase: None,
                    settlement: Some(settlement),
                }),
        );
        if !removed.is_empty() {
            event.archived = false;
            event.removed_with_payment.extend(removed);
        }
    }
}

/// The event as it will be once the operation `tx_id` becomes `new_tx`;
/// `None` when no event owns the operation. Checks everything first so the
/// ledger edit can go ahead knowing the event follows.
pub fn plan_tx_edit(
    doc: &VaultDoc,
    tx_id: &str,
    new_tx: &Transaction,
) -> Result<Option<SharedEvent>> {
    let refuse = || Error::InvalidInput(CHANGE_IN_EVENT.into());
    for event in &doc.shared_events {
        let mut next = event.clone();
        let mut found = false;
        for item in &mut next.items {
            let Some(purchase) = item
                .purchases
                .iter_mut()
                .find(|purchase| purchase.tx_id.as_deref() == Some(tx_id))
            else {
                continue;
            };
            let Transaction::Payment(payment) = new_tx else {
                return Err(refuse());
            };
            if !payment.asset.is_fiat()
                || !payment
                    .asset
                    .symbol
                    .eq_ignore_ascii_case(&purchase.currency)
            {
                return Err(refuse());
            }
            purchase.amount = payment.amount;
            purchase.price_mode = mwt_model::PriceMode::Total;
            purchase.at = new_tx.meta().timestamp.clone();
            found = true;
        }
        if let Some(index) = next
            .settlements
            .iter()
            .position(|settlement| settlement.tx_id.as_deref() == Some(tx_id))
        {
            let old = doc.transactions.iter().find(|tx| tx.meta().id == tx_id);
            let (Transaction::Transfer(now), Some(Transaction::Transfer(was))) = (new_tx, old)
            else {
                return Err(refuse());
            };
            if now.debt != was.debt
                || !now.asset.symbol.eq_ignore_ascii_case(&event.currency)
                || !now.amount.is_positive()
            {
                return Err(refuse());
            }
            let mut settlement = next.settlements.remove(index);
            debug_assert!(settlement.kind == SettlementKind::Transfer);
            // What is owed with this settlement taken out.
            let computed = split::compute(&next);
            let saldo = computed
                .person(&settlement.participant_id)
                .map_or(Amount::ZERO, |row| row.saldo);
            let allowed = match settlement.direction {
                SettleDirection::ToOrganizer => saldo,
                SettleDirection::FromOrganizer => -saldo,
            };
            if now.amount > allowed + split::settle_tolerance() {
                return Err(Error::InvalidInput(
                    "this is more than is owed in that direction".into(),
                ));
            }
            settlement.amount = now.amount;
            settlement.at = new_tx.meta().timestamp.clone();
            next.settlements.insert(index, settlement);
            found = true;
        }
        if found {
            next.archived = false;
            return Ok(Some(next));
        }
    }
    Ok(None)
}

pub fn apply_edit(events: &mut [SharedEvent], next: Option<SharedEvent>) {
    if let Some(next) = next
        && let Some(slot) = events.iter_mut().find(|event| event.id == next.id)
    {
        *slot = next;
    }
}
