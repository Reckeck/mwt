//! Tests of the contacts area: the old inline tests (names and bodies unchanged),
//! then the pure functions on a bare document and the delete scenario.

use super::contacts::*;
use crate::Amount;
use crate::model::contact::{Contact, ContactAccount, ContactKind, DetailKind};
use crate::model::store::StoreKind;
use crate::model::transaction::{AccountRef, DebtEffect, Transaction};
use crate::service::UnlockedVault;
use crate::service::test_support::*;

fn ann(vault: &mut UnlockedVault) -> Contact {
    vault
        .create_contact(ContactDraft {
            group: String::new(),
            name: "Ann".into(),
            kind: ContactKind::Person,
            note: String::new(),
            accounts: vec![ContactAccount {
                value: "DE00 1234".into(),
                kind: DetailKind::Iban,
                ..ContactAccount::default()
            }],
        })
        .unwrap()
}

fn with_debt(tx: Transaction, effect: DebtEffect) -> Transaction {
    match tx {
        Transaction::Transfer(mut inner) => {
            inner.debt = Some(effect);
            Transaction::Transfer(inner)
        }
        other => other,
    }
}

#[test]
fn debts_add_up_per_contact_and_count_in_net_worth() {
    let (_dir, mut vault) = setup();
    vault.set_base_currency("usd");
    quote_usd(&mut vault);
    let store = vault.doc.stores[1].id.clone();
    let contact = ann(&mut vault);
    let mine = || AccountRef::Store(store.clone());
    let theirs = || AccountRef::Contact {
        contact_id: contact.id.clone(),
        account_id: None,
    };
    vault
        .add_transaction(income(&store, "10000", "salary"))
        .unwrap();
    for (from, to, raw, effect) in [
        (mine(), theirs(), "5000", DebtEffect::Lend),
        (theirs(), mine(), "2000", DebtEffect::Collect),
        (theirs(), mine(), "700", DebtEffect::Borrow),
        (mine(), theirs(), "200", DebtEffect::Repay),
    ] {
        vault
            .add_transaction(with_debt(transfer_between(from, to, usd(), raw), effect))
            .unwrap();
    }
    let balances = vault.contact_balances();
    assert_eq!(balances.len(), 1);
    assert_eq!(balances[0].receivable, amount("3000"));
    assert_eq!(balances[0].payable, amount("500"));

    let worth = vault.net_worth();
    // 10000 − 5000 + 2000 + 700 − 200 on the exchange
    assert_eq!(worth.financial, amount("7500"));
    assert_eq!(worth.receivables, amount("3000"));
    assert_eq!(worth.payables, amount("500"));
    assert_eq!(worth.net, amount("10000"));
    // a debt is neither income nor spending
    for tx in &vault.doc.transactions[1..] {
        assert_eq!(vault.classify_flow(tx), crate::service::FlowClass::Ignored);
    }
}

#[test]
fn turnover_counts_sent_and_received_but_not_internal_moves() {
    let (_dir, mut vault) = setup();
    let store = vault.doc.stores[1].id.clone();
    let sub = vault
        .create_store("Sub", StoreKind::Exchange, "", "", Some(store.clone()))
        .unwrap()
        .id;
    let contact = ann(&mut vault);
    let account = contact.accounts[0].id.clone();
    let theirs = |account_id: Option<String>| AccountRef::Contact {
        contact_id: contact.id.clone(),
        account_id,
    };
    vault.add_transaction(income(&store, "1000", "in")).unwrap();
    vault
        .add_transaction(transfer_between(
            AccountRef::Store(store.clone()),
            theirs(Some(account.clone())),
            usd(),
            "300",
        ))
        .unwrap();
    vault
        .add_transaction(transfer_between(
            theirs(None),
            AccountRef::Store(store.clone()),
            usd(),
            "50",
        ))
        .unwrap();
    // internal, but tagged with the contact: still not turnover
    let mut internal = transfer_between(
        AccountRef::Store(store.clone()),
        AccountRef::Store(sub),
        usd(),
        "10",
    );
    internal.meta_mut().contact_id = Some(contact.id.clone());
    vault.add_transaction(internal).unwrap();

    let all = vault.contact_turnover(&contact.id, None).unwrap();
    assert_eq!(all.tx_count, 2);
    assert_eq!(all.rows[0].sent, amount("300"));
    assert_eq!(all.rows[0].received, amount("50"));
    let one = vault.contact_turnover(&contact.id, Some(&account)).unwrap();
    assert_eq!(one.tx_count, 1);
}

