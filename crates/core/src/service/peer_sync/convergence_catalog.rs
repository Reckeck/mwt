//! Stage M of the sync fixes: the edit kinds that reproduce the catalog bugs
//! (keys and links after a merge) and the invariants checked once the devices
//! have converged. Every violation belongs to a catalog item; an item still
//! open is tolerated only while it is listed in [`KNOWN_OPEN`]. A fix slice
//! removes its item from the list, so the same test then guards the fix.
//!
//! `random_edits_without_known_open_flags_are_red_today` runs the harness
//! with the list empty: it shows which invariants the current code breaks.

use super::convergence_tests::{Edit, Ledger, Rng, event_ids, guest, pick};
use super::*;
use crate::ipc_contract::{
    AttachPaymentInput, SharedEventDraft, SharedItemDraft, SharedPurchaseDraft,
};
use crate::model::planning::AssetClass;
use crate::model::shared_event::{ItemSplit, ME, PriceMode};
use crate::model::store::{AssetRef, MAX_STORE_DEPTH, StoreKind, StoreScope};
use crate::model::transaction::Transaction;
use crate::service::StoreExtras;
use crate::service::allocation::AssetClassTarget;
use crate::service::contacts::DebtTermsDraft;
use crate::service::goals::GoalDraft;
use crate::service::test_support::{amount, income_of, payment, usd};
use crate::service::utilities::UtilityDraft;
use crate::service::valuation::ValuationInput;
use std::collections::{BTreeMap, BTreeSet};

/// One item of the stage M catalog (`trade-app-sync-bugs.md`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Item {
    /// 25: a cycle, a missing parent or a parent that is not a wallet.
    AccountTree25,
    /// 28: a live record names a store, category or contact that is gone.
    DeletedReferenced28,
    /// 103: two debt terms for one contact and asset.
    DebtTerms103,
    /// 104: the shares do not add up to 100, or a link names a missing class.
    AssetClasses104,
    /// 105: two readings for one meter and day, two valuations for one asset and day.
    Readings105,
    /// 114: more than one root Cash wallet.
    CashWallets114,
    /// 115: a live record names a participant that was removed.
    DeletedParticipant115,
    /// 116: the event currency does not match the purchases in it.
    EventCurrency116,
    /// 122: an envelope of a deleted goal, or two for one goal and wallet.
    GoalEnvelopes122,
    /// 123: two contacts for one guest, a payment linked to two events.
    GuestsAndLinks123,
    /// A purchase added to an item next to another device's one is lost
    /// (purchases live inside the item; decision 4 of the stage).
    ParallelPurchases,
}

/// The items that still reproduce today. Each fix slice removes its own.
pub(super) const KNOWN_OPEN: &[Item] = &[
    Item::EventCurrency116,
    Item::GuestsAndLinks123,
    Item::ParallelPurchases,
];

#[derive(Clone, Debug)]
pub(super) struct Violation {
    pub item: Item,
    pub text: String,
}

fn found(out: &mut Vec<Violation>, item: Item, text: String) {
    out.push(Violation { item, text });
}

/// A stand-in for a non-financial asset a property holds (valuations need one).
fn flat_asset() -> AssetRef {
    AssetRef {
        symbol: "FLAT".into(),
        name: "Flat".into(),
        coingecko_id: None,
        chain: None,
        contract: None,
        quote: "FLAT".into(),
    }
}

/// The purchase of the flat; the random edits may delete it, so a valuation
/// buys it again when the device holds none.
fn flat_topup(property: &str) -> Transaction {
    let mut flat = income_of(property, flat_asset(), "1", "1", "flat");
    if let Transaction::Topup(topup) = &mut flat {
        topup.details.unit_price = Some(amount("100000"));
    }
    flat
}

