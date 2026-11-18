//! Tests of the validate area: the old inline tests (names and bodies unchanged),
//! then the pure rules on a bare document and the delegate pins.

use crate::model::chain::Chain;
use crate::model::contact::{ContactAccount, ContactKind, DetailKind};
use crate::model::store::{AssetRef, StoreKind};
use crate::model::transaction::*;
use crate::service::ContactDraft;
use crate::service::test_support::*;

fn rejected(result: crate::Result<Transaction>) -> String {
    match result {
        Err(err) => err.to_string(),
        Ok(tx) => panic!("expected a rejection, got {}", tx.tag()),
    }
}

/// V2: a kind the store does not accept.
#[test]
fn a_kind_must_fit_the_store() {
    let (_dir, mut vault) = setup();
    let cold = vault.doc.stores[2].id.clone();
    assert!(rejected(vault.add_transaction(trade(&cold, "1", "1", "x"))).contains("not possible"));
    let flat = store(&mut vault, "Flat", StoreKind::Property, None);
    assert!(rejected(vault.add_transaction(payment(&flat, "1", "x"))).contains("not possible"));
}

/// V3: cash and bank accounts hold fiat only.
#[test]
fn a_bank_holds_only_fiat() {
    let (_dir, mut vault) = empty_vault();
    let bank = store(&mut vault, "Bank", StoreKind::Bank, None);
    assert!(
        rejected(vault.add_transaction(income_of(&bank, btc(), "1", "1", "x")))
            .contains("only fiat")
    );
    vault.add_transaction(income(&bank, "1", "x")).unwrap();
}

/// V5, V12: topups and payments need a category of a matching group.
#[test]
fn categories_must_fit_the_verb() {
    let (_dir, mut vault) = setup();
    let store = vault.doc.stores[1].id.clone();
    let mut bare = payment(&store, "1", "x");
    bare.meta_mut().categories.clear();
    assert!(rejected(vault.add_transaction(bare)).contains("choose a category"));

    let mut wrong = income(&store, "1", "x");
    wrong.meta_mut().categories = vec!["groceries".into()];
    assert!(rejected(vault.add_transaction(wrong)).contains("does not fit"));

    vault.archive_category("groceries", true).unwrap();
    assert!(rejected(vault.add_transaction(payment(&store, "1", "x"))).contains("archived"));
}

/// V6: a transfer needs one of the user's stores and two different sides.
#[test]
fn transfers_need_a_store_and_two_sides() {
    let (_dir, mut vault) = setup();
    let raw = |text: &str| AccountRef::Raw(text.into());
    assert!(
        rejected(vault.add_transaction(transfer_between(raw("a"), raw("b"), usd(), "1")))
            .contains("must be your wallet")
    );
    let store = vault.doc.stores[1].id.clone();
    let mut convert = Transaction::Convert(ConvertTx {
        meta: meta("fx", &[]),
        from: ConvertSide {
            account: AccountRef::Store(store.clone()),
            asset: usd(),
            amount: amount("1"),
            price: PriceStamp::default(),
        },
        to: ConvertSide {
            account: raw("exchanger"),
            asset: usd(),
            amount: amount("1"),
            price: PriceStamp::default(),
        },
        fee: None,
        rate_override: None,
    });
    assert!(rejected(vault.add_transaction(convert.clone())).contains("two different assets"));
    if let Transaction::Convert(inner) = &mut convert {
        inner.to.asset = AssetRef::fiat("EUR", "Euro");
    }
    vault.add_transaction(convert).unwrap();
}

/// V7: a contact account must belong to the contact and be active.
#[test]
fn contact_accounts_are_checked() {
    let (_dir, mut vault) = setup();
    let store = vault.doc.stores[1].id.clone();
    let contact = vault
        .create_contact(ContactDraft {
            group: String::new(),
            name: "Mom".into(),
            kind: ContactKind::Person,
            note: String::new(),
            accounts: vec![ContactAccount {
                value: "4276 0000".into(),
                kind: DetailKind::Card,
                ..ContactAccount::default()
            }],
        })
        .unwrap();
    let account_id = contact.accounts[0].id.clone();
    let to = |account: &str| AccountRef::Contact {
        contact_id: contact.id.clone(),
        account_id: Some(account.into()),
    };
    let send = |to: AccountRef| transfer_between(AccountRef::Store(store.clone()), to, usd(), "5");
    assert!(rejected(vault.add_transaction(send(to("other")))).contains("does not belong"));
    vault.add_transaction(send(to(&account_id))).unwrap();
}