fn summary_of(vault: &UnlockedVault, contact_id: &str) -> ContactSummary {
    vault
        .contact_summaries()
        .unwrap()
        .into_iter()
        .find(|summary| summary.contact_id == contact_id)
        .unwrap()
}

#[test]
fn summary_of_a_contact_without_operations_is_empty() {
    let (_dir, mut vault) = setup();
    let contact = ann(&mut vault);
    let summary = summary_of(&vault, &contact.id);
    assert_eq!(
        summary,
        ContactSummary {
            contact_id: contact.id.clone(),
            ..ContactSummary::default()
        }
    );
}

#[test]
fn summary_converts_debts_both_ways_and_skips_internal_moves() {
    let (_dir, mut vault) = setup();
    vault.set_base_currency("usd");
    quote_usd(&mut vault);
    vault.doc.fx_cache.push(crate::model::vault::FxPoint {
        currency: "eur".into(),
        per_usd: amount("0.5"),
        ts: "2000-01-01T00:00:00Z".into(),
        source: "test".into(),
    });
    let store = vault.doc.stores[1].id.clone();
    let sub = vault
        .create_store("Sub", StoreKind::Exchange, "", "", Some(store.clone()))
        .unwrap()
        .id;
    let contact = ann(&mut vault);
    let mine = || AccountRef::Store(store.clone());
    let theirs = || AccountRef::Contact {
        contact_id: contact.id.clone(),
        account_id: None,
    };
    let eur = crate::model::store::AssetRef::fiat("EUR", "Euro");
    vault
        .add_transaction(income(&store, "10000", "in"))
        .unwrap();
    // they owe me 100 USD; I owe them 50 EUR (= 100 USD at 0.5 EUR per USD)
    vault
        .add_transaction(with_debt(
            transfer_between(mine(), theirs(), usd(), "100"),
            DebtEffect::Lend,
        ))
        .unwrap();
    vault
        .add_transaction(with_debt(
            transfer_between(theirs(), mine(), eur, "50"),
            DebtEffect::Borrow,
        ))
        .unwrap();
    let mut internal = transfer_between(mine(), AccountRef::Store(sub), usd(), "10");
    internal.meta_mut().contact_id = Some(contact.id.clone());
    vault.add_transaction(internal).unwrap();

    let summary = summary_of(&vault, &contact.id);
    assert_eq!(summary.tx_count, 2);
    assert_eq!(summary.receivable_base, amount("100"));
    assert_eq!(summary.payable_base, amount("100"));
    assert!(!summary.missing_rate);
    assert!(summary.last_tx_at.is_some());
    assert_eq!(summary.last_asset.as_deref(), Some("EUR"));
    assert_eq!(summary.last_amount, Some(amount("50")));
}

#[test]
fn summary_flags_a_debt_currency_without_a_rate() {
    let (_dir, mut vault) = setup();
    vault.set_base_currency("usd");
    quote_usd(&mut vault);
    let store = vault.doc.stores[1].id.clone();
    let contact = ann(&mut vault);
    let eur = crate::model::store::AssetRef::fiat("EUR", "Euro");
    vault.add_transaction(income(&store, "1000", "in")).unwrap();
    vault
        .add_transaction(with_debt(
            transfer_between(
                AccountRef::Store(store),
                AccountRef::Contact {
                    contact_id: contact.id.clone(),
                    account_id: None,
                },
                eur,
                "30",
            ),
            DebtEffect::Lend,
        ))
        .unwrap();
    let summary = summary_of(&vault, &contact.id);
    assert!(summary.missing_rate);
    assert_eq!(summary.receivable_base, Amount::ZERO);
    assert_eq!(summary.last_amount, Some(amount("-30")));
}