/// A property with a flat in it and a metered utility, made on device 0 and
/// pulled by the others, so valuations and readings have something to hang on.
pub(super) fn add_catalog_stores(devices: &mut [UnlockedVault], ledger: &mut Ledger) {
    let property = devices[0]
        .create_store_with(
            "conv flat",
            StoreKind::Property,
            "#555",
            "",
            None,
            StoreExtras {
                property: Some(crate::model::store::PropertyInfo {
                    category: crate::model::store::PropertyCategory::Flat,
                    ..Default::default()
                }),
                ..StoreExtras::default()
            },
        )
        .unwrap()
        .id;
    devices[0].add_transaction(flat_topup(&property)).unwrap();
    ledger.thing_key = flat_asset().key();
    ledger.utility_id = devices[0]
        .create_utility_item(UtilityDraft {
            name: "Electricity".into(),
            active: true,
            kind: crate::model::planning::UtilityKind::Metered {
                unit: "kWh".into(),
                tariff: amount("5"),
                formula: None,
                zones: Vec::new(),
            },
            currency: "usd".into(),
            property_id: property.clone(),
            store_id: None,
            category: "utilities".into(),
        })
        .unwrap()
        .id;
    ledger.property_id = property;
    for name in ["anchor A", "anchor B"] {
        let contact = devices[0]
            .create_contact(crate::service::contacts::ContactDraft {
                name: name.into(),
                kind: crate::model::contact::ContactKind::Person,
                note: String::new(),
                accounts: Vec::new(),
                group: String::new(),
            })
            .unwrap();
        ledger.anchors.push(contact.id);
    }
    for to in 1..devices.len() {
        super::convergence_tests::pull(devices, 0, to);
    }
}

/// One of the first `limit` of the sorted ids. A small pool makes two
/// devices that edit independently reach for the same record now and then.
fn pick_early(rng: &mut Rng, mut ids: Vec<String>, limit: usize) -> Option<String> {
    ids.sort();
    ids.truncate(limit);
    pick(rng, &ids).cloned()
}

fn pick_event(
    vault: &UnlockedVault,
    rng: &mut Rng,
) -> Option<crate::model::shared_event::SharedEvent> {
    let id = pick_early(rng, event_ids(vault), 2)?;
    vault
        .doc
        .shared_events
        .iter()
        .find(|event| event.id == id)
        .cloned()
}

fn purchase_ids(vault: &UnlockedVault, event_id: &str) -> BTreeSet<(String, String)> {
    vault
        .doc
        .shared_events
        .iter()
        .filter(|event| event.id == event_id)
        .flat_map(|event| &event.items)
        .flat_map(|item| {
            item.purchases
                .iter()
                .map(|purchase| (item.id.clone(), purchase.id.clone()))
        })
        .collect()
}

