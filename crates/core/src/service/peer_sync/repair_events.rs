//! Stage M, rules of `repair_invariants` for shared events (catalog 116, 123).
//!
//! - A guest who became a contact on two devices, or on an older build, has
//!   one contact: the deterministic `guest-…` id wins, what pointed to the
//!   other one moves over and the other one goes.
//! - R6: an operation is linked to one event only, once. Of several links the
//!   one whose item was stamped first stays (equal stamps: the smaller event
//!   id, then item id); the others are removed. The operation is untouched.
//! - R7a: purchases the app converted itself (not typed rates) at rate 1 name
//!   the currency the event was settled in. When they agree on one currency
//!   that is not the event's, the event goes back to it; otherwise nothing
//!   changes and the purchases are reported as needing a rate.
//! - R7b: settlements record the organizer. When they agree on one organizer
//!   that is not the event's, the event goes back to it. Settlements of older
//!   builds have no organizer: they are only detected.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use super::UnlockedVault;
use super::repair::RepairReport;
use crate::model::contact::Contact;
use crate::model::shared_event::ME;
use crate::model::transaction::AccountRef;
use crate::service::sync_ids;
use crate::sync::flatten;
use crate::sync::registry;
use crate::sync::{Stamp, state::RecordStamps};

/// One link purchase of an operation inside an event.
struct Link {
    tx_id: String,
    stamp: Stamp,
    event_id: String,
    item_id: String,
    item_index: usize,
    purchase_index: usize,
}

impl UnlockedVault {
    // -- 123a: guests that became contacts -------------------------------------

    /// A participant whose contact is not the deterministic one, while the
    /// deterministic contact exists and the other one carries the guest's name
    /// and is used by nobody else: the other contact folds into the
    /// deterministic one.
    pub(super) fn repair_guest_contacts(&mut self, report: &mut RepairReport) {
        let mut uses: BTreeMap<&str, usize> = BTreeMap::new();
        for person in self
            .doc
            .shared_events
            .iter()
            .flat_map(|event| &event.participants)
        {
            if let Some(contact) = person.contact_id.as_deref() {
                *uses.entry(contact).or_default() += 1;
            }
        }
        let mut folds: BTreeSet<(String, String)> = BTreeSet::new();
        for event in &self.doc.shared_events {
            for person in &event.participants {
                let Some(current) = person.contact_id.as_deref() else {
                    continue;
                };
                let wanted = sync_ids::guest_contact_id(&event.id, &person.id);
                if current == wanted || uses.get(current) != Some(&1) {
                    continue;
                }
                let named_alike = self.doc.contacts.iter().any(|contact| {
                    contact.id == current && contact.name.trim() == person.name.trim()
                });
                if named_alike && self.doc.contacts.iter().any(|contact| contact.id == wanted) {
                    folds.insert((current.to_string(), wanted));
                }
            }
        }
        for (loser, winner) in folds {
            self.fold_contact(&loser, &winner);
            self.repoint_contact(&loser, &winner);
            self.doc.contacts.retain(|contact| contact.id != loser);
            report.merged_duplicates.push(loser);
        }
    }

    /// What the loser changed later than the winner moves into the winner; the
    /// accounts of both stay (an operation may name an account by its id).
    fn fold_contact(&mut self, loser: &str, winner: &str) {
        let stamps_of = |id: &str| -> RecordStamps {
            self.doc
                .sync
                .stamps
                .get(&format!("contacts:{id}"))
                .cloned()
                .unwrap_or_default()
        };
        let (loser_stamps, winner_stamps) = (stamps_of(loser), stamps_of(winner));
        let find = |id: &str| self.doc.contacts.iter().find(|contact| contact.id == id);
        let (Some(from), Some(into)) = (find(loser), find(winner)) else {
            return;
        };
        let (Ok(Value::Object(other)), Ok(Value::Object(mut kept))) =
            (serde_json::to_value(from), serde_json::to_value(into))
        else {
            return;
        };
        for (name, value) in other {
            if name != "id" && loser_stamps.field(&name) > winner_stamps.field(&name) {
                kept.insert(name, value);
            }
        }
        let Ok(mut merged) = serde_json::from_value::<Contact>(Value::Object(kept)) else {
            return;
        };
        for account in &from.accounts {
            if !merged.accounts.iter().any(|known| known.id == account.id) {
                merged.accounts.push(account.clone());
            }
        }
        for account in &into.accounts {
            if !merged.accounts.iter().any(|known| known.id == account.id) {
                merged.accounts.push(account.clone());
            }
        }
        if let Some(target) = self
            .doc
            .contacts
            .iter_mut()
            .find(|contact| contact.id == winner)
        {
            *target = merged;
        }
    }

