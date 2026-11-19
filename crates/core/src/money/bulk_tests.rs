//! Tests of the bulk area: several operations at once, all or nothing.

use crate::model::store::StoreKind;
use crate::service::test_support::*;

use super::bulk::*;
use crate::error::Error;
use crate::model::transaction::AccountRef;
use crate::model::vault::VaultDoc;
use crate::service::UnlockedVault;

fn two_payments() -> (tempfile::TempDir, UnlockedVault, String, Vec<String>) {
    let (dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    vault
        .add_transaction(income(&card, "100", "salary"))
        .unwrap();
    let ids = ["food", "taxi"]
        .iter()
        .map(|comment| {
            vault
                .add_transaction(payment(&card, "10", comment))
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
    let deleted = vault.delete_transactions(&ids).unwrap();
    assert_eq!(deleted.len(), 2);
    assert_eq!(holding_of(&vault, &card, &usd()), amount("100"));
    vault.restore_transactions(deleted).unwrap();
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
    assert!(vault.delete_transactions(&ids).is_err());
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
    let before = vault.set_transaction_categories(changes).unwrap();
    assert!(before.iter().all(|entry| entry.categories == ["groceries"]));
    let now = vault
        .doc
        .transactions
        .iter()
        .find(|tx| tx.meta().id == ids[0])
        .unwrap();
    assert_eq!(now.meta().categories, ["transport"]);
    vault.set_transaction_categories(before).unwrap();
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
    let before = vault
        .move_transactions(vec![TxStore {
            id: ids[0].clone(),
            store_id: other.clone(),
        }])
        .unwrap();
    assert_eq!(before[0].store_id, card);
    assert_eq!(holding_of(&vault, &other, &usd()), amount("-10"));

    let moved = vault
        .add_transaction(transfer_between(
            crate::model::transaction::AccountRef::Store(card.clone()),
            crate::model::transaction::AccountRef::Store(other.clone()),
            usd(),
            "5",
        ))
        .unwrap();
    let refused = vault.move_transactions(vec![
        TxStore {
            id: ids[1].clone(),
            store_id: other.clone(),
        },
        TxStore {
            id: moved.meta().id.clone(),
            store_id: card.clone(),
        },
    ]);
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

// -- the pure parts, on a bare document ---------------------------------------

#[test]
fn tx_index_finds_an_operation_in_a_bare_document() {
    let tx = payment("card", "5", "a");
    let id = tx.meta().id.clone();
    let doc = VaultDoc {
        transactions: vec![income("card", "9", "b"), tx],
        ..VaultDoc::default()
    };
    assert_eq!(tx_index(&doc, &id).unwrap(), 1);
    assert!(matches!(tx_index(&doc, "nope"), Err(Error::Missing(_))));
}

#[test]
fn with_categories_returns_the_new_operation_and_the_old_categories() {
    let tx = payment("card", "5", "a");
    let change = TxCategories {
        id: tx.meta().id.clone(),
        categories: vec!["taxi".into()],
    };
    let (changed, had) = with_categories(&tx, change);
    assert_eq!(changed.meta().categories, vec!["taxi".to_string()]);
    assert_eq!(had.categories, vec!["groceries".to_string()]);
    assert_eq!(
        tx.meta().categories,
        vec!["groceries".to_string()],
        "the input is not touched"
    );
}

#[test]
fn with_store_moves_a_payment_and_refuses_a_transfer() {
    let tx = payment("card", "5", "a");
    let change = TxStore {
        id: tx.meta().id.clone(),
        store_id: "cash".into(),
    };
    let (moved, had) = with_store(&tx, change).unwrap();
    assert_eq!(moved.legs()[0].store_id, "cash");
    assert_eq!(had.store_id, "card");
    let two_sided = transfer("card", "cash", "5");
    let refused = TxStore {
        id: two_sided.meta().id.clone(),
        store_id: "x".into(),
    };
    assert!(with_store(&two_sided, refused).is_err());
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
    let deleted = vault.delete_transactions(&ids).unwrap();
    assert_eq!(commits.count(), 1);
    vault.restore_transactions(deleted).unwrap();
    assert_eq!(commits.count(), 2);
    let changes = ids
        .iter()
        .map(|id| TxCategories {
            id: id.clone(),
            categories: vec!["taxi".into()],
        })
        .collect();
    vault.set_transaction_categories(changes).unwrap();
    assert_eq!(commits.count(), 3);
}

/// The change set of a delete: the listed operations leave
/// `doc.transactions`, the holdings are rebuilt from what is left, nothing
/// else in the document moves.
#[test]
fn a_delete_changes_the_operations_and_the_holdings_only() {
    let (_dir, mut vault, card, ids) = two_payments();
    let before: serde_json::Value = serde_json::to_value(&vault.doc).unwrap();
    vault.delete_transactions(&ids).unwrap();
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
            vault
                .add_transaction(payment(&card, "1", &format!("extra {index}")))
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
    let err = vault.set_transaction_categories(changes).unwrap_err();
    assert!(matches!(err, Error::Missing(_)), "{err}");
    assert_eq!(document_json(&vault), before);
    assert_eq!(commits.count(), 0);

    let err = vault.delete_transactions(&ids).unwrap_err();
    assert!(matches!(err, Error::Missing(_)), "{err}");
    assert_eq!(document_json(&vault), before);
    assert_eq!(commits.count(), 0);
}

#[test]
fn a_move_that_fails_on_a_transfer_leaves_the_payments_where_they_were() {
    let (_dir, mut vault, card, ids) = two_payments();
    let cash = store(&mut vault, "Cash2", StoreKind::Bank, None);
    let two_sided = vault
        .add_transaction(transfer_between(
            AccountRef::Store(card.clone()),
            AccountRef::Store(cash.clone()),
            usd(),
            "1",
        ))
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
    assert!(vault.move_transactions(changes).is_err());
    assert_eq!(document_json(&vault), before);
}