/// The edits of the catalog. A refusal of the core is a normal outcome of a
/// random edit, as in the other appliers.
pub(super) fn apply_catalog_edit(
    kind: Edit,
    vault: &mut UnlockedVault,
    rng: &mut Rng,
    ledger: &mut Ledger,
) {
    let number = ledger.fresh();
    match kind {
        Edit::SetDebtTerms => {
            let mut ids: Vec<String> = ledger
                .anchors
                .iter()
                .filter(|id| vault.doc.contacts.iter().any(|contact| &contact.id == *id))
                .cloned()
                .collect();
            if ids.is_empty() {
                ids = vault
                    .doc
                    .contacts
                    .iter()
                    .map(|contact| contact.id.clone())
                    .collect();
            }
            let Some(contact_id) = pick(rng, &ids).cloned() else {
                return;
            };
            let clear = rng.below(5) == 0;
            let _ = vault.set_debt_terms(DebtTermsDraft {
                contact_id,
                asset: ["USD", "EUR"][rng.below(2)].into(),
                due_at: (!clear).then(|| format!("2026-0{}-1{}", 4 + rng.below(5), rng.below(9))),
                note: if clear {
                    String::new()
                } else {
                    format!("terms {number}")
                },
            });
        }
        Edit::AddMeterReading => {
            let day = 1 + rng.below(3);
            // Roughly rising with the day, so most readings pass the monotonic rule.
            let value = amount(&format!("{}", day * 100 + rng.below(50)));
            let _ = vault.add_meter_reading(
                &ledger.utility_id,
                value,
                "2026-03",
                Some(u8::try_from(day).unwrap_or(1)),
                None,
            );
        }
        Edit::AddValuation => {
            let held = vault.doc.holdings.iter().any(|row| {
                row.store_id == ledger.property_id && row.asset.key() == ledger.thing_key
            });
            if !held {
                let _ = vault.add_transaction(flat_topup(&ledger.property_id));
            }
            let _ = vault.add_valuation(ValuationInput {
                store_id: ledger.property_id.clone(),
                asset_key: ledger.thing_key.clone(),
                date: format!("2026-03-0{}", 1 + rng.below(3)),
                unit_price: amount(&format!("{}", 1000 + rng.below(500))),
                currency: "usd".into(),
            });
        }
        Edit::MoveAccount => {
            let banks: Vec<String> = vault
                .doc
                .stores
                .iter()
                .filter(|candidate| candidate.name.starts_with("bank") && !candidate.archived)
                .map(|candidate| candidate.id.clone())
                .collect();
            let subs: Vec<String> = vault
                .doc
                .stores
                .iter()
                .filter(|candidate| candidate.name.starts_with("sv"))
                .map(|candidate| candidate.id.clone())
                .collect();
            if rng.below(5) == 0 {
                // An empty wallet goes while another device moves an account into it.
                let empty: Vec<String> = banks
                    .iter()
                    .filter(|bank| vault.children_of(bank).is_empty())
                    .cloned()
                    .collect();
                if let Some(bank) = pick_early(rng, empty, 2)
                    && vault.delete_store(&bank).is_ok()
                {
                    ledger.deleted.insert(format!("stores:{bank}"));
                }
                return;
            }
            let (Some(sub), Some(target)) = (pick(rng, &subs).cloned(), pick_early(rng, banks, 2))
            else {
                return;
            };
            if vault.move_store(&sub, Some(&target)).is_ok() {
                ledger.touched_after.insert(format!("stores:{sub}"));
            }
        }
        Edit::CreateGoal => {
            if vault.doc.goals.len() >= 4 {
                return;
            }
            let store_id = vault.doc.stores[0].id.clone();
            let _ = vault.create_goal(GoalDraft {
                name: format!("goal {number}"),
                active: true,
                target_amount: amount("1000"),
                currency: "usd".into(),
                starts_at: "2026-01-01T00:00:00Z".into(),
                deadline: Some("2026-12-31T00:00:00Z".into()),
                priority: 3,
                monthly_commitment: None,
                scope: StoreScope {
                    store_ids: vec![store_id],
                    include_children: true,
                },
                scenario_id: None,
                expected_return: None,
            });
        }
        Edit::DeleteGoal => {
            let ids: Vec<String> = vault.doc.goals.iter().map(|goal| goal.id.clone()).collect();
            let Some(id) = pick(rng, &ids).cloned() else {
                return;
            };
            if vault
                .delete_goal(&id, chrono::Utc::now().timestamp())
                .is_ok()
            {
                ledger.deleted.insert(format!("goals:{id}"));
            }
        }
        Edit::FundGoal => {
            let ids: Vec<String> = vault.doc.goals.iter().map(|goal| goal.id.clone()).collect();
            let Some(id) = pick(rng, &ids).cloned() else {
                return;
            };
            let store_id = vault.doc.stores[0].id.clone();
            let _ = vault.fund_goal(&id, &store_id, amount(&format!("{}", 1 + rng.below(20))));
        }
        Edit::CreatePayment => {
            let store_id = vault.doc.stores[0].id.clone();
            let _ = vault.add_transaction(payment(
                &store_id,
                &format!("{}", 1 + rng.below(5)),
                "conv pay",
            ));
        }
        Edit::MakeGuestContact => {
            let Some(event) = pick_event(vault, rng) else {
                return;
            };
            let guests: Vec<String> = event
                .participants
                .iter()
                .filter(|person| person.contact_id.is_none())
                .map(|person| person.id.clone())
                .collect();
            if let Some(person) = pick_early(rng, guests, 2).as_ref()
                && vault
                    .make_shared_participant_contact(&event.id, person)
                    .is_ok()
            {
                // An edit of the person after another device removed them wins.
                ledger
                    .touched_after
                    .insert(format!("{}/participant/{person}", event.id));
            }
        }
        Edit::AttachPayment => {
            let Some(event) = pick_event(vault, rng) else {
                return;
            };
            let payments: Vec<String> = vault
                .doc
                .transactions
                .iter()
                .filter(|tx| matches!(tx, Transaction::Payment(_)))
                .map(|tx| tx.meta().id.clone())
                .collect();
            let Some(tx_id) = pick_early(rng, payments, 2) else {
                return;
            };
            let before = purchase_ids(vault, &event.id);
            let attached = vault.attach_payment_to_shared_event(AttachPaymentInput {
                event_id: event.id.clone(),
                tx_id,
                new_item_title: Some(format!("attached {number}")),
                ..Default::default()
            });
            if attached.is_ok() {
                remember_purchases(vault, &event.id, &before, ledger);
            }
        }
        Edit::ChangeEventCurrency => {
            // Only an event nothing is bought in yet may change its currency.
            let bare: Vec<String> = vault
                .doc
                .shared_events
                .iter()
                .filter(|event| event.items.iter().all(|item| item.purchases.is_empty()))
                .map(|event| event.id.clone())
                .collect();
            let Some(event) = pick_early(rng, bare, 3).and_then(|id| {
                vault
                    .doc
                    .shared_events
                    .iter()
                    .find(|event| event.id == id)
                    .cloned()
            }) else {
                return;
            };
            let _ = vault.update_shared_event(
                &event.id,
                SharedEventDraft {
                    title: event.title.clone(),
                    note: event.note.clone(),
                    starts_on: event.starts_on.clone(),
                    ends_on: event.ends_on.clone(),
                    currency: ["usd", "eur"][rng.below(2)].into(),
                    rounding_step: Some(event.rounding_step),
                    category_id: event.category_id.clone(),
                    organizer: event.organizer.clone(),
                    i_participate: event.i_participate,
                    participants: Vec::new(),
                },
            );
        }
        Edit::SetAssetClasses => set_asset_classes(vault, rng),
        Edit::UndoLastBatch => undo_last_batch(vault, ledger),
        Edit::SharedPurchaseToItem => {
            let Some(event) = pick_event(vault, rng) else {
                return;
            };
            let (Some(item), Some(person)) = (
                pick(rng, &event.items).cloned(),
                pick(rng, &event.participants).cloned(),
            ) else {
                return;
            };
            let before = purchase_ids(vault, &event.id);
            let added = vault.add_shared_purchase(
                &event.id,
                SharedPurchaseDraft {
                    item_id: Some(item.id),
                    buyer: person.id,
                    qty: amount("1"),
                    amount: amount(&format!("{}", 10 + rng.below(90))),
                    currency: event.currency.clone(),
                    price_mode: PriceMode::Total,
                    ..Default::default()
                },
            );
            if added.is_ok() {
                remember_purchases(vault, &event.id, &before, ledger);
            }
        }
        _ => {}
    }
}

