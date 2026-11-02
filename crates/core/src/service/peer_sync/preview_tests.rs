//! The question before a batch is applied: readable labels, the fields an
//! edit changes, and the capital and balances once applied.

use super::tests::{settle, two_devices};
use super::*;
use crate::model::transaction::Transaction;
use crate::service::test_support::income;
use crate::sync::plan::ChangeKind;
use crate::sync::registry::COLLECTIONS;

fn cash_id(vault: &UnlockedVault) -> String {
    vault.doc.stores[0].id.clone()
}

/// What `from` holds that `into` lacks or has older, as the session sends it.
fn batch_for(into: &mut UnlockedVault, from: &mut UnlockedVault) -> BTreeMap<String, RecordState> {
    let mine = into.sync_manifest(None).unwrap();
    let theirs = from.sync_manifest(None).unwrap();
    let (wanted, _) = into.sync_diff(&mine, &theirs);
    from.sync_records(&wanted).unwrap()
}

#[test]
fn preview_lists_changed_fields() {
    let (_dir, mut left, mut right) = two_devices();
    let cash = cash_id(&left);
    left.add_transaction(income(&cash, "10", "salary")).unwrap();
    settle(&mut left, &mut right);
    let id = right.doc.transactions[0].meta().id.clone();
    let mut edited = right.doc.transactions[0].clone();
    if let Transaction::Topup(tx) = &mut edited {
        tx.meta.comment = "bonus".into();
        tx.amount = crate::Amount::from(25u32);
    }
    right.update_transaction(&id, edited).unwrap();

    let incoming = batch_for(&mut left, &mut right);
    let summary = left.sync_preview(&incoming).unwrap();
    let changes = summary
        .collections
        .iter()
        .find(|group| group.collection == "transactions")
        .expect("transactions changed");
    let item = &changes.items[0];
    assert_eq!(item.kind, ChangeKind::Updated);
    assert_eq!(item.key, id);
    let comment = item
        .fields
        .iter()
        .find(|field| field.field == "meta.comment")
        .expect("comment field");
    assert_eq!(comment.before.as_deref(), Some("salary"));
    assert_eq!(comment.after.as_deref(), Some("bonus"));
    let amount = item
        .fields
        .iter()
        .find(|field| field.field == "amount")
        .expect("amount field");
    assert_eq!(amount.before.as_deref(), Some("10"));
    assert_eq!(amount.after.as_deref(), Some("25"));
    assert_eq!(amount.money_currency.as_deref(), Some("usd"));
    // Ids and internals are not offered as edits.
    assert!(item.fields.iter().all(|field| !field.field.ends_with("id")));
    assert!(item.label.contains("bonus"), "label: {}", item.label);
}

#[test]
fn preview_shows_balances_after() {
    let (_dir, mut left, mut right) = two_devices();
    let cash = cash_id(&left);
    left.add_transaction(income(&cash, "100", "base")).unwrap();
    settle(&mut left, &mut right);
    right.add_transaction(income(&cash, "20", "more")).unwrap();

    let incoming = batch_for(&mut left, &mut right);
    let after = left
        .sync_preview_after(&incoming)
        .unwrap()
        .expect("something changes");
    assert_eq!(after.currency, "usd");
    assert_eq!(after.net_worth_before, crate::Amount::from(100u32));
    assert_eq!(after.net_worth_after, crate::Amount::from(120u32));
    let store = after
        .stores
        .iter()
        .find(|store| store.id == cash)
        .expect("cash is listed");
    assert_eq!(store.before, crate::Amount::from(100u32));
    assert_eq!(store.after, crate::Amount::from(120u32));
    assert_eq!(store.name, left.doc.stores[0].name);
}

#[test]
fn preview_does_not_touch_the_vault() {
    let (_dir, mut left, mut right) = two_devices();
    let cash = cash_id(&left);
    right
        .add_transaction(income(&cash, "20", "theirs"))
        .unwrap();
    let incoming = batch_for(&mut left, &mut right);
    left.sync_preview(&incoming).unwrap();
    let doc_before = serde_json::to_value(&left.doc).unwrap();
    let file_before = std::fs::read(&left.path).unwrap();

    let after = left.sync_preview_after(&incoming).unwrap();
    assert!(after.is_some());

    assert_eq!(serde_json::to_value(&left.doc).unwrap(), doc_before);
    assert_eq!(std::fs::read(&left.path).unwrap(), file_before);
    assert!(left.doc.transactions.is_empty());
    assert!(left.doc.sync.batches.is_empty());
}

/// A record of `coll` whose only identity is `id` (every key path set to it).
fn opaque_record(coll: &crate::sync::registry::Collection, id: &str) -> Value {
    if coll.key.is_empty() {
        return Value::String(id.to_string());
    }
    let mut record = serde_json::Map::new();
    for path in coll.key {
        let steps: Vec<&str> = path.split('.').filter(|step| *step != "*").collect();
        let mut node = &mut record;
        for step in &steps[..steps.len() - 1] {
            node = node
                .entry((*step).to_string())
                .or_insert_with(|| Value::Object(serde_json::Map::new()))
                .as_object_mut()
                .unwrap();
        }
        node.insert(steps[steps.len() - 1].to_string(), Value::String(id.into()));
    }
    let body = Value::Object(record);
    if coll.key.first().is_some_and(|path| path.starts_with("*.")) {
        serde_json::json!({ "variant": body })
    } else {
        body
    }
}

#[test]
fn every_collection_label_is_not_a_raw_key() {
    let id = "7f3c9a52-18de-4b0a-9c11-5d2e8a1b6f40";
    for coll in COLLECTIONS {
        // A key that is itself a name or a date is the label by design.
        let named = coll
            .key
            .iter()
            .any(|path| ["name", "date", "month"].contains(path));
        let record = opaque_record(coll, if named { "Rent" } else { id });
        let label = crate::sync::view::label(coll, &record);
        if named {
            assert!(label.contains("Rent"), "{}: {label}", coll.name);
        } else {
            assert!(
                !label.contains("7f3c9a52"),
                "{}: label shows the raw key: {label}",
                coll.name
            );
        }
    }
}

#[test]
fn a_transaction_label_has_what_where_when_and_how_much() {
    let coll = crate::sync::registry::find("transactions").unwrap();
    let record = serde_json::json!({"payment": {
        "meta": {"id": "t1", "comment": "", "categories": ["groceries"],
                 "timestamp": "2026-11-03T10:00:00Z"},
        "amount": "2500",
        "asset": {"symbol": "RUB", "name": "Ruble"},
    }});
    let label = crate::sync::view::label(coll, &record);
    assert!(label.contains("Groceries"), "{label}");
    assert!(label.contains("2026-11-03"), "{label}");
    assert!(label.contains("-2500 RUB"), "{label}");
}
