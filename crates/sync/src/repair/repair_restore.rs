//! Stage M, rule R3 of `repair_invariants`: a deleted record that live money
//! still points to comes back.
//!
//! The delete guards (store, category, contact, shared-event participant) look
//! at one device only. A deletes, B meanwhile books an operation, a purchase or
//! a settlement that uses the record: after the merge the money names a record
//! that is gone. Deletion loses to a live money reference (user decision of
//! stage M): the record is put back AS IT WAS (same id, same fields, not
//! archived), stamped fresh as a local edit, so it beats the tombstone on every
//! device through `resolve_deletion`, and the user is told in the notice.
//!
//! The value comes from the batch history, and only from the batch that
//! brought the deletion here (`deleted_value`): its `before` is the last live
//! value this device had. A device that deleted the record itself, or whose
//! batches were pruned, restores nothing and invents nothing: the id goes to
//! the report, and the device that received the deletion in a batch restores
//! the latest value; the restore reaches this one by sync.
//!
//! Money references looked at: payments named by a shared purchase or
//! settlement (`tx_id`), operations (stores, categories, contacts),
//! envelopes and valuations (store), debt terms and event guests (contact), and
//! in a shared event the organizer, buyers, settlements, hand-set totals and
//! splits (participant). Participants are not records of their own: they come
//! back into the event from its previous value.
//!
//! Order: first of the rules. Natural keys, R1 and R2 all assume references
//! resolve, and a restored store may need R2 (its parent can be gone too, then
//! it becomes a wallet of its own) or R1.

use std::collections::{BTreeMap, BTreeSet};

use serde::de::DeserializeOwned;

use super::repair_report::RepairReport;
use mwt_model::Category;
use mwt_model::Contact;
use mwt_model::Store;
use mwt_model::Transaction;
use mwt_model::VaultDoc;
use mwt_model::{ItemSplit, ME, Participant, SharedEvent};

/// The last live value this device had of a record that is gone: the
/// `before` of the batch that brought the deletion here, i.e. the entry
/// whose `after.tombstone` is the record's current tombstone. A deletion
/// made on this device leaves no such entry, and an older entry would hold
/// a value from before later edits (restoring it with a fresh stamp would
/// beat the right value everywhere), so then there is nothing to restore.
pub fn deleted_value<Record: DeserializeOwned>(doc: &VaultDoc, key: &str) -> Option<Record> {
    let tombstone = doc.sync.tombstones.get(key)?;
    doc.sync
        .batches
        .iter()
        .rev()
        .flat_map(|batch| batch.entries.iter())
        .filter(|entry| entry.key == key && entry.after.tombstone.as_ref() == Some(tombstone))
        .find_map(|entry| entry.before.value.clone())
        .and_then(|value| serde_json::from_value(value).ok())
}

/// The event as it was before the batch that removed the participant: the
/// newest entry of the event whose `before` holds the person and whose
/// stamp of the field `participants.<id>` (a removal stamps the field of
/// the entry that went) is the one the event has now. A local removal
/// restamps the field, so it matches no entry.
fn removed_participant(
    doc: &VaultDoc,
    event_id: &str,
    participant_id: &str,
) -> Option<Participant> {
    let key = format!("shared_events:{event_id}");
    let field = format!("participants.{participant_id}");
    let current = doc.sync.stamps.get(&key)?.fields.get(&field)?;
    doc.sync
        .batches
        .iter()
        .rev()
        .flat_map(|batch| batch.entries.iter())
        .filter(|entry| {
            entry.key == key
                && entry
                    .after
                    .stamps
                    .as_ref()
                    .and_then(|stamps| stamps.fields.get(&field))
                    == Some(current)
        })
        .filter_map(|entry| entry.before.value.clone())
        .filter_map(|value| serde_json::from_value::<SharedEvent>(value).ok())
        .find_map(|event| {
            event
                .participants
                .into_iter()
                .find(|person| person.id == participant_id)
        })
}

// -- parts that left with a payment ------------------------------------------