fn remember_purchases(
    vault: &UnlockedVault,
    event_id: &str,
    before: &BTreeSet<(String, String)>,
    ledger: &mut Ledger,
) {
    for (item, purchase) in purchase_ids(vault, event_id).difference(before) {
        ledger
            .purchases
            .push((event_id.to_string(), item.clone(), purchase.clone()));
    }
}

/// Baskets drawn from three fixed ids, so two devices editing at once edit the
/// same records; shares add up to 100 on the device that saves them.
fn set_asset_classes(vault: &mut UnlockedVault, rng: &mut Rng) {
    let ids = ["cls-a", "cls-b", "cls-c"];
    let count = 1 + rng.below(ids.len());
    let first = rng.below(ids.len());
    let chosen: Vec<&str> = (0..count)
        .map(|offset| ids[(first + offset) % ids.len()])
        .collect();
    let mut classes = Vec::new();
    let mut left = 100usize;
    for (index, id) in chosen.iter().enumerate() {
        let share = if index + 1 == chosen.len() {
            left
        } else {
            let part = 10 * (1 + rng.below(left / 10 - (chosen.len() - index - 1)));
            left -= part;
            part
        };
        classes.push(AssetClass {
            id: (*id).into(),
            name: format!("class {id}"),
            target_pct: amount(&format!("{share}")),
            cap: false,
        });
    }
    match rng.below(6) {
        // Back to the preset.
        0 => {
            let _ = vault.set_asset_classes(Vec::new());
        }
        // Put an asset or a wallet in one of the baskets.
        1 => {
            let target = if rng.below(2) == 0 {
                AssetClassTarget::Asset {
                    asset_key: usd().key(),
                }
            } else {
                AssetClassTarget::Store {
                    store_id: vault.doc.stores[0].id.clone(),
                }
            };
            let class = ids[rng.below(ids.len())].to_string();
            let _ = vault.assign_asset_class(target, Some(class));
        }
        _ => {
            let _ = vault.set_asset_classes(classes);
        }
    }
}

