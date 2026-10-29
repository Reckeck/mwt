//! When the ledger changes an operation an event made or adopted, the event
//! follows: deleting it removes the purchase or the settlement, editing its
//! amount or date edits them. What the event cannot follow (another currency,
//! another kind of operation, more than is owed) is refused.

use crate::Amount;
use crate::error::{Error, Result};
use crate::model::shared_event::{SettleDirection, SettlementKind, SharedEvent};
use crate::model::transaction::Transaction;

use super::super::UnlockedVault;
use super::split;

const CHANGE_IN_EVENT: &str = "this operation belongs to a shared event: change it in the event";

impl UnlockedVault {
    /// Drop the purchases and settlements of every event that point at an
    /// operation about to be deleted. Reopens an archived event.
    pub(crate) fn cascade_shared_delete(&mut self, tx_id: &str) {
        for event in &mut self.doc.shared_events {
            let mut touched = false;
            for item in &mut event.items {
                let before = item.purchases.len();
                item.purchases
                    .retain(|purchase| purchase.tx_id.as_deref() != Some(tx_id));
                touched |= item.purchases.len() != before;
            }
            let before = event.settlements.len();
            event
                .settlements
                .retain(|settlement| settlement.tx_id.as_deref() != Some(tx_id));
            touched |= event.settlements.len() != before;
            if touched {
                event.archived = false;
            }
        }
    }