    /// Every reference to the contact `old` points to `new`, field by field.
    fn repoint_contact(&mut self, old: &str, new: &str) {
        let swap = |field: &mut Option<String>| {
            if field.as_deref() == Some(old) {
                *field = Some(new.to_string());
            }
        };
        for tx in &mut self.doc.transactions {
            swap(&mut tx.meta_mut().contact_id);
            for account in tx.accounts_mut() {
                if let AccountRef::Contact { contact_id, .. } = account
                    && contact_id == old
                {
                    *contact_id = new.to_string();
                }
            }
        }
        for node in &mut self.doc.stores {
            swap(&mut node.contact_id);
        }
        for terms in &mut self.doc.debt_terms {
            if terms.contact_id == old {
                terms.contact_id = new.to_string();
            }
        }
        for event in &mut self.doc.shared_events {
            for person in &mut event.participants {
                swap(&mut person.contact_id);
            }
        }
    }

    // -- 123b and R6: one link per operation -----------------------------------

    pub(super) fn repair_event_links(&mut self, report: &mut RepairReport) {
        let collection = registry::find("shared_events");
        let mut links: Vec<Link> = Vec::new();
        for event in &self.doc.shared_events {
            let stamps = self
                .doc
                .sync
                .stamps
                .get(&format!("shared_events:{}", event.id));
            for (item_index, item) in event.items.iter().enumerate() {
                for (purchase_index, purchase) in item.purchases.iter().enumerate() {
                    let Some(tx_id) = purchase.tx_id.as_ref().filter(|_| !purchase.tx_owned) else {
                        continue;
                    };
                    let stamp = match (collection, stamps) {
                        (Some(collection), Some(stamps)) => flatten::entry_stamp(
                            collection,
                            stamps,
                            &format!("items.{}.purchases.{}", item.id, purchase.id),
                        )
                        .clone(),
                        _ => Stamp::default(),
                    };
                    links.push(Link {
                        tx_id: tx_id.clone(),
                        stamp,
                        event_id: event.id.clone(),
                        item_id: item.id.clone(),
                        item_index,
                        purchase_index,
                    });
                }
            }
        }
        let mut by_tx: BTreeMap<String, Vec<Link>> = BTreeMap::new();
        for link in links {
            by_tx.entry(link.tx_id.clone()).or_default().push(link);
        }
        // (event id, item index, purchase index) of the links to remove.
        let mut doomed: Vec<(String, usize, usize)> = Vec::new();
        for (tx_id, mut group) in by_tx.into_iter().filter(|(_, group)| group.len() > 1) {
            group.sort_by(|left, right| {
                (
                    &left.stamp,
                    &left.event_id,
                    &left.item_id,
                    left.purchase_index,
                )
                    .cmp(&(
                        &right.stamp,
                        &right.event_id,
                        &right.item_id,
                        right.purchase_index,
                    ))
            });
            let events: BTreeSet<&str> = group.iter().map(|link| link.event_id.as_str()).collect();
            if events.len() > 1 {
                report.unlinked_extra.push(tx_id);
            } else {
                report.merged_links.push(tx_id);
            }
            doomed.extend(
                group
                    .into_iter()
                    .skip(1)
                    .map(|link| (link.event_id, link.item_index, link.purchase_index)),
            );
        }
        // Highest index first, so the ones still to remove keep their place.
        doomed.sort_by(|left, right| right.cmp(left));
        for (event_id, item_index, purchase_index) in doomed {
            if let Some(event) = self
                .doc
                .shared_events
                .iter_mut()
                .find(|event| event.id == event_id)
            {
                event.items[item_index].purchases.remove(purchase_index);
            }
        }
    }

    // -- R7: currency and organizer ---------------------------------------------

    pub(super) fn repair_event_money(&mut self, report: &mut RepairReport) {
        for event in &mut self.doc.shared_events {
            let auto: Vec<(&str, &str)> = event
                .items
                .iter()
                .flat_map(|item| &item.purchases)
                .filter(|purchase| purchase.has_auto_rate_one())
                .map(|purchase| (purchase.id.as_str(), purchase.currency.as_str()))
                .collect();
            let currencies: BTreeSet<&str> = auto.iter().map(|(_, currency)| *currency).collect();
            if currencies.len() == 1 {
                let only = currencies
                    .iter()
                    .next()
                    .copied()
                    .unwrap_or_default()
                    .to_string();
                if only != event.currency {
                    event.currency = only;
                    report
                        .restored_currency
                        .push((event.id.clone(), event.title.clone()));
                }
            } else if currencies.len() > 1 {
                report.needs_rate.extend(
                    event
                        .items
                        .iter()
                        .flat_map(|item| &item.purchases)
                        .filter(|purchase| purchase.needs_rate(&event.currency))
                        .map(|purchase| purchase.id.clone()),
                );
            }
            let organizers: BTreeSet<&str> = event
                .settlements
                .iter()
                .filter_map(|settlement| settlement.organizer.as_deref())
                .collect();
            if organizers.len() == 1 {
                let only = organizers
                    .iter()
                    .next()
                    .copied()
                    .unwrap_or_default()
                    .to_string();
                let known = only == ME || event.participants.iter().any(|person| person.id == only);
                if known && only != event.organizer {
                    event.organizer = only;
                    report
                        .restored_organizer
                        .push((event.id.clone(), event.title.clone()));
                }
            }
            if event.settlements.iter().any(|settlement| {
                settlement.organizer.is_none() && settlement.participant_id == event.organizer
            }) {
                report.organizer_flags.push(event.id.clone());
            }
        }
    }
}