/// Undo the newest batch this device applied. Undo brings records back, so
/// everything the batch touched is exempt from the "deleted stays deleted" check.
fn undo_last_batch(vault: &mut UnlockedVault, ledger: &mut Ledger) {
    let Some(batch) = vault
        .doc
        .sync
        .batches
        .iter()
        .rev()
        .find(|batch| batch.undone_at.is_none() && !batch.entries.is_empty())
        .cloned()
    else {
        return;
    };
    if vault
        .sync_undo(&batch.id)
        .is_ok_and(|report| report.restored > 0)
    {
        let keys: Vec<String> = ledger.deleted.iter().cloned().collect();
        for key in keys {
            let owner = key.split('/').next().unwrap_or(&key).to_string();
            let touched = batch
                .entries
                .iter()
                .any(|entry| entry.key == key || (key.contains('/') && entry.key.contains(&owner)));
            if touched {
                ledger.touched_after.insert(key);
            }
        }
    }
}

/// Every invariant of the stage on one converged device.
pub(super) fn invariants(vault: &UnlockedVault, ledger: &Ledger) -> Vec<Violation> {
    let mut out = Vec::new();
    check_cash_and_tree(vault, &mut out);
    check_references(vault, &mut out);
    check_natural_keys(vault, &mut out);
    check_shared_events(vault, ledger, &mut out);
    out
}

fn check_cash_and_tree(vault: &UnlockedVault, out: &mut Vec<Violation>) {
    let roots = vault
        .doc
        .stores
        .iter()
        .filter(|candidate| candidate.kind == StoreKind::Cash && candidate.parent_id.is_none())
        .count();
    if roots != 1 {
        found(
            out,
            Item::CashWallets114,
            format!("{roots} root Cash wallets"),
        );
    }
    let by_id: BTreeMap<&str, &crate::model::store::Store> = vault
        .doc
        .stores
        .iter()
        .map(|candidate| (candidate.id.as_str(), candidate))
        .collect();
    for node in &vault.doc.stores {
        let Some(parent_id) = node.parent_id.as_deref() else {
            continue;
        };
        let Some(parent) = by_id.get(parent_id) else {
            found(
                out,
                Item::AccountTree25,
                format!("account {} has a missing parent {parent_id}", node.id),
            );
            continue;
        };
        if parent.parent_id.is_some() {
            // Covers a cycle too: every member of one has a parent that has one.
            found(
                out,
                Item::AccountTree25,
                format!("account {} sits under the account {parent_id}", node.id),
            );
        } else if MAX_STORE_DEPTH == 0 {
            found(
                out,
                Item::AccountTree25,
                format!("account {} is nested", node.id),
            );
        }
        if parent.kind != node.kind {
            found(
                out,
                Item::AccountTree25,
                format!("account {} is not of its parent's kind", node.id),
            );
        }
    }
}