#[test]
fn used_contacts_are_archived_and_used_accounts_deactivated() {
    let (_dir, mut vault) = setup();
    let store = vault.doc.stores[1].id.clone();
    let contact = ann(&mut vault);
    let account = contact.accounts[0].clone();
    vault
        .add_transaction(transfer_between(
            AccountRef::Store(store),
            AccountRef::Contact {
                contact_id: contact.id.clone(),
                account_id: Some(account.id.clone()),
            },
            usd(),
            "1",
        ))
        .unwrap();
    let refused = vault.delete_contact(&contact.id).unwrap_err();
    assert_eq!(refused.code(), "CONTACT_IN_USE");
    let updated = vault
        .update_contact(
            &contact.id,
            ContactDraft {
                name: "Ann B.".into(),
                ..ContactDraft::default()
            },
        )
        .unwrap();
    assert_eq!(updated.accounts.len(), 1);
    assert!(!updated.accounts[0].active);
    vault.archive_contact(&contact.id, true).unwrap();

    let unused = vault
        .create_contact(ContactDraft {
            name: "Bob".into(),
            ..ContactDraft::default()
        })
        .unwrap();
    vault.delete_contact(&unused.id).unwrap();
}

#[test]
fn debt_terms_are_set_replaced_and_cleared() {
    let (_dir, mut vault) = crate::service::test_support::empty_vault();
    let friend = vault
        .create_contact(ContactDraft {
            name: "Friend".into(),
            ..ContactDraft::default()
        })
        .unwrap();
    let draft = |due: Option<&str>, note: &str| DebtTermsDraft {
        contact_id: friend.id.clone(),
        asset: "usd".into(),
        due_at: due.map(str::to_string),
        note: note.into(),
    };
    assert!(vault.set_debt_terms(draft(Some("31.12.2026"), "")).is_err());
    vault
        .set_debt_terms(draft(Some("2026-12-31"), "for the car"))
        .unwrap();
    vault.set_debt_terms(draft(Some("2027-01-15"), "")).unwrap();
    let terms = vault.debt_terms(Some(&friend.id));
    assert_eq!(terms.len(), 1);
    assert_eq!(terms[0].asset, "USD");
    assert_eq!(terms[0].due_at.as_deref(), Some("2027-01-15"));
    assert!(vault.set_debt_terms(draft(None, "")).unwrap().is_none());
    assert!(vault.debt_terms(None).is_empty());
}

#[test]
fn a_bare_document_decides_deletes_and_terms_without_a_vault() {
    let mut doc = crate::model::VaultDoc::default();
    let contact = plan_create_contact(
        ContactDraft {
            name: " Ann ".into(),
            ..ContactDraft::default()
        },
        "2026-01-01T00:00:00Z",
    )
    .unwrap();
    assert_eq!(contact.name, "Ann");
    assert!(plan_delete_contact(&doc, &contact.id).is_err());
    doc.contacts.push(contact.clone());
    plan_delete_contact(&doc, &contact.id).unwrap();
    let plan = plan_debt_terms(
        &doc,
        DebtTermsDraft {
            contact_id: contact.id.clone(),
            asset: " usd ".into(),
            due_at: None,
            note: String::new(),
        },
    )
    .unwrap();
    assert_eq!(plan.asset, "USD");
    assert!(plan.clears());
}