/// A payment is back after its deletion (a newer edit won): what left the
/// event with it returns, unless the payment found another place meanwhile
/// or the item is gone. Every device does this from the same record, so
/// they arrive at the same event. Once a payment is gone for good (its
/// tombstone is too) the record has nothing left to wait for.
pub fn restore_removed_with_payment(doc: &mut VaultDoc) {
    let alive: BTreeSet<String> = doc
        .transactions
        .iter()
        .map(|tx| tx.meta().id.clone())
        .collect();
    let waiting: BTreeSet<String> = doc.sync.tombstones.keys().cloned().collect();
    let linked: BTreeSet<String> = doc.shared_events.iter().flat_map(event_links).collect();
    for event in &mut doc.shared_events {
        for record in std::mem::take(&mut event.removed_with_payment) {
            if !alive.contains(&record.tx_id) {
                if waiting.contains(&format!("transactions:{}", record.tx_id)) {
                    event.removed_with_payment.push(record);
                }
                continue;
            }
            if linked.contains(&record.tx_id) {
                continue;
            }
            let item = record
                .item_id
                .as_ref()
                .and_then(|item_id| event.items.iter_mut().find(|item| item.id == *item_id));
            if let (Some(purchase), Some(item)) = (record.purchase, item) {
                item.purchases.push(purchase);
            }
            if let Some(settlement) = record.settlement {
                event.settlements.push(settlement);
            }
        }
    }
}

// -- payments of shared events -----------------------------------------------

/// Payments (and debt transfers) that a shared purchase or settlement
/// names, which are gone with a tombstone. Deletion loses to a live money
/// reference: the payment comes back as it was. A payment that was never
/// here (no tombstone) is not touched; one with no value to restore goes
/// to `unrestored` and the purchase stays as it is. True when a payment came back.
pub fn restore_referenced_transactions(doc: &mut VaultDoc, report: &mut RepairReport) -> bool {
    let named: BTreeSet<String> = doc.shared_events.iter().flat_map(event_links).collect();
    let mut restored_any = false;
    for id in named {
        if doc.transactions.iter().any(|tx| tx.meta().id == id) {
            continue;
        }
        let key = format!("transactions:{id}");
        if !doc.sync.tombstones.contains_key(&key) {
            continue;
        }
        match deleted_value::<Transaction>(doc, &key) {
            Some(tx) => {
                let label = if tx.meta().comment.trim().is_empty() {
                    tx.meta().timestamp.clone()
                } else {
                    tx.meta().comment.clone()
                };
                report.restored.push((id, label));
                doc.transactions.push(tx);
                restored_any = true;
            }
            None => report.unrestored.push(id),
        }
    }
    // The caller recomputes the balances, which follow the operations; the tracker stamps
    // the payment fresh and drops its tombstone when the repair settles.
    restored_any
}

// -- participants of shared events ----------------------------------------

/// Every participant id an event names with money or a split (the
/// owner included).
fn named_participants(event: &SharedEvent) -> Vec<&str> {
    let mut named: Vec<&str> = vec![event.organizer.as_str()];
    for settlement in &event.settlements {
        named.push(&settlement.participant_id);
        named.extend(settlement.to.iter().map(String::as_str));
    }
    named.extend(
        event
            .adjustments
            .iter()
            .map(|fix| fix.participant_id.as_str()),
    );
    for item in &event.items {
        match &item.split {
            ItemSplit::Equal { among } => named.extend(among.iter().map(String::as_str)),
            ItemSplit::ByUnits { units } => named.extend(units.keys().map(String::as_str)),
        }
        named.extend(
            item.purchases
                .iter()
                .map(|purchase| purchase.buyer.as_str()),
        );
    }
    named
}

/// Whether a live record of the event still names the participant.
#[cfg(any(test, feature = "testing"))]
pub fn event_names_participant(doc: &VaultDoc, event_id: &str, participant_id: &str) -> bool {
    doc.shared_events
        .iter()
        .filter(|event| event.id == event_id)
        .any(|event| named_participants(event).contains(&participant_id))
}

fn missing_participants(doc: &VaultDoc) -> BTreeMap<String, BTreeSet<String>> {
    let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for event in &doc.shared_events {
        let people: BTreeSet<&str> = event
            .participants
            .iter()
            .map(|person| person.id.as_str())
            .chain([ME])
            .collect();
        for who in named_participants(event) {
            if !people.contains(who) {
                out.entry(event.id.clone())
                    .or_default()
                    .insert(who.to_string());
            }
        }
    }
    out
}