    /// The event as it will be once the operation `tx_id` becomes `new_tx`;
    /// `None` when no event owns the operation. Checks everything first so the
    /// ledger edit can go ahead knowing the event follows.
    pub(crate) fn plan_shared_tx_edit(
        &self,
        tx_id: &str,
        new_tx: &Transaction,
    ) -> Result<Option<SharedEvent>> {
        let refuse = || Error::InvalidInput(CHANGE_IN_EVENT.into());
        for event in &self.doc.shared_events {
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
                purchase.price_mode = crate::model::shared_event::PriceMode::Total;
                purchase.at = new_tx.meta().timestamp.clone();
                found = true;
            }
            if let Some(index) = next
                .settlements
                .iter()
                .position(|settlement| settlement.tx_id.as_deref() == Some(tx_id))
            {
                let old = self
                    .doc
                    .transactions
                    .iter()
                    .find(|tx| tx.meta().id == tx_id);
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

    pub(crate) fn apply_shared_edit(&mut self, next: Option<SharedEvent>) {
        if let Some(next) = next
            && let Some(slot) = self
                .doc
                .shared_events
                .iter_mut()
                .find(|event| event.id == next.id)
        {
            *slot = next;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::*;
    use crate::ipc_contract::{SetShareOverrideInput, SettleSharedInput, SharedRemainderInput};
    use crate::model::shared_event::ME;

    fn settle_petya(scenario: &mut Scenario, raw: &str, at: Option<&str>) -> String {
        let wallet = scenario.wallet.clone();
        scenario
            .vault
            .settle_shared(SettleSharedInput {
                event_id: scenario.event.clone(),
                participant_id: "petya".into(),
                direction: SettleDirection::ToOrganizer,
                amount: amount(raw),
                store_id: Some(wallet),
                at: at.map(str::to_string),
            })
            .unwrap();
        scenario
            .vault
            .shared_tx_links()
            .into_iter()
            .find(|link| link.link.settlement_id.is_some())
            .unwrap()
            .tx_id
    }

    fn petya_debt(scenario: &Scenario, until: i64) -> Amount {
        let petya = scenario.contact("petya");
        scenario
            .vault
            .contact_balances_until(until)
            .into_iter()
            .filter(|row| row.contact_id == petya)
            .fold(Amount::ZERO, |sum, row| sum + row.receivable - row.payable)
    }

    #[test]
    fn deleting_a_linked_payment_in_the_ledger_removes_the_purchase() {
        let mut scenario = skewers_scenario();
        let link = scenario
            .vault
            .shared_tx_links()
            .into_iter()
            .find(|link| link.link.purchase_id.is_some())
            .unwrap();
        scenario.vault.delete_transaction(&link.tx_id).unwrap();
        let view = scenario.vault.shared_event_view(&scenario.event).unwrap();
        // the meat (4 800) was bought with that payment or the beer was; either way one is gone
        assert!(view.totals.spent < amount("7100"));
        assert!(
            scenario
                .vault
                .shared_tx_links()
                .iter()
                .all(|other| other.tx_id != link.tx_id)
        );
    }

    #[test]
    fn deleting_a_settlement_operation_reopens_the_debt() {
        let mut scenario = skewers_scenario();
        let tx = settle_petya(&mut scenario, "1735", None);
        assert_eq!(petya_debt(&scenario, i64::MAX), amount("0"));
        scenario
            .vault
            .delete_transactions(std::slice::from_ref(&tx))
            .unwrap();
        assert_eq!(petya_debt(&scenario, i64::MAX), amount("1735"));
        let view = scenario.vault.shared_event_view(&scenario.event).unwrap();
        assert!(view.event.settlements.is_empty());
    }

    fn edited(
        scenario: &Scenario,
        tx_id: &str,
        change: impl FnOnce(&mut Transaction),
    ) -> Transaction {
        let mut tx = scenario
            .vault
            .doc
            .transactions
            .iter()
            .find(|tx| tx.meta().id == tx_id)
            .unwrap()
            .clone();
        change(&mut tx);
        tx
    }

    #[test]
    fn editing_the_amount_and_date_of_a_linked_operation_follows_into_the_event() {
        let mut scenario = skewers_scenario();
        let purchase_tx = scenario
            .vault
            .shared_tx_links()
            .into_iter()
            .find(|link| link.link.purchase_id.is_some())
            .unwrap()
            .tx_id;
        let tx = edited(&scenario, &purchase_tx, |tx| {
            if let Transaction::Payment(inner) = tx {
                inner.amount = amount("5000");
                inner.meta.timestamp = "2026-10-04T10:00:00Z".into();
            }
        });
        scenario.vault.update_transaction(&purchase_tx, tx).unwrap();
        let event = scenario.vault.find_shared_event(&scenario.event).unwrap();
        let purchase = event
            .items
            .iter()
            .flat_map(|item| &item.purchases)
            .find(|purchase| purchase.tx_id.as_deref() == Some(purchase_tx.as_str()))
            .unwrap();
        assert_eq!(purchase.total(), amount("5000"));
        assert_eq!(purchase.at, "2026-10-04T10:00:00Z");

        // a settlement edit
        let settle_tx = settle_petya(&mut scenario, "1000", None);
        let tx = edited(&scenario, &settle_tx, |tx| {
            if let Transaction::Transfer(inner) = tx {
                inner.amount = amount("1200");
            }
        });
        scenario.vault.update_transaction(&settle_tx, tx).unwrap();
        let view = scenario.vault.shared_event_view(&scenario.event).unwrap();
        assert_eq!(view.event.settlements[0].amount, amount("1200"));
    }

    #[test]
    fn the_event_refuses_what_it_cannot_follow() {
        let mut scenario = skewers_scenario();
        let purchase_tx = scenario
            .vault
            .shared_tx_links()
            .into_iter()
            .find(|link| link.link.purchase_id.is_some())
            .unwrap()
            .tx_id;
        // another currency
        let tx = edited(&scenario, &purchase_tx, |tx| {
            if let Transaction::Payment(inner) = tx {
                inner.asset = crate::model::store::AssetRef::fiat("USD", "USD");
            }
        });
        assert!(scenario.vault.update_transaction(&purchase_tx, tx).is_err());
        // another kind
        let tx = edited(&scenario, &purchase_tx, |tx| {
            if let Transaction::Payment(inner) = tx {
                *tx = Transaction::Topup(crate::model::transaction::TopupTx {
                    meta: inner.meta.clone(),
                    store_id: inner.store_id.clone(),
                    asset: inner.asset.clone(),
                    amount: inner.amount,
                    fee: None,
                    from: None,
                    details: Default::default(),
                    price: Default::default(),
                });
            }
        });
        assert!(scenario.vault.update_transaction(&purchase_tx, tx).is_err());
        // a settlement above what is owed
        let settle_tx = settle_petya(&mut scenario, "1000", None);
        let tx = edited(&scenario, &settle_tx, |tx| {
            if let Transaction::Transfer(inner) = tx {
                inner.amount = amount("1736");
            }
        });
        assert!(scenario.vault.update_transaction(&settle_tx, tx).is_err());
        // nothing changed
        let view = scenario.vault.shared_event_view(&scenario.event).unwrap();
        assert_eq!(view.event.settlements[0].amount, amount("1000"));
        assert_eq!(view.totals.spent, amount("7100"));
    }

    #[test]
    fn event_debts_are_dated_and_event_transfers_are_never_counted_twice() {
        let mut scenario = skewers_scenario();
        settle_petya(&mut scenario, "1735", Some("2026-10-05"));
        let before = crate::pricing::history::secs_or_now("2026-10-04T00:00:00Z");
        let after = crate::pricing::history::secs_or_now("2026-10-06T00:00:00Z");
        // the purchases are dated 3 October: Petya owes 1 735 on the 4th, nothing on the 6th
        assert_eq!(petya_debt(&scenario, before), amount("1735"));
        assert_eq!(petya_debt(&scenario, after), amount("0"));
        assert_eq!(petya_debt(&scenario, i64::MAX), amount("0"));
    }

    #[test]
    fn deleting_the_event_turns_its_transfers_into_ordinary_debt_transfers() {
        let mut scenario = skewers_scenario();
        settle_petya(&mut scenario, "1735", None);
        scenario
            .vault
            .delete_shared_event(&scenario.event.clone())
            .unwrap();
        // a bare Collect of 1 735 with nothing lent before: the debt goes negative
        assert_eq!(petya_debt(&scenario, i64::MAX), amount("-1735"));
        assert!(scenario.vault.shared_tx_links().is_empty());
    }

    #[test]
    fn editing_an_archived_event_reopens_it() {
        let mut scenario = skewers_scenario();
        let id = scenario.event.clone();
        // settle everything by set-off, then archive
        for (who, raw) in [
            ("petya", "1735"),
            ("vasya", "1685"),
            ("anya", "1060"),
            ("olya", "1060"),
        ] {
            scenario
                .vault
                .settle_shared(SettleSharedInput {
                    event_id: id.clone(),
                    participant_id: who.into(),
                    direction: SettleDirection::ToOrganizer,
                    amount: amount(raw),
                    store_id: None,
                    at: None,
                })
                .unwrap();
        }
        scenario.vault.archive_shared_event(&id, true).unwrap();
        assert!(scenario.vault.find_shared_event(&id).unwrap().archived);
        scenario
            .vault
            .add_shared_item(
                &id,
                crate::ipc_contract::SharedItemDraft {
                    title: "Лук".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(!scenario.vault.find_shared_event(&id).unwrap().archived);
    }

    #[test]
    fn a_participant_with_operations_keeps_their_contact_and_a_lone_sharer_stays() {
        let mut scenario = skewers_scenario();
        let tx = settle_petya(&mut scenario, "100", None);
        assert!(!tx.is_empty());
        let other = scenario.contact("anya");
        let relink = scenario.vault.update_shared_participant(
            &scenario.event.clone(),
            "petya",
            crate::ipc_contract::SharedParticipantDraft {
                contact_id: Some(other),
                ..Default::default()
            },
        );
        assert!(relink.is_err());
        // Olya is the only one sharing the napkins
        let id = scenario.event.clone();
        let napkins = scenario
            .vault
            .add_shared_item(
                &id,
                crate::ipc_contract::SharedItemDraft {
                    title: "Салфетки".into(),
                    ..Default::default()
                },
            )
            .unwrap()
            .event
            .items
            .last()
            .unwrap()
            .id
            .clone();
        scenario
            .vault
            .set_shared_item_split(crate::ipc_contract::SetSharedItemSplitInput {
                event_id: id.clone(),
                item_id: napkins,
                split: crate::model::shared_event::ItemSplit::Equal {
                    among: vec!["olya".into()],
                },
            })
            .unwrap();
        let err = scenario
            .vault
            .remove_shared_participant(&id, "olya")
            .unwrap_err();
        assert!(err.to_string().contains("Салфетки"), "{err}");
    }

    #[test]
    fn fixed_totals_are_checked_when_set_and_flagged_when_they_stop_fitting() {
        let mut scenario = skewers_scenario();
        let id = scenario.event.clone();
        let set = |scenario: &mut Scenario, who: &str, total: Option<&str>| {
            scenario
                .vault
                .set_shared_share_override(SetShareOverrideInput {
                    event_id: id.clone(),
                    participant_id: who.into(),
                    total: total.map(amount),
                })
        };
        assert!(set(&mut scenario, "petya", Some("9000")).is_err());
        assert!(set(&mut scenario, "petya", Some("1000")).is_ok());
        // everybody fixed to a sum that is not what was bought
        for who in [ME, "vasya", "anya"] {
            assert!(set(&mut scenario, who, Some("100")).is_ok());
        }
        assert!(set(&mut scenario, "olya", Some("100")).is_err());
        // a purchase removed later makes the totals not fit
        assert!(set(&mut scenario, "olya", Some("5800")).is_ok());
        let tx = scenario
            .vault
            .shared_tx_links()
            .into_iter()
            .find(|link| link.link.purchase_id.is_some())
            .unwrap()
            .tx_id;
        scenario.vault.delete_transaction(&tx).unwrap();
        let view = scenario.vault.shared_event_view(&id).unwrap();
        assert!(view.overrides_invalid);
    }

    #[test]
    fn passing_a_rest_on_never_lands_on_an_organizer_who_is_out() {
        let mut scenario = skewers_scenario();
        let id = scenario.event.clone();
        scenario
            .vault
            .find_shared_event_mut(&id)
            .unwrap()
            .i_participate = false;
        // the purchases of mine now have no place; only the debt rule matters here
        let rest = scenario.vault.reassign_shared(SharedRemainderInput {
            event_id: id.clone(),
            participant_id: "vasya".into(),
            amount: Some(amount("100")),
            to: vec![ME.into()],
        });
        assert!(rest.is_err());
        let view = scenario
            .vault
            .reassign_shared(SharedRemainderInput {
                event_id: id.clone(),
                participant_id: "vasya".into(),
                amount: Some(amount("100")),
                to: Vec::new(),
            })
            .unwrap();
        assert!(view.event.settlements[0].to.iter().all(|who| who != ME));
    }
}
