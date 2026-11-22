//! Tests of the bulk area that need the vault (`UnlockedVault`): moved from
//! `money/bulk_tests.rs` with their assertions unchanged (crate-split T13).

use crate::error::Error;
use crate::model::store::StoreKind;
use crate::model::transaction::AccountRef;
use crate::money::bulk::*;
use crate::service::UnlockedVault;
use crate::service::test_support::*;

fn two_payments() -> (tempfile::TempDir, UnlockedVault, String, Vec<String>) {
    let (dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        income(&card, "100", "salary"),
    )
    .unwrap();
    let ids = ["food", "taxi"]
        .iter()
        .map(|comment| {
            crate::usecases::money::transactions::add_transaction(
                &mut vault,
                payment(&card, "10", comment),
            )
            .unwrap()
            .meta()
            .id
            .clone()
        })
        .collect();
    (dir, vault, card, ids)
}

#[test]
fn delete_and_restore_round_trip() {
    let (_dir, mut vault, card, ids) = two_payments();
    let deleted = crate::usecases::money::bulk::delete_transactions(&mut vault, &ids).unwrap();
    assert_eq!(deleted.len(), 2);
    assert_eq!(holding_of(&vault, &card, &usd()), amount("100"));
    crate::usecases::money::bulk::restore_transactions(&mut vault, deleted).unwrap();
    assert_eq!(holding_of(&vault, &card, &usd()), amount("80"));
    let restored: Vec<_> = vault
        .doc
        .transactions
        .iter()
        .map(|tx| tx.meta().id.clone())
        .collect();
    assert!(ids.iter().all(|id| restored.contains(id)));
}

#[test]
fn an_unknown_id_deletes_nothing() {
    let (_dir, mut vault, _card, mut ids) = two_payments();
    ids.push("nope".into());
    assert!(crate::usecases::money::bulk::delete_transactions(&mut vault, &ids).is_err());
    assert_eq!(vault.doc.transactions.len(), 3);
}

#[test]
fn categories_are_set_and_given_back() {
    let (_dir, mut vault, _card, ids) = two_payments();
    let changes = ids
        .iter()
        .map(|id| TxCategories {
            id: id.clone(),
            categories: vec!["transport".into()],
        })
        .collect();
    let before =
        crate::usecases::money::bulk::set_transaction_categories(&mut vault, changes).unwrap();
    assert!(before.iter().all(|entry| entry.categories == ["groceries"]));
    let now = vault
        .doc
        .transactions
        .iter()
        .find(|tx| tx.meta().id == ids[0])
        .unwrap();
    assert_eq!(now.meta().categories, ["transport"]);
    crate::usecases::money::bulk::set_transaction_categories(&mut vault, before).unwrap();
    let back = vault
        .doc
        .transactions
        .iter()
        .find(|tx| tx.meta().id == ids[0])
        .unwrap();
    assert_eq!(back.meta().categories, ["groceries"]);
}

#[test]
fn payments_move_but_transfers_do_not() {
    let (_dir, mut vault, card, ids) = two_payments();
    let other = store(&mut vault, "Other", StoreKind::Bank, None);
    let before = crate::usecases::money::bulk::move_transactions(
        &mut vault,
        vec![TxStore {
            id: ids[0].clone(),
            store_id: other.clone(),
        }],
    )
    .unwrap();
    assert_eq!(before[0].store_id, card);
    assert_eq!(holding_of(&vault, &other, &usd()), amount("-10"));

    let moved = crate::usecases::money::transactions::add_transaction(
        &mut vault,
        transfer_between(
            crate::model::transaction::AccountRef::Store(card.clone()),
            crate::model::transaction::AccountRef::Store(other.clone()),
            usd(),
            "5",
        ),
    )
    .unwrap();
    let refused = crate::usecases::money::bulk::move_transactions(
        &mut vault,
        vec![
            TxStore {
                id: ids[1].clone(),
                store_id: other.clone(),
            },
            TxStore {
                id: moved.meta().id.clone(),
                store_id: card.clone(),
            },
        ],
    );
    assert!(refused.is_err());
    // All or nothing: the payment stayed where it was.
    let second = vault
        .doc
        .transactions
        .iter()
        .find(|tx| tx.meta().id == ids[1])
        .unwrap();
    assert_eq!(second.primary_store_id(), Some(card.as_str()));
}

// -- the commit and the change set --------------------------------------------

/// Counts the commits a vault marks once it batches its writes.
#[derive(Default)]
pub(super) struct Commits(std::sync::atomic::AtomicUsize);

impl Commits {
    /// Make `vault` batch its writes and count each commit it marks from now on.
    pub(super) fn watch(vault: &mut UnlockedVault) -> std::sync::Arc<Self> {
        let commits = std::sync::Arc::new(Self::default());
        vault.set_write_behind(Some(commits.clone()));
        commits
    }