pub fn restore_participants(doc: &mut VaultDoc, report: &mut RepairReport) {
    for (event_id, ids) in missing_participants(doc) {
        for id in ids {
            let person = removed_participant(doc, &event_id, &id);
            let Some(person) = person else {
                report.unrestored.push(id);
                continue;
            };
            if let Some(event) = doc
                .shared_events
                .iter_mut()
                .find(|event| event.id == event_id)
            {
                report.restored.push((id, person.name.clone()));
                event.participants.push(person);
            }
        }
    }
}

// -- stores, categories, contacts -------------------------------------------

/// The ids that money names: stores, categories, contacts.
fn money_names(doc: &VaultDoc) -> (BTreeSet<String>, BTreeSet<String>, BTreeSet<String>) {
    let mut stores = BTreeSet::new();
    let mut categories = BTreeSet::new();
    let mut contacts = BTreeSet::new();
    for tx in &doc.transactions {
        stores.extend(tx.store_ids());
        categories.extend(tx.meta().categories.iter().cloned());
        contacts.extend(tx.contact_ids().into_iter().map(str::to_string));
    }
    stores.extend(
        doc.envelopes
            .iter()
            .map(|envelope| envelope.store_id.clone()),
    );
    stores.extend(
        doc.valuations
            .iter()
            .map(|valuation| valuation.store_id.clone()),
    );
    contacts.extend(doc.debt_terms.iter().map(|terms| terms.contact_id.clone()));
    contacts.extend(
        doc.shared_events
            .iter()
            .flat_map(|event| &event.participants)
            .filter_map(|person| person.contact_id.clone()),
    );
    (stores, categories, contacts)
}

/// Whether money still names the record under this sync key
/// (`stores:<id>`, `categories:<key>`, `contacts:<id>`, and
/// `transactions:<id>` for a payment a shared event names).
#[cfg(any(test, feature = "testing"))]
pub fn money_names_record(doc: &VaultDoc, key: &str) -> bool {
    let (stores, categories, contacts) = money_names(doc);
    match key.split_once(':') {
        Some(("stores", id)) => stores.contains(id),
        Some(("categories", id)) => categories.contains(id),
        Some(("contacts", id)) => contacts.contains(id),
        Some(("transactions", id)) => doc
            .shared_events
            .iter()
            .flat_map(event_links)
            .any(|linked| linked == id),
        _ => false,
    }
}

pub fn restore_referenced_records(doc: &mut VaultDoc, report: &mut RepairReport) {
    let (named_stores, named_categories, named_contacts) = money_names(doc);
    let lost_stores: BTreeSet<String> = named_stores
        .into_iter()
        .filter(|id| !doc.stores.iter().any(|node| node.id == *id))
        .collect();
    let lost_categories: BTreeSet<String> = named_categories
        .into_iter()
        .filter(|key| !doc.categories.iter().any(|cat| cat.key == *key))
        .collect();
    let lost_contacts: BTreeSet<String> = named_contacts
        .into_iter()
        .filter(|id| !doc.contacts.iter().any(|contact| contact.id == *id))
        .collect();

    for id in lost_stores {
        match deleted_value::<Store>(doc, &format!("stores:{id}")) {
            Some(node) => {
                report.restored.push((id, node.name.clone()));
                doc.stores.push(node);
            }
            None => report.unrestored.push(id),
        }
    }
    for key in lost_categories {
        match deleted_value::<Category>(doc, &format!("categories:{key}")) {
            Some(category) => {
                let name = category.name.clone().unwrap_or_else(|| key.clone());
                report.restored.push((key, name));
                doc.categories.push(category);
            }
            None => report.unrestored.push(key),
        }
    }
    for id in lost_contacts {
        match deleted_value::<Contact>(doc, &format!("contacts:{id}")) {
            Some(contact) => {
                report.restored.push((id, contact.name.clone()));
                doc.contacts.push(contact);
            }
            None => report.unrestored.push(id),
        }
    }
}

/// The operations a shared event names.
fn event_links(event: &SharedEvent) -> Vec<String> {
    let purchases = event.items.iter().flat_map(|item| &item.purchases);
    purchases
        .filter_map(|purchase| purchase.tx_id.clone())
        .chain(
            event
                .settlements
                .iter()
                .filter_map(|settlement| settlement.tx_id.clone()),
        )
        .collect()
}
