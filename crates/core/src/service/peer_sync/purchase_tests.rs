//! Protocol 3: every purchase of a shared event is an entry of its own
//! (`items.<item id>.purchases.<purchase id>`), so purchases added, edited or
//! removed on different devices meet one by one instead of the later write of
//! the whole item winning.

use super::tests::{settle, two_devices};
use super::*;
use crate::ipc_contract::{
    SharedEventDraft, SharedItemDraft, SharedParticipantDraft, SharedPurchaseDraft,
};
use crate::model::shared_event::PriceMode;
use crate::service::test_support::amount;

fn pause() {
    std::thread::sleep(std::time::Duration::from_millis(5));
}

fn draft(item_id: &str, buyer: &str, price: &str) -> SharedPurchaseDraft {
    SharedPurchaseDraft {
        item_id: Some(item_id.into()),
        buyer: buyer.into(),
        qty: amount("1"),
        amount: amount(price),
        currency: "usd".into(),
        price_mode: PriceMode::Total,
        ..Default::default()
    }
}

/// An event with one guest and one item, on both devices.
fn event_with_item(
    left: &mut UnlockedVault,
    right: &mut UnlockedVault,
) -> (String, String, String) {
    let event = left
        .create_shared_event(SharedEventDraft {
            title: "trip".into(),
            currency: "usd".into(),
            participants: vec![SharedParticipantDraft {
                name: "ann".into(),
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap()
        .event;
    let view = left
        .add_shared_item(
            &event.id,
            SharedItemDraft {
                title: "taxi".into(),
                ..Default::default()
            },
        )
        .unwrap();
    settle(left, right);
    (
        event.id,
        view.event.items[0].id.clone(),
        view.event.participants[0].id.clone(),
    )
}

/// `(id, total)` of the purchases of the first item, in order.
fn purchases(vault: &UnlockedVault) -> Vec<(String, String)> {
    vault.doc.shared_events[0]
        .items
        .first()
        .map(|item| {
            item.purchases
                .iter()
                .map(|purchase| (purchase.id.clone(), purchase.total().to_string()))
                .collect()
        })
        .unwrap_or_default()
}

fn totals(vault: &UnlockedVault) -> Vec<String> {
    let mut totals: Vec<String> = purchases(vault)
        .into_iter()
        .map(|(_, total)| total)
        .collect();
    totals.sort();
    totals
}

fn add(vault: &mut UnlockedVault, event: &str, item: &str, buyer: &str, price: &str) -> String {
    let view = vault
        .add_shared_purchase(event, draft(item, buyer, price))
        .unwrap();
    view.event
        .items
        .iter()
        .find(|candidate| candidate.id == item)
        .unwrap()
        .purchases
        .last()
        .unwrap()
        .id
        .clone()
}

#[test]
fn an_edit_of_one_purchase_and_a_new_one_elsewhere_both_stay() {
    let (_dir, mut left, mut right) = two_devices();
    let (event, item, buyer) = event_with_item(&mut left, &mut right);
    let first = add(&mut left, &event, &item, &buyer, "10");
    settle(&mut left, &mut right);
    left.update_shared_purchase(&event, &first, draft(&item, &buyer, "15"))
        .unwrap();
    pause();
    add(&mut right, &event, &item, &buyer, "20");
    settle(&mut left, &mut right);
    assert_eq!(totals(&left), vec!["15", "20"]);
    assert_eq!(purchases(&left), purchases(&right), "same order too");
}

#[test]
fn a_removed_purchase_and_an_edit_of_another_both_stay() {
    let (_dir, mut left, mut right) = two_devices();
    let (event, item, buyer) = event_with_item(&mut left, &mut right);
    let first = add(&mut left, &event, &item, &buyer, "10");
    let second = add(&mut left, &event, &item, &buyer, "20");
    settle(&mut left, &mut right);
    left.delete_shared_purchase(&event, &first).unwrap();
    pause();
    right
        .update_shared_purchase(&event, &second, draft(&item, &buyer, "25"))
        .unwrap();
    settle(&mut left, &mut right);
    assert_eq!(totals(&left), vec!["25"]);
    assert_eq!(purchases(&left), purchases(&right));
}

/// The item is removed, then the other device (which has not seen that) buys
/// something for it: the later write wins, as for any other field, and both
/// devices end equal. Here the purchase is later, so the item comes back with
/// it alone.
#[test]
fn a_purchase_made_after_the_item_was_removed_brings_the_item_back() {
    let (_dir, mut left, mut right) = two_devices();
    let (event, item, buyer) = event_with_item(&mut left, &mut right);
    left.delete_shared_item(&event, &item).unwrap();
    pause();
    let late = add(&mut right, &event, &item, &buyer, "20");
    settle(&mut left, &mut right);
    assert_eq!(purchases(&left), purchases(&right));
    let kept = purchases(&left);
    assert_eq!(kept.len(), 1, "{kept:?}");
    assert_eq!(kept[0].0, late);
}

/// The purchase was made first, the item removed after: the removal wins and
/// the purchase goes with its item.
#[test]
fn an_item_removed_after_a_purchase_took_it_along() {
    let (_dir, mut left, mut right) = two_devices();
    let (event, item, buyer) = event_with_item(&mut left, &mut right);
    add(&mut right, &event, &item, &buyer, "20");
    pause();
    left.delete_shared_item(&event, &item).unwrap();
    settle(&mut left, &mut right);
    assert!(left.doc.shared_events[0].items.is_empty());
    assert!(right.doc.shared_events[0].items.is_empty());
}

/// A vault synced before protocol 3: the stamps name only `items.<id>`.
#[test]
fn stamps_of_an_older_vault_without_purchase_entries_still_converge() {
    let (_dir, mut left, mut right) = two_devices();
    let (event, item, buyer) = event_with_item(&mut left, &mut right);
    let first = add(&mut left, &event, &item, &buyer, "10");
    left.sync_manifest(None).unwrap();
    // Rewrite the stamps the way an older build left them.
    let key = format!("shared_events:{event}");
    let stamps = left.doc.sync.stamps.get_mut(&key).unwrap();
    let purchase_prefix = format!("items.{item}.purchases.");
    let newest = stamps
        .fields
        .iter()
        .filter(|(name, _)| name.starts_with(&purchase_prefix))
        .map(|(_, stamp)| stamp.clone())
        .max()
        .expect("the new build stamps the purchase");
    stamps
        .fields
        .retain(|name, _| !name.starts_with(&purchase_prefix));
    stamps.fields.insert(format!("items.{item}"), newest);
    left.persist().unwrap();
    // A device that never met the event gets it, then both edit.
    settle(&mut left, &mut right);
    assert_eq!(totals(&right), vec!["10"]);
    left.update_shared_purchase(&event, &first, draft(&item, &buyer, "11"))
        .unwrap();
    pause();
    add(&mut right, &event, &item, &buyer, "30");
    settle(&mut left, &mut right);
    assert_eq!(totals(&left), vec!["11", "30"]);
    assert_eq!(purchases(&left), purchases(&right));
}

#[test]
fn undoing_a_sync_that_added_a_purchase_removes_only_that_purchase() {
    let (_dir, mut left, mut right) = two_devices();
    let (event, item, buyer) = event_with_item(&mut left, &mut right);
    add(&mut left, &event, &item, &buyer, "10");
    settle(&mut left, &mut right);
    add(&mut right, &event, &item, &buyer, "20");
    let (applied, _) = super::tests::exchange(&mut left, &mut right);
    let batch = applied.expect("the left device took the purchase in");
    assert_eq!(totals(&left), vec!["10", "20"]);
    left.sync_undo(&batch.id).unwrap();
    assert_eq!(totals(&left), vec!["10"]);
}
