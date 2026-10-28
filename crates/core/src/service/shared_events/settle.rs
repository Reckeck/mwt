//! Settling an event: money back, set-offs, write-offs, passing a rest on to
//! the others. Everything goes through the organizer. Money of mine becomes a
//! debt transfer (`Collect` when I get it back, `Repay` when I give it) linked
//! to the event; the debt itself is never stored, it is the event's balance.

use crate::Amount;
use crate::error::{Error, Result};
use crate::ipc_contract::{SetSharedDueInput, SettleSharedInput, SharedRemainderInput};
use crate::model::contact::DebtTerms;
use crate::model::shared_event::{ME, SettleDirection, Settlement, SettlementKind, SharedEvent};
use crate::model::store::AssetRef;
use crate::model::transaction::{
    AccountRef, DebtEffect, PriceStamp, Transaction, TransferTx, TxMeta,
};
use crate::model::vault::now_iso;

use super::super::UnlockedVault;
use super::super::contacts::DebtTermsDraft;
use super::purchases::normalize_moment;
use super::split::{self, Split};
use super::view::SharedEventView;

fn tolerance() -> Amount {
    Amount::new(rust_decimal::Decimal::new(5, 3))
}

fn moment(raw: Option<&str>) -> Result<String> {
    match raw.map(str::trim).filter(|raw| !raw.is_empty()) {
        Some(raw) => normalize_moment(raw),
        None => Ok(now_iso()),
    }
}

/// The person, checked: me (when I take part) or a participant, and not the organizer.
fn check_settling_person(event: &SharedEvent, computed: &Split, id: &str) -> Result<()> {
    if id == event.organizer {
        return Err(Error::InvalidInput(
            "the organizer settles with the others, not with themselves".into(),
        ));
    }
    if computed.person(id).is_none() {
        return Err(Error::Missing(format!("participant {id}")));
    }
    Ok(())
}

impl UnlockedVault {
    /// The debt transfer for money of mine moving between me and the other
    /// side of the settlement.
    fn settlement_transfer(
        &self,
        event: &SharedEvent,
        settlement: &Settlement,
        store_id: &str,
    ) -> Result<Transaction> {
        if !event.i_participate {
            return Err(Error::InvalidInput(
                "you do not take part in this event: settle without an operation".into(),
            ));
        }
        let involves_me = event.organizer == ME || settlement.participant_id == ME;
        if !involves_me {
            return Err(Error::InvalidInput(
                "a settlement between other people is a set-off without an operation".into(),
            ));
        }
        let other = if event.organizer == ME {
            settlement.participant_id.as_str()
        } else {
            event.organizer.as_str()
        };
        let contact_id = event
            .participants
            .iter()
            .find(|person| person.id == other)
            .and_then(|person| person.contact_id.clone())
            .ok_or_else(|| {
                Error::InvalidInput(
                    "a guest has no contact: settle without an operation, or make them a contact"
                        .into(),
                )
            })?;
        // The money comes to me when the other side pays me.
        let receive =
            (event.organizer == ME) == (settlement.direction == SettleDirection::ToOrganizer);
        let wallet = AccountRef::Store(store_id.to_string());
        let contact = AccountRef::Contact {
            contact_id,
            account_id: None,
        };
        let (effect, from, to) = if receive {
            (DebtEffect::Collect, contact, wallet)
        } else {
            (DebtEffect::Repay, wallet, contact)
        };
        let mut meta = TxMeta::new(&event.title);
        meta.timestamp = settlement.at.clone();
        let code = event.currency.to_uppercase();
        Ok(Transaction::Transfer(TransferTx {
            meta,
            from,
            to,
            asset: AssetRef::fiat(&code, &code),
            amount: settlement.amount,
            fee: None,
            network: None,
            tx_hash: None,
            debt: Some(effect),
            price: PriceStamp::default(),
        }))
    }