    pub(super) fn count(&self) -> usize {
        self.0.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl crate::kernel::session::SaveHook for Commits {
    fn dirty(&self) {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

/// The document as JSON. The holdings are derived: they get fresh ids and no
/// fixed order at every rebuild (today's behaviour, also on the way back from
/// a failed call), so they are compared by what they hold, sorted.
fn document_json(vault: &UnlockedVault) -> String {
    let mut doc = serde_json::to_value(&vault.doc).unwrap();
    let holdings = doc["holdings"].as_array_mut().unwrap();
    for holding in holdings.iter_mut() {
        holding["id"] = serde_json::Value::Null;
    }
    holdings.sort_by_key(|holding| holding.to_string());
    doc.to_string()
}

/// Each bulk call is one commit: the revision moves by one, whatever the
/// number of items.
#[test]
fn a_bulk_call_is_one_commit() {
    let (_dir, mut vault, _card, ids) = two_payments();
    let commits = Commits::watch(&mut vault);
    let deleted = crate::usecases::money::bulk::delete_transactions(&mut vault, &ids).unwrap();
    assert_eq!(commits.count(), 1);
    crate::usecases::money::bulk::restore_transactions(&mut vault, deleted).unwrap();
    assert_eq!(commits.count(), 2);
    let changes = ids
        .iter()
        .map(|id| TxCategories {
            id: id.clone(),
            categories: vec!["taxi".into()],
        })
        .collect();
    crate::usecases::money::bulk::set_transaction_categories(&mut vault, changes).unwrap();
    assert_eq!(commits.count(), 3);
}

/// The change set of a delete: the listed operations leave
/// `doc.transactions`, the holdings are rebuilt from what is left, nothing
/// else in the document moves.
#[test]
fn a_delete_changes_the_operations_and_the_holdings_only() {
    let (_dir, mut vault, card, ids) = two_payments();
    let before: serde_json::Value = serde_json::to_value(&vault.doc).unwrap();
    crate::usecases::money::bulk::delete_transactions(&mut vault, &ids).unwrap();
    let after: serde_json::Value = serde_json::to_value(&vault.doc).unwrap();
    let changed: Vec<&str> = before
        .as_object()
        .unwrap()
        .keys()
        .filter(|key| before[key.as_str()] != after[key.as_str()])
        .map(String::as_str)
        .collect();
    assert!(changed.contains(&"transactions") && changed.contains(&"holdings"));
    assert!(
        changed
            .iter()
            .all(|key| ["transactions", "holdings", "sync"].contains(key)),
        "unexpected change in {changed:?}"
    );
    assert_eq!(holding_of(&vault, &card, &usd()), amount("100"));
}

/// An action that fails on item 5 reports it, and nothing of the four before
/// it stays: the document is as it was, no commit is made.
#[test]
fn a_bulk_that_fails_on_the_fifth_item_leaves_nothing_behind() {
    let (_dir, mut vault, card, _) = two_payments();
    let mut ids: Vec<String> = (0..4)
        .map(|index| {
            crate::usecases::money::transactions::add_transaction(
                &mut vault,
                payment(&card, "1", &format!("extra {index}")),
            )
            .unwrap()
            .meta()
            .id
            .clone()
        })
        .collect();
    ids.push("missing".into());
    let before = document_json(&vault);
    let commits = Commits::watch(&mut vault);

    let changes: Vec<TxCategories> = ids
        .iter()
        .map(|id| TxCategories {
            id: id.clone(),
            categories: vec!["taxi".into()],
        })
        .collect();
    let err =
        crate::usecases::money::bulk::set_transaction_categories(&mut vault, changes).unwrap_err();
    assert!(matches!(err, Error::Missing(_)), "{err}");
    assert_eq!(document_json(&vault), before);
    assert_eq!(commits.count(), 0);

    let err = crate::usecases::money::bulk::delete_transactions(&mut vault, &ids).unwrap_err();
    assert!(matches!(err, Error::Missing(_)), "{err}");
    assert_eq!(document_json(&vault), before);
    assert_eq!(commits.count(), 0);
}

#[test]
fn a_move_that_fails_on_a_transfer_leaves_the_payments_where_they_were() {
    let (_dir, mut vault, card, ids) = two_payments();
    let cash = store(&mut vault, "Cash2", StoreKind::Bank, None);
    let two_sided = crate::usecases::money::transactions::add_transaction(
        &mut vault,
        transfer_between(
            AccountRef::Store(card.clone()),
            AccountRef::Store(cash.clone()),
            usd(),
            "1",
        ),
    )
    .unwrap()
    .meta()
    .id
    .clone();
    let before = document_json(&vault);
    let changes = ids
        .iter()
        .chain(std::iter::once(&two_sided))
        .map(|id| TxStore {
            id: id.clone(),
            store_id: cash.clone(),
        })
        .collect();
    assert!(crate::usecases::money::bulk::move_transactions(&mut vault, changes).is_err());
    assert_eq!(document_json(&vault), before);
}