#[test]
fn a_huge_banknote_total_is_rejected_not_a_panic() {
    let (_dir, mut vault) = empty_vault();
    let cash = store(&mut vault, "Wallet", StoreKind::Cash, None);
    let Transaction::Topup(mut topup) = income(&cash, "300", "found") else {
        unreachable!()
    };
    topup.details.cash_breakdown = Some(vec![CashNote {
        denomination: amount("500000000000000000"),
        count: u32::MAX,
    }]);
    assert!(rejected(vault.add_transaction(Transaction::Topup(topup))).contains("too large"));
}

/// V8: details only where they make sense, banknotes must add up.
#[test]
fn kind_details_are_checked() {
    let (_dir, mut vault) = empty_vault();
    let cash = store(&mut vault, "Wallet", StoreKind::Cash, None);
    let Transaction::Topup(mut topup) = income(&cash, "300", "found") else {
        unreachable!()
    };
    topup.details.cash_breakdown = Some(vec![CashNote {
        denomination: amount("100"),
        count: 2,
    }]);
    assert!(rejected(vault.add_transaction(Transaction::Topup(topup.clone()))).contains("add up"));
    topup.details.cash_breakdown = Some(vec![CashNote {
        denomination: amount("100"),
        count: 3,
    }]);
    vault
        .add_transaction(Transaction::Topup(topup.clone()))
        .unwrap();
    topup.details.network = Some(Chain::Bitcoin);
    assert!(rejected(vault.add_transaction(Transaction::Topup(topup))).contains("crypto wallets"));
}

/// V9: realized PnL only on a close; leverage positive.
#[test]
fn futures_fields_are_checked() {
    let (_dir, mut vault) = setup();
    let store = vault.doc.stores[1].id.clone();
    let futures = |action: FuturesAction, pnl: Option<&str>| {
        Transaction::TradeFutures(TradeFuturesTx {
            meta: meta("perp", &[]),
            store_id: store.clone(),
            contract: AssetRef::perp("bybit", "BTCUSDT"),
            settle: AssetRef::fiat("USDT", "Tether"),
            direction: FuturesDirection::Long,
            action,
            quantity: amount("1"),
            fill_price: amount("100"),
            leverage: Some(amount("10")),
            fee: None,
            closed_pnl: pnl.map(amount),
            order_id: None,
            category: FuturesCategory::Linear,
            price: PriceStamp::default(),
        })
    };
    assert!(
        rejected(vault.add_transaction(futures(FuturesAction::Open, Some("5"))))
            .contains("closing")
    );
    vault
        .add_transaction(futures(FuturesAction::Open, None))
        .unwrap();
    vault
        .add_transaction(futures(FuturesAction::Close, Some("5")))
        .unwrap();
}

/// V11: a date too far ahead, or not a date at all.
#[test]
fn dates_are_checked() {
    let (_dir, mut vault) = setup();
    let store = vault.doc.stores[1].id.clone();
    let mut later = income(&store, "1", "x");
    at(&mut later, "2999-01-01T00:00:00Z");
    assert!(rejected(vault.add_transaction(later)).contains("future"));
    let mut garbage = income(&store, "1", "x");
    at(&mut garbage, "yesterday");
    assert!(rejected(vault.add_transaction(garbage)).contains("RFC3339"));
}

/// V14: the debt direction must match the sides.
#[test]
fn debt_direction_must_match_the_sides() {
    let (_dir, mut vault) = setup();
    let store = vault.doc.stores[1].id.clone();
    let contact = vault
        .create_contact(ContactDraft {
            name: "Ann".into(),
            ..ContactDraft::default()
        })
        .unwrap();
    let ann = AccountRef::Contact {
        contact_id: contact.id,
        account_id: None,
    };
    let mut lend = transfer_between(ann.clone(), AccountRef::Store(store.clone()), usd(), "10");
    if let Transaction::Transfer(inner) = &mut lend {
        inner.debt = Some(DebtEffect::Lend);
    }
    assert!(rejected(vault.add_transaction(lend)).contains("lending"));
    let mut borrow = transfer_between(ann, AccountRef::Store(store), usd(), "10");
    if let Transaction::Transfer(inner) = &mut borrow {
        inner.debt = Some(DebtEffect::Borrow);
    }
    vault.add_transaction(borrow).unwrap();
}