#[test]
fn deleting_a_contact_unlinks_its_debt_terms_in_one_transaction() {
    let (_dir, mut vault) = empty_vault();
    let friend = vault
        .create_contact(ContactDraft {
            name: "Friend".into(),
            ..ContactDraft::default()
        })
        .unwrap();
    vault
        .set_debt_terms(DebtTermsDraft {
            contact_id: friend.id.clone(),
            asset: "usd".into(),
            due_at: Some("2026-12-31".into()),
            note: String::new(),
        })
        .unwrap();
    let before = vault.doc.debt_terms.len();
    assert_eq!(before, 1);
    let commits = std::sync::Arc::new(Commits::default());
    vault.set_write_behind(Some(commits.clone()));
    vault.delete_contact(&friend.id).unwrap();
    assert!(vault.doc.contacts.is_empty());
    assert!(vault.doc.debt_terms.is_empty());
    assert_eq!(
        commits.0.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "the contact and its terms go in one commit"
    );
}

/// Counts the commits a vault marks once it batches its writes.
#[derive(Default)]
struct Commits(std::sync::atomic::AtomicUsize);

impl crate::kernel::session::SaveHook for Commits {
    fn dirty(&self) {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

/// A value as JSON with every id and timestamp blanked, so twin vaults compare.
fn without_ids<T: serde::Serialize>(value: &T) -> serde_json::Value {
    fn blank(node: &mut serde_json::Value) {
        match node {
            serde_json::Value::Object(fields) => {
                for (key, child) in fields.iter_mut() {
                    if key == "id" || key.ends_with("_id") || key.ends_with("_at") {
                        *child = serde_json::Value::Null;
                    } else {
                        blank(child);
                    }
                }
            }
            serde_json::Value::Array(items) => items.iter_mut().for_each(blank),
            _ => {}
        }
    }
    let mut json = serde_json::to_value(value).unwrap();
    blank(&mut json);
    json
}

fn draft(name: &str) -> ContactDraft {
    ContactDraft {
        name: name.into(),
        ..ContactDraft::default()
    }
}

#[test]
fn the_delegates_equal_the_scenarios() {
    use crate::usecases::household::contacts as scenario;
    let (_dir_a, mut by_method) = empty_vault();
    let (_dir_b, mut by_scenario) = empty_vault();

    let method = by_method.create_contact(draft(" Ann ")).unwrap();
    let planned = scenario::create_contact(&mut by_scenario, draft(" Ann ")).unwrap();
    assert_eq!(without_ids(&method), without_ids(&planned));

    let method = by_method.update_contact(&method.id, draft("Anna")).unwrap();
    let planned = scenario::update_contact(&mut by_scenario, &planned.id, draft("Anna")).unwrap();
    assert_eq!(without_ids(&method), without_ids(&planned));

    by_method.archive_contact(&method.id, true).unwrap();
    scenario::archive_contact(&mut by_scenario, &planned.id, true).unwrap();
    assert_eq!(
        without_ids(&by_method.doc.contacts),
        without_ids(&by_scenario.doc.contacts)
    );

    let terms = |contact_id: &str| DebtTermsDraft {
        contact_id: contact_id.into(),
        asset: "usd".into(),
        due_at: Some("2026-12-31".into()),
        note: "back".into(),
    };
    let by_delegate = by_method.set_debt_terms(terms(&method.id)).unwrap();
    let by_usecase = scenario::set_debt_terms(&mut by_scenario, terms(&planned.id)).unwrap();
    assert_eq!(without_ids(&by_delegate), without_ids(&by_usecase));
    assert_eq!(
        by_method.doc.debt_terms.len(),
        by_scenario.doc.debt_terms.len()
    );

    by_method.delete_contact(&method.id).unwrap();
    scenario::delete_contact(&mut by_scenario, &planned.id).unwrap();
    assert!(by_method.doc.contacts.is_empty() && by_scenario.doc.contacts.is_empty());
    assert_eq!(
        by_method.doc.debt_terms.len(),
        by_scenario.doc.debt_terms.len()
    );
}