fn check_references(vault: &UnlockedVault, out: &mut Vec<Violation>) {
    let doc = &vault.doc;
    let stores: BTreeSet<&str> = doc.stores.iter().map(|node| node.id.as_str()).collect();
    let categories: BTreeSet<&str> = doc.categories.iter().map(|cat| cat.key.as_str()).collect();
    let contacts: BTreeSet<&str> = doc
        .contacts
        .iter()
        .map(|contact| contact.id.as_str())
        .collect();
    let goals: BTreeSet<&str> = doc.goals.iter().map(|goal| goal.id.as_str()).collect();
    let classes: BTreeSet<String> = vault
        .effective_classes()
        .into_iter()
        .map(|class| class.id)
        .collect();
    let mut missing = |item: Item, what: &str, owner: String, target: &str, known: bool| {
        if !known {
            found(
                out,
                item,
                format!("{owner} names a missing {what} {target}"),
            );
        }
    };
    for tx in &doc.transactions {
        let owner = format!("transaction {}", tx.meta().id);
        for id in tx.store_ids() {
            missing(
                Item::DeletedReferenced28,
                "store",
                owner.clone(),
                &id,
                stores.contains(id.as_str()),
            );
        }
        for key in &tx.meta().categories {
            missing(
                Item::DeletedReferenced28,
                "category",
                owner.clone(),
                key,
                categories.contains(key.as_str()),
            );
        }
        for id in tx.contact_ids() {
            missing(
                Item::DeletedReferenced28,
                "contact",
                owner.clone(),
                id,
                contacts.contains(id),
            );
        }
    }
    for envelope in &doc.envelopes {
        let owner = format!("envelope {}", envelope.id);
        missing(
            Item::DeletedReferenced28,
            "store",
            owner.clone(),
            &envelope.store_id,
            stores.contains(envelope.store_id.as_str()),
        );
        if let Some(goal) = &envelope.goal_id {
            missing(
                Item::GoalEnvelopes122,
                "goal",
                owner,
                goal,
                goals.contains(goal.as_str()),
            );
        }
    }
    for terms in &doc.debt_terms {
        missing(
            Item::DeletedReferenced28,
            "contact",
            format!("debt terms {}", terms.id),
            &terms.contact_id,
            contacts.contains(terms.contact_id.as_str()),
        );
    }
    for valuation in &doc.valuations {
        missing(
            Item::DeletedReferenced28,
            "store",
            format!("valuation {}", valuation.id),
            &valuation.store_id,
            stores.contains(valuation.store_id.as_str()),
        );
    }
    for link in &doc.asset_class_of {
        missing(
            Item::AssetClasses104,
            "class",
            format!("asset {}", link.asset_key),
            &link.class_id,
            classes.contains(&link.class_id),
        );
    }
    for node in &doc.stores {
        if let Some(class) = &node.asset_class {
            missing(
                Item::AssetClasses104,
                "class",
                format!("wallet {}", node.id),
                class,
                classes.contains(class),
            );
        }
    }
}

fn check_natural_keys(vault: &UnlockedVault, out: &mut Vec<Violation>) {
    let doc = &vault.doc;
    let mut seen = BTreeSet::new();
    for terms in &doc.debt_terms {
        if !seen.insert((terms.contact_id.clone(), terms.asset.clone())) {
            found(
                out,
                Item::DebtTerms103,
                format!(
                    "second debt terms for {} in {}",
                    terms.contact_id, terms.asset
                ),
            );
        }
    }
    let mut seen = BTreeSet::new();
    for reading in &doc.meter_readings {
        let key = (
            reading.item_id.clone(),
            reading.zone.clone(),
            reading.sort_key(),
        );
        if !seen.insert(key) {
            found(
                out,
                Item::Readings105,
                format!(
                    "second reading of {} for {}",
                    reading.item_id, reading.period
                ),
            );
        }
    }
    let mut seen = BTreeSet::new();
    for valuation in &doc.valuations {
        if !seen.insert((valuation.asset_key.clone(), valuation.date.clone())) {
            found(
                out,
                Item::Readings105,
                format!(
                    "second valuation of {} on {}",
                    valuation.asset_key, valuation.date
                ),
            );
        }
    }
    let mut seen = BTreeSet::new();
    for envelope in &doc.envelopes {
        if let Some(goal) = &envelope.goal_id
            && !seen.insert((
                goal.clone(),
                envelope.store_id.clone(),
                envelope.asset.key(),
            ))
        {
            found(
                out,
                Item::GoalEnvelopes122,
                format!("second envelope of goal {goal} in {}", envelope.store_id),
            );
        }
    }
    let total = doc
        .asset_classes
        .iter()
        .fold(crate::Amount::ZERO, |sum, class| sum + class.target_pct);
    if doc
        .asset_classes
        .iter()
        .any(|class| !class.target_pct.is_zero())
        && total != crate::Amount::from(100)
    {
        found(
            out,
            Item::AssetClasses104,
            format!("shares add up to {total}"),
        );
    }
}

