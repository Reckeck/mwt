//! Tests of the transactions area: the pure rules on a read context, and the
//! scenarios against the delegates they replace.

use super::transactions::*;
use crate::error::Error;
use crate::model::store::StoreKind;
use crate::model::transaction::{Transaction, TxOrigin};
use crate::model::vault::VaultDoc;
use crate::service::test_support::*;
use crate::usecases::money::transactions::patch_transaction;

fn workflow_origin(estimated: bool) -> TxOrigin {
    TxOrigin::Workflow {
        workflow_id: "wf".into(),
        run_id: "run".into(),
        planned_at: None,
        planned_date: None,
        estimated,
        run_key: None,
    }
}

#[test]
fn a_taken_or_empty_id_is_replaced_by_a_fresh_one() {
    let (_dir, mut vault) = empty_vault();
    let cash = store(&mut vault, "Cash", StoreKind::Cash, None);
    let saved = vault.add_transaction(income(&cash, "1", "a")).unwrap();
    let mut twin = income(&cash, "2", "b");
    twin.meta_mut().id = saved.meta().id.clone();
    ensure_fresh_id(&vault.doc, &mut twin);
    assert_ne!(twin.meta().id, saved.meta().id);

    let mut blank = income(&cash, "3", "c");
    blank.meta_mut().id = "  ".into();
    ensure_fresh_id(&VaultDoc::default(), &mut blank);
    assert!(!blank.meta().id.trim().is_empty());

    let mut own = income(&cash, "4", "d");
    own.meta_mut().id = "mine".into();
    ensure_fresh_id(&VaultDoc::default(), &mut own);
    assert_eq!(own.meta().id, "mine");
}

#[test]
fn an_edit_keeps_the_id_origin_keep_mark_and_bill() {
    let (_dir, mut vault) = empty_vault();
    let cash = store(&mut vault, "Cash", StoreKind::Cash, None);
    let mut stored = income(&cash, "100", "pay");
    stored.meta_mut().origin = workflow_origin(true);
    stored.meta_mut().keep = true;
    stored.meta_mut().utility_item_id = Some("bill".into());

    let same_amount = patched(&stored, "id-1", replacing(income(&cash, "100", "pay")));
    assert_eq!(same_amount.meta().id, "id-1");
    assert_eq!(same_amount.meta().origin, workflow_origin(true));
    assert!(same_amount.meta().keep);
    assert_eq!(same_amount.meta().utility_item_id.as_deref(), Some("bill"));

    let real_amount = patched(&stored, "id-1", replacing(income(&cash, "120", "pay")));
    assert_eq!(real_amount.meta().origin, workflow_origin(false));
}

#[test]
fn add_linked_transactions_joins_one_chain_or_saves_nothing() {
    let (_dir, mut vault) = empty_vault();
    let cash = store(&mut vault, "Cash", StoreKind::Cash, None);
    let saved = vault
        .add_linked_transactions(
            vec![income(&cash, "1", "a"), income(&cash, "2", "b")],
            Some(" chain "),
        )
        .unwrap();
    assert_eq!(saved.len(), 2);
    assert!(
        saved
            .iter()
            .all(|tx| tx.meta().flow_id.as_deref() == Some("chain"))
    );

    let before = vault.doc.transactions.len();
    let mut bad = income(&cash, "3", "c");
    bad.meta_mut().categories = vec!["no-such-category".into()];
    let refused = vault.add_linked_transactions(vec![income(&cash, "3", "ok"), bad], None);
    assert!(refused.is_err());
    assert_eq!(vault.doc.transactions.len(), before);
    assert!(matches!(
        vault.add_linked_transactions(Vec::new(), None),
        Err(Error::InvalidInput(_))
    ));
}

#[test]
fn deleting_removes_the_operation_and_refuses_an_unknown_one() {
    let (_dir, mut vault) = empty_vault();
    let cash = store(&mut vault, "Cash", StoreKind::Cash, None);
    let saved = vault.add_transaction(income(&cash, "5", "a")).unwrap();
    vault.delete_transaction(&saved.meta().id).unwrap();
    assert!(vault.doc.transactions.is_empty());
    assert!(matches!(
        vault.delete_transaction(&saved.meta().id),
        Err(Error::Missing(_))
    ));
}

#[test]
fn an_edit_of_an_unknown_operation_is_refused() {
    let (_dir, mut vault) = empty_vault();
    let cash = store(&mut vault, "Cash", StoreKind::Cash, None);
    let edit: Transaction = income(&cash, "1", "a");
    assert!(matches!(
        vault.update_transaction("nope", edit),
        Err(Error::Missing(_))
    ));
}

#[test]
fn history_comes_newest_first() {
    let (_dir, mut vault) = empty_vault();
    let cash = store(&mut vault, "Cash", StoreKind::Cash, None);
    let mut old = income(&cash, "1", "old");
    old.meta_mut().timestamp = "2025-01-01T00:00:00Z".into();
    let mut new = income(&cash, "1", "new");
    new.meta_mut().timestamp = "2025-02-01T00:00:00Z".into();
    vault.add_transaction(old).unwrap();
    vault.add_transaction(new).unwrap();
    let history = vault.transactions(&Default::default());
    assert_eq!(history[0].meta().comment, "new");
}

fn linked_operation(vault: &mut crate::service::UnlockedVault, cash: &str) -> Transaction {
    let mut tx = income(cash, "100", "water");
    tx.meta_mut().flow_id = Some("flow-1".into());
    tx.meta_mut().utility_item_id = Some("util-1".into());
    tx.meta_mut().origin = TxOrigin::Import {
        exec_id: "exec-1".into(),
    };
    vault.add_transaction(tx).unwrap()
}

#[test]
fn a_patch_keeps_every_field_it_does_not_name() {
    let (_dir, mut vault) = empty_vault();
    let cash = store(&mut vault, "Cash", StoreKind::Cash, None);
    let saved = linked_operation(&mut vault, &cash);
    let id = saved.meta().id.clone();

    let patched = patch_transaction(
        &mut vault,
        &id,
        TransactionPatch::of(income(&cash, "120", "water")),
    )
    .unwrap();

    let meta = patched.meta();
    assert_eq!(meta.id, id);
    assert_eq!(meta.flow_id.as_deref(), Some("flow-1"));
    assert_eq!(meta.utility_item_id.as_deref(), Some("util-1"));
    assert_eq!(
        meta.origin,
        TxOrigin::Import {
            exec_id: "exec-1".into()
        }
    );
}

#[test]
fn a_patch_changes_only_the_links_it_names() {
    let (_dir, mut vault) = empty_vault();
    let cash = store(&mut vault, "Cash", StoreKind::Cash, None);
    let saved = linked_operation(&mut vault, &cash);
    let patch = TransactionPatch {
        utility_item_id: Some(None),
        flow_id: Some(Some("flow-2".into())),
        ..TransactionPatch::of(income(&cash, "100", "water"))
    };

    let patched = patch_transaction(&mut vault, &saved.meta().id, patch).unwrap();

    assert_eq!(patched.meta().utility_item_id, None);
    assert_eq!(patched.meta().flow_id.as_deref(), Some("flow-2"));
}

#[test]
fn a_replacing_edit_never_drops_the_flow_link() {
    let (_dir, mut vault) = empty_vault();
    let cash = store(&mut vault, "Cash", StoreKind::Cash, None);
    let saved = linked_operation(&mut vault, &cash);

    let updated = vault
        .update_transaction(&saved.meta().id, income(&cash, "120", "water"))
        .unwrap();

    assert_eq!(updated.meta().flow_id.as_deref(), Some("flow-1"));
}