    /// Money between a participant and the organizer (or a set-off).
    pub fn settle_shared(&mut self, input: SettleSharedInput) -> Result<SharedEventView> {
        let event = self.find_shared_event(&input.event_id)?.clone();
        let computed = split::compute(&event);
        check_settling_person(&event, &computed, &input.participant_id)?;
        if !input.amount.is_positive() {
            return Err(Error::InvalidInput("the amount must be positive".into()));
        }
        let saldo = computed
            .person(&input.participant_id)
            .map_or(Amount::ZERO, |row| row.saldo);
        let allowed = match input.direction {
            SettleDirection::ToOrganizer => saldo,
            SettleDirection::FromOrganizer => -saldo,
        };
        if input.amount > allowed + tolerance() {
            return Err(Error::InvalidInput(
                "this is more than is owed in that direction".into(),
            ));
        }
        let mut settlement = Settlement {
            id: uuid::Uuid::new_v4().to_string(),
            kind: SettlementKind::Offset,
            participant_id: input.participant_id.clone(),
            direction: input.direction,
            amount: input.amount,
            at: moment(input.at.as_deref())?,
            tx_id: None,
            to: Vec::new(),
        };
        if let Some(store_id) = input
            .store_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
        {
            let transfer = self.settlement_transfer(&event, &settlement, store_id)?;
            let saved = self.add_transaction(transfer)?;
            settlement.kind = SettlementKind::Transfer;
            settlement.tx_id = Some(saved.meta().id.clone());
        }
        self.find_shared_event_mut(&input.event_id)?
            .settlements
            .push(settlement);
        self.persist()?;
        self.shared_event_view(&input.event_id)
    }

    /// The rest of what someone owes the organizer, checked, and how much of it to move.
    fn open_rest(
        event: &SharedEvent,
        participant_id: &str,
        amount: Option<Amount>,
    ) -> Result<Amount> {
        let computed = split::compute(event);
        check_settling_person(event, &computed, participant_id)?;
        let saldo = computed
            .person(participant_id)
            .map_or(Amount::ZERO, |row| row.saldo);
        if !saldo.is_positive() {
            return Err(Error::InvalidInput("nothing is owed here".into()));
        }
        let amount = amount.unwrap_or(saldo);
        if !amount.is_positive() || amount > saldo + tolerance() {
            return Err(Error::InvalidInput(
                "the amount must be positive and at most what is owed".into(),
            ));
        }
        Ok(amount.min(saldo))
    }

    /// «Списать остаток»: I give up what a participant owes me; it becomes my
    /// expense in the category of the event.
    pub fn write_off_shared(&mut self, input: SharedRemainderInput) -> Result<SharedEventView> {
        let event = self.find_shared_event(&input.event_id)?;
        if event.organizer != ME || !event.i_participate {
            return Err(Error::InvalidInput(
                "only the organizer can write a debt off, and only when they take part".into(),
            ));
        }
        let amount = Self::open_rest(event, &input.participant_id, input.amount)?;
        self.find_shared_event_mut(&input.event_id)?
            .settlements
            .push(Settlement {
                id: uuid::Uuid::new_v4().to_string(),
                kind: SettlementKind::WriteOff,
                participant_id: input.participant_id,
                direction: SettleDirection::ToOrganizer,
                amount,
                at: now_iso(),
                tx_id: None,
                to: Vec::new(),
            });
        self.persist()?;
        self.shared_event_view(&input.event_id)
    }

    /// «Переложить на других»: what a participant has not returned is split
    /// between the others.
    pub fn reassign_shared(&mut self, input: SharedRemainderInput) -> Result<SharedEventView> {
        let event = self.find_shared_event(&input.event_id)?;
        let amount = Self::open_rest(event, &input.participant_id, input.amount)?;
        let people = split::people_ids(event);
        let targets: Vec<String> = if input.to.is_empty() {
            people
                .into_iter()
                .filter(|id| *id != input.participant_id)
                .collect()
        } else {
            if let Some(unknown) = input.to.iter().find(|id| !people.contains(id)) {
                return Err(Error::Missing(format!("participant {unknown}")));
            }
            input
                .to
                .iter()
                .filter(|id| **id != input.participant_id)
                .cloned()
                .collect()
        };
        if targets.is_empty() {
            return Err(Error::InvalidInput("there is nobody to put it on".into()));
        }
        self.find_shared_event_mut(&input.event_id)?
            .settlements
            .push(Settlement {
                id: uuid::Uuid::new_v4().to_string(),
                kind: SettlementKind::Reassign,
                participant_id: input.participant_id,
                direction: SettleDirection::ToOrganizer,
                amount,
                at: now_iso(),
                tx_id: None,
                to: targets,
            });
        self.persist()?;
        self.shared_event_view(&input.event_id)
    }