fn check_shared_events(vault: &UnlockedVault, ledger: &Ledger, out: &mut Vec<Violation>) {
    let doc = &vault.doc;
    let contacts: BTreeSet<&str> = doc
        .contacts
        .iter()
        .map(|contact| contact.id.as_str())
        .collect();
    let mut guest_names: BTreeMap<&str, usize> = BTreeMap::new();
    for contact in &doc.contacts {
        if contact.name.starts_with("guest ") {
            *guest_names.entry(contact.name.as_str()).or_default() += 1;
        }
    }
    for (name, count) in guest_names.iter().filter(|(_, count)| **count > 1) {
        found(
            out,
            Item::GuestsAndLinks123,
            format!("{count} contacts named {name}"),
        );
    }
    let mut links: BTreeMap<String, usize> = BTreeMap::new();
    for link in vault.shared_tx_links() {
        *links.entry(link.tx_id).or_default() += 1;
    }
    for (tx_id, count) in links.iter().filter(|(_, count)| **count > 1) {
        found(
            out,
            Item::GuestsAndLinks123,
            format!("payment {tx_id} is linked {count} times"),
        );
    }
    for event in &doc.shared_events {
        let people: BTreeSet<&str> = event
            .participants
            .iter()
            .map(|person| person.id.as_str())
            .chain([ME])
            .collect();
        let mut named = |who: &str, what: &str| {
            if !people.contains(who) {
                found(
                    out,
                    Item::DeletedParticipant115,
                    format!(
                        "event {}: {what} names a missing participant {who}",
                        event.id
                    ),
                );
            }
        };
        named(&event.organizer, "the organizer");
        for settlement in &event.settlements {
            named(&settlement.participant_id, "a settlement");
        }
        for fix in &event.adjustments {
            named(&fix.participant_id, "an adjustment");
        }
        for item in &event.items {
            match &item.split {
                ItemSplit::Equal { among } => among.iter().for_each(|who| named(who, "a split")),
                ItemSplit::ByUnits { units } => {
                    units.keys().for_each(|who| named(who, "a split"));
                }
            }
            for purchase in &item.purchases {
                named(&purchase.buyer, "a purchase");
            }
        }
        for person in &event.participants {
            if let Some(contact) = &person.contact_id
                && !contacts.contains(contact.as_str())
            {
                found(
                    out,
                    Item::DeletedReferenced28,
                    format!(
                        "participant {} names a missing contact {contact}",
                        person.id
                    ),
                );
            }
        }
        for purchase in event.items.iter().flat_map(|item| &item.purchases) {
            if !purchase.rate_manual
                && purchase.rate_to_event == crate::Amount::from(1)
                && purchase.currency != event.currency
            {
                found(
                    out,
                    Item::EventCurrency116,
                    format!(
                        "event {}: purchase {} in {} has rate 1 to {}",
                        event.id, purchase.id, purchase.currency, event.currency
                    ),
                );
            }
        }
    }
    for (event_id, item_id, purchase_id) in &ledger.purchases {
        let Some(item) = doc
            .shared_events
            .iter()
            .find(|event| &event.id == event_id)
            .and_then(|event| event.items.iter().find(|item| &item.id == item_id))
        else {
            continue;
        };
        if !item
            .purchases
            .iter()
            .any(|purchase| &purchase.id == purchase_id)
        {
            found(
                out,
                Item::ParallelPurchases,
                format!("purchase {purchase_id} of item {item_id} is gone"),
            );
        }
    }
}

/// The deterministic form of the purchase loss: two devices add one purchase
/// each to the same item concurrently.
#[cfg(test)]
pub(super) fn two_purchases_to_one_item() -> usize {
    let (_dir, mut left, mut right) = super::tests::two_devices();
    let event = left
        .create_shared_event(SharedEventDraft {
            title: "trip".into(),
            currency: "usd".into(),
            participants: vec![guest("ann".into())],
            ..Default::default()
        })
        .unwrap();
    let event_id = event.event.id.clone();
    left.add_shared_item(
        &event_id,
        SharedItemDraft {
            title: "taxi".into(),
            ..Default::default()
        },
    )
    .unwrap();
    super::tests::settle(&mut left, &mut right);
    for (vault, price) in [(&mut left, "10"), (&mut right, "20")] {
        let current = vault.doc.shared_events[0].clone();
        vault
            .add_shared_purchase(
                &event_id,
                SharedPurchaseDraft {
                    item_id: Some(current.items[0].id.clone()),
                    buyer: current.participants[0].id.clone(),
                    qty: amount("1"),
                    amount: amount(price),
                    currency: "usd".into(),
                    price_mode: PriceMode::Total,
                    ..Default::default()
                },
            )
            .unwrap();
    }
    super::tests::settle(&mut left, &mut right);
    assert_eq!(
        left.doc.shared_events[0].items[0].purchases.len(),
        right.doc.shared_events[0].items[0].purchases.len(),
        "the devices agree even when a purchase is lost"
    );
    left.doc.shared_events[0].items[0].purchases.len()
}