/// §8.1: the first matching active rule fills empty categories only.
#[test]
fn rules_fill_empty_categories_in_order() {
    let (_dir, mut vault) = setup();
    let store = vault.doc.stores[1].id.clone();
    let late = vault
        .create_categorization_rule("такси", "transport")
        .unwrap();
    vault.create_categorization_rule("ТАКСИ", "taxi").unwrap();
    // the second rule should run first
    let ids: Vec<String> = vault
        .categorization_rules()
        .into_iter()
        .rev()
        .map(|rule| rule.id)
        .collect();
    vault.reorder_categorization_rules(&ids).unwrap();

    let mut ride = payment(&store, "300", "Яндекс Такси до дома");
    ride.meta_mut().categories.clear();
    let stored = vault.add_transaction(ride).unwrap();
    assert_eq!(stored.meta().categories, vec!["taxi"]);

    // a category the user chose is never replaced
    let chosen = vault
        .add_transaction(payment(&store, "300", "такси"))
        .unwrap();
    assert_eq!(chosen.meta().categories, vec!["groceries"]);

    // an inactive rule does nothing
    vault
        .update_categorization_rule(&late.id, "такси", "transport", false)
        .unwrap();
    assert_eq!(vault.categorization_rules().len(), 2);
}

/// V16 (review item 7.3): a wallet the crypto module reads takes no
/// hand-made entry, whichever way it is reached — the engine writes its
/// operations, and a second hand-kept ledger beside them counts the same
/// money twice. The interface hides the actions; this is the rule itself.
#[test]
fn a_wallet_the_crypto_module_reads_takes_no_hand_made_entry() {
    let (_dir, mut vault) = empty_vault();
    let cash = store(&mut vault, "Cash", StoreKind::Cash, None);
    let followed = store(&mut vault, "Cold", StoreKind::Crypto, None);
    vault
        .doc
        .stores
        .iter_mut()
        .find(|have| have.id == followed)
        .expect("the wallet was created")
        .crypto_account_id = Some("cacc-1".into());

    assert!(rejected(vault.add_transaction(income(&followed, "1", "x"))).contains("crypto module"));
    assert!(
        rejected(vault.add_transaction(payment(&followed, "1", "x"))).contains("crypto module")
    );
    // Reached as the other side of a transfer from an ordinary wallet.
    let into = transfer_between(
        AccountRef::Store(cash.clone()),
        AccountRef::Store(followed.clone()),
        usd(),
        "1",
    );
    assert!(rejected(vault.add_transaction(into)).contains("crypto module"));
    let out_of = transfer_between(
        AccountRef::Store(followed.clone()),
        AccountRef::Store(cash.clone()),
        usd(),
        "1",
    );
    assert!(rejected(vault.add_transaction(out_of)).contains("crypto module"));

    // An ordinary wallet is not affected, and nothing was written above.
    vault.add_transaction(income(&cash, "1", "x")).unwrap();
    assert_eq!(vault.doc.transactions.len(), 1);
}

/// Mid-2026: after every timestamp the helpers write.
const NOW_MS: i64 = 1_800_000_000_000;

#[test]
fn the_rules_run_on_a_bare_document_and_the_delegate_agrees() {
    let (_dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    let good = payment(&card, "50", "coffee");
    let mut empty = payment(&card, "50", "coffee");
    empty.meta_mut().categories.clear();
    for tx in [&good, &empty] {
        let pure = super::validate::validate_transaction_at(&vault.doc, tx, NOW_MS)
            .map_err(|err| err.to_string());
        let delegate = vault
            .validate_transaction(tx)
            .map_err(|err| err.to_string());
        assert_eq!(pure, delegate);
    }
    assert!(super::validate::validate_transaction_at(&vault.doc, &empty, NOW_MS).is_err());
}

#[test]
fn category_keys_are_trimmed_lowercased_and_deduplicated() {
    let (_dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    let mut tx = payment(&card, "5", "coffee");
    tx.meta_mut().categories = vec![" Coffee ".into(), "coffee".into(), "  ".into()];
    super::validate::normalize_categories(&mut tx);
    assert_eq!(tx.meta().categories, vec!["coffee"]);
    assert_eq!(
        crate::service::UnlockedVault::store_sides(&tx),
        vec![card.as_str()]
    );
}