    /// Take a settlement back, with the operation it made.
    pub fn delete_shared_settlement(
        &mut self,
        event_id: &str,
        settlement_id: &str,
    ) -> Result<SharedEventView> {
        let event = self.find_shared_event_mut(event_id)?;
        let index = event
            .settlements
            .iter()
            .position(|settlement| settlement.id == settlement_id)
            .ok_or_else(|| Error::Missing(format!("settlement {settlement_id}")))?;
        let removed = event.settlements.remove(index);
        if let Some(tx_id) = &removed.tx_id {
            self.drop_shared_tx(tx_id);
            self.recompute_holdings();
        }
        self.persist()?;
        self.shared_event_view(event_id)
    }

    /// The due date of a contact participant's debt (the existing debt terms,
    /// keyed by contact and currency); an empty date clears it.
    pub fn set_shared_due(&mut self, input: SetSharedDueInput) -> Result<Option<DebtTerms>> {
        let event = self.find_shared_event(&input.event_id)?;
        let contact_id = event
            .participants
            .iter()
            .find(|person| person.id == input.participant_id)
            .and_then(|person| person.contact_id.clone())
            .ok_or_else(|| {
                Error::InvalidInput("a term needs a participant who is a contact".into())
            })?;
        let asset = event.currency.to_uppercase();
        let note = self
            .doc
            .debt_terms
            .iter()
            .find(|terms| terms.contact_id == contact_id && terms.asset == asset)
            .map(|terms| terms.note.clone())
            .unwrap_or_default();
        self.set_debt_terms(DebtTermsDraft {
            contact_id,
            asset,
            due_at: input.due_at,
            note,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::*;
    use crate::ipc_contract::{SharedEventDraft, SharedParticipantDraft, SharedPurchaseDraft};
    use crate::model::shared_event::{PriceMode, SharedEventStatus};

    fn settle(
        scenario: &Scenario,
        who: &str,
        direction: SettleDirection,
        raw: &str,
        store: Option<&str>,
    ) -> SettleSharedInput {
        SettleSharedInput {
            event_id: scenario.event.clone(),
            participant_id: who.into(),
            direction,
            amount: amount(raw),
            store_id: store.map(str::to_string),
            at: None,
        }
    }

    /// The debt transfer an event settlement made.
    fn last_transfer(scenario: &Scenario) -> TransferTx {
        let linked: Vec<String> = scenario
            .vault
            .shared_tx_links()
            .into_iter()
            .filter(|link| link.link.settlement_id.is_some())
            .map(|link| link.tx_id)
            .collect();
        scenario
            .vault
            .doc
            .transactions
            .iter()
            .find_map(|tx| match tx {
                Transaction::Transfer(inner) if linked.contains(&inner.meta.id) => {
                    Some(inner.clone())
                }
                _ => None,
            })
            .unwrap()
    }

    fn debt_with(scenario: &Scenario, contact: &str) -> Amount {
        scenario
            .vault
            .contact_balances()
            .into_iter()
            .filter(|row| row.contact_id == contact)
            .fold(Amount::ZERO, |sum, row| sum + row.receivable - row.payable)
    }

    #[test]
    fn debts_appear_at_once_in_contact_cards_and_go_out_when_returned() {
        let mut scenario = skewers_scenario();
        let petya = scenario.contact("petya");
        let vasya = scenario.contact("vasya");
        assert_eq!(debt_with(&scenario, &petya), amount("1735"));
        assert_eq!(debt_with(&scenario, &vasya), amount("1685"));
        // the guest is nowhere in the contacts
        assert_eq!(
            scenario.vault.contact_balances().len(),
            3,
            "Petya, Vasya and Anya only"
        );
        let cards = scenario.vault.contact_shared_events(&petya);
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].amount, amount("1735"));

        let person = scenario.person("petya");
        let wallet = scenario.wallet.clone();
        let view = scenario
            .vault
            .settle_shared(settle(
                &scenario,
                &person,
                SettleDirection::ToOrganizer,
                "1735",
                Some(&wallet),
            ))
            .unwrap();
        assert_eq!(view.status, SharedEventStatus::Settling);
        assert_eq!(debt_with(&scenario, &petya), amount("0"));
        assert!(scenario.vault.contact_shared_events(&petya).is_empty());
        // the money is in my wallet, and the operation is a Collect from Petya
        assert_eq!(scenario.balance(), amount("43400") + amount("1735"));
        let transfer = last_transfer(&scenario);
        assert_eq!(transfer.debt, Some(DebtEffect::Collect));
        assert_eq!(transfer.amount, amount("1735"));
    }

    #[test]
    fn a_partial_return_leaves_the_rest_and_too_much_is_refused() {
        let mut scenario = skewers_scenario();
        let vasya = scenario.person("vasya");
        let wallet = scenario.wallet.clone();
        scenario
            .vault
            .settle_shared(settle(
                &scenario,
                &vasya,
                SettleDirection::ToOrganizer,
                "1000",
                Some(&wallet),
            ))
            .unwrap();
        assert_eq!(
            debt_with(&scenario, &scenario.contact("vasya")),
            amount("685")
        );
        let too_much = scenario.vault.settle_shared(settle(
            &scenario,
            &vasya,
            SettleDirection::ToOrganizer,
            "686",
            Some(&wallet),
        ));
        assert!(too_much.is_err());
        // giving money to someone who owes me makes no sense either
        let backwards = scenario.vault.settle_shared(settle(
            &scenario,
            &vasya,
            SettleDirection::FromOrganizer,
            "10",
            None,
        ));
        assert!(backwards.is_err());
    }

    #[test]
    fn a_set_off_changes_the_balance_without_an_operation() {
        let mut scenario = skewers_scenario();
        let anya = scenario.person("anya");
        let before = scenario.vault.doc.transactions.len();
        scenario
            .vault
            .settle_shared(settle(
                &scenario,
                &anya,
                SettleDirection::ToOrganizer,
                "1060",
                None,
            ))
            .unwrap();
        assert_eq!(scenario.vault.doc.transactions.len(), before);
        assert_eq!(debt_with(&scenario, &scenario.contact("anya")), amount("0"));
    }

    #[test]
    fn a_guest_settles_without_an_operation_only() {
        let mut scenario = skewers_scenario();
        let olya = scenario.person("olya");
        let wallet = scenario.wallet.clone();
        let refused = scenario.vault.settle_shared(settle(
            &scenario,
            &olya,
            SettleDirection::ToOrganizer,
            "1060",
            Some(&wallet),
        ));
        assert!(refused.is_err());
        let view = scenario
            .vault
            .settle_shared(settle(
                &scenario,
                &olya,
                SettleDirection::ToOrganizer,
                "1060",
                None,
            ))
            .unwrap();
        let guest = view.people.iter().find(|row| row.id == olya).unwrap();
        assert!(guest.saldo.is_zero());
    }

    #[test]
    fn writing_off_the_rest_closes_the_debt_and_is_my_position() {
        let mut scenario = skewers_scenario();
        let vasya = scenario.person("vasya");
        let wallet = scenario.wallet.clone();
        scenario
            .vault
            .settle_shared(settle(
                &scenario,
                &vasya,
                SettleDirection::ToOrganizer,
                "1000",
                Some(&wallet),
            ))
            .unwrap();
        let rest = |amount: Option<Amount>| SharedRemainderInput {
            event_id: scenario.event.clone(),
            participant_id: vasya.clone(),
            amount,
            to: Vec::new(),
        };
        let view = scenario.vault.write_off_shared(rest(None)).unwrap();
        assert_eq!(
            debt_with(&scenario, &scenario.contact("vasya")),
            amount("0")
        );
        let mine = view.mine.unwrap();
        assert_eq!(mine.written_off, amount("685"));
        // nothing to write off any more
        assert!(scenario.vault.write_off_shared(rest(None)).is_err());
    }

    #[test]
    fn putting_the_rest_on_the_others_shifts_their_shares() {
        let mut scenario = skewers_scenario();
        let vasya = scenario.person("vasya");
        let wallet = scenario.wallet.clone();
        scenario
            .vault
            .settle_shared(settle(
                &scenario,
                &vasya,
                SettleDirection::ToOrganizer,
                "1000",
                Some(&wallet),
            ))
            .unwrap();
        let view = scenario
            .vault
            .reassign_shared(SharedRemainderInput {
                event_id: scenario.event.clone(),
                participant_id: vasya.clone(),
                amount: None,
                to: Vec::new(),
            })
            .unwrap();
        // 685 over Me, Petya, Anya, Olya: 171.25 each
        let row = |id: &str| view.people.iter().find(|row| row.id == id).unwrap();
        assert_eq!(row(&vasya).share, amount("1500"));
        assert!(row(&vasya).saldo.is_zero());
        assert_eq!(row(ME).share, amount("1231.25"));
        assert_eq!(
            debt_with(&scenario, &scenario.contact("petya")),
            amount("1906.25")
        );
        let total = view
            .people
            .iter()
            .fold(Amount::ZERO, |sum, row| sum + row.share);
        assert_eq!(total, amount("7100"));
    }

    #[test]
    fn a_settlement_can_be_taken_back_with_its_operation() {
        let mut scenario = skewers_scenario();
        let petya = scenario.person("petya");
        let wallet = scenario.wallet.clone();
        let view = scenario
            .vault
            .settle_shared(settle(
                &scenario,
                &petya,
                SettleDirection::ToOrganizer,
                "500",
                Some(&wallet),
            ))
            .unwrap();
        let settlement = view.event.settlements[0].clone();
        let balance = scenario.balance();
        scenario
            .vault
            .delete_shared_settlement(&scenario.event, &settlement.id)
            .unwrap();
        assert_eq!(scenario.balance(), balance - amount("500"));
        assert_eq!(
            debt_with(&scenario, &scenario.contact("petya")),
            amount("1735")
        );
    }

    #[test]
    fn when_someone_else_organizes_one_repay_settles_my_share() {
        let mut scenario = gift_scenario();
        let anya = scenario.contact("anya");
        // I owe Anya my 1850: a negative debt of hers
        assert_eq!(debt_with(&scenario, &anya), amount("-1850"));
        let view = scenario.vault.shared_event_view(&scenario.event).unwrap();
        let mine = view.mine.clone().unwrap();
        assert_eq!(mine.i_owe, amount("1850"));
        assert_eq!(
            mine.pay_to.as_deref(),
            Some(scenario.person("anya").as_str())
        );
        let wallet = scenario.wallet.clone();
        let transfers = |scenario: &Scenario| {
            scenario
                .vault
                .doc
                .transactions
                .iter()
                .filter(|tx| matches!(tx, Transaction::Transfer(_)))
                .count()
        };
        let before = transfers(&scenario);
        let view = scenario
            .vault
            .settle_shared(settle(
                &scenario,
                ME,
                SettleDirection::ToOrganizer,
                "1850",
                Some(&wallet),
            ))
            .unwrap();
        assert_eq!(transfers(&scenario), before + 1);
        let repay = last_transfer(&scenario);
        assert_eq!(repay.debt, Some(DebtEffect::Repay));
        assert_eq!(repay.amount, amount("1850"));
        assert_eq!(debt_with(&scenario, &anya), amount("0"));
        assert!(view.mine.unwrap().i_owe.is_zero());
    }

    #[test]
    fn a_calculator_for_others_touches_no_wallet_and_no_contact() {
        let mut scenario = gift_scenario();
        let draft = SharedEventDraft {
            title: "Для друзей".into(),
            currency: "rub".into(),
            i_participate: false,
            organizer: "pa".into(),
            participants: vec![
                SharedParticipantDraft {
                    id: Some("pa".into()),
                    contact_id: Some(scenario.contact("anya")),
                    ..Default::default()
                },
                SharedParticipantDraft {
                    id: Some("pb".into()),
                    contact_id: Some(scenario.contact("petya")),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let id = scenario.vault.create_shared_event(draft).unwrap().event.id;
        let cake = |buyer: &str| SharedPurchaseDraft {
            new_item_title: Some("Торт".into()),
            buyer: buyer.into(),
            qty: amount("1"),
            amount: amount("900"),
            price_mode: PriceMode::Total,
            store_id: Some(scenario.wallet.clone()),
            ..Default::default()
        };
        // I cannot buy for it, and it creates no debts with my contacts
        assert!(scenario.vault.add_shared_purchase(&id, cake("")).is_err());
        let view = scenario.vault.add_shared_purchase(&id, cake("pa")).unwrap();
        assert!(view.mine.is_none());
        assert_eq!(
            view.people.iter().find(|row| row.id == "pb").unwrap().saldo,
            amount("450")
        );
        let balance = scenario.balance();
        let money = SettleSharedInput {
            event_id: id.clone(),
            participant_id: "pb".into(),
            direction: SettleDirection::ToOrganizer,
            amount: amount("450"),
            store_id: Some(scenario.wallet.clone()),
            at: None,
        };
        // settling is a set-off only
        assert!(scenario.vault.settle_shared(money.clone()).is_err());
        scenario
            .vault
            .settle_shared(SettleSharedInput {
                store_id: None,
                ..money
            })
            .unwrap();
        assert_eq!(scenario.balance(), balance);
        let petya = scenario.contact("petya");
        assert!(
            scenario
                .vault
                .contact_shared_events(&petya)
                .iter()
                .all(|row| row.event_id != id)
        );
    }

    #[test]
    fn a_term_can_be_set_for_a_contact_debt() {
        let mut scenario = skewers_scenario();
        let petya = scenario.person("petya");
        let terms = scenario
            .vault
            .set_shared_due(SetSharedDueInput {
                event_id: scenario.event.clone(),
                participant_id: petya.clone(),
                due_at: Some("2026-12-01".into()),
            })
            .unwrap()
            .unwrap();
        assert_eq!(terms.asset, "RUB");
        assert_eq!(terms.due_at.as_deref(), Some("2026-12-01"));
        let view = scenario.vault.shared_event_view(&scenario.event).unwrap();
        let row = view.people.iter().find(|row| row.id == petya).unwrap();
        assert_eq!(row.due_at.as_deref(), Some("2026-12-01"));
        // a guest has no contact to hang a term on
        let olya = scenario.person("olya");
        assert!(
            scenario
                .vault
                .set_shared_due(SetSharedDueInput {
                    event_id: scenario.event.clone(),
                    participant_id: olya,
                    due_at: Some("2026-12-01".into()),
                })
                .is_err()
        );
    }

    /// The risk register asks for a bench on a big vault: 500 events with
    /// debts must not slow the contact balances noticeably.
    #[test]
    fn five_hundred_events_keep_the_contact_balances_fast() {
        let mut scenario = skewers_scenario();
        let template = scenario.vault.doc.shared_events[0].clone();
        for index in 0..500 {
            let mut copy = template.clone();
            copy.id = format!("bulk-{index}");
            for item in &mut copy.items {
                for purchase in &mut item.purchases {
                    purchase.tx_id = None;
                    purchase.tx_owned = false;
                }
            }
            scenario.vault.doc.shared_events.push(copy);
        }
        let started = std::time::Instant::now();
        let balances = scenario.vault.contact_balances();
        let elapsed = started.elapsed();
        assert_eq!(balances.len(), 3);
        // 501 events x 1 735 for Petya
        let petya = balances
            .iter()
            .find(|row| row.contact_id == scenario.contact("petya"))
            .unwrap();
        assert_eq!(petya.receivable, amount("1735") * amount("501"));
        assert!(
            elapsed < std::time::Duration::from_secs(2),
            "took {elapsed:?}"
        );
    }

    #[test]
    fn the_organizer_cannot_settle_with_themselves() {
        let mut scenario = skewers_scenario();
        let refused = scenario.vault.settle_shared(settle(
            &scenario,
            ME,
            SettleDirection::ToOrganizer,
            "10",
            None,
        ));
        assert!(refused.is_err());
    }
}
