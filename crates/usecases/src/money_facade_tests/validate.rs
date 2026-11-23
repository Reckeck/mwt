//! Tests of the validate area that need the vault (`Session`): moved from
//! `money/validate_tests.rs` with their assertions unchanged (crate-split T13).

use crate::household::contacts::ContactDraft;
use crate::model::chain::Chain;
use crate::model::contact::{ContactAccount, ContactKind, DetailKind};
use crate::model::store::{AssetRef, StoreKind};
use crate::model::transaction::*;
use crate::service::test_support::*;

fn rejected(result: crate::Result<Transaction>) -> String {
    match result {
        Err(err) => err.to_string(),
        Ok(tx) => panic!("expected a rejection, got {}", tx.tag()),
    }
}

/// Mid-2026: after every timestamp the helpers write.
const NOW_MS: i64 = 1_800_000_000_000;

/// V2: a kind the store does not accept.
#[test]
fn a_kind_must_fit_the_store() {
    let (_dir, mut vault) = setup();
    let cold = vault.doc.stores[2].id.clone();
    assert!(
        rejected(crate::usecases::money::transactions::add_transaction(
            &mut vault,
            trade(&cold, "1", "1", "x")
        ))
        .contains("not possible")
    );
    let flat = store(&mut vault, "Flat", StoreKind::Property, None);
    assert!(
        rejected(crate::usecases::money::transactions::add_transaction(
            &mut vault,
            payment(&flat, "1", "x")
        ))
        .contains("not possible")
    );
}

/// V3: cash and bank accounts hold fiat only.
#[test]
fn a_bank_holds_only_fiat() {
    let (_dir, mut vault) = empty_vault();
    let bank = store(&mut vault, "Bank", StoreKind::Bank, None);
    assert!(
        rejected(crate::usecases::money::transactions::add_transaction(
            &mut vault,
            income_of(&bank, btc(), "1", "1", "x")
        ))
        .contains("only fiat")
    );
    crate::usecases::money::transactions::add_transaction(&mut vault, income(&bank, "1", "x"))
        .unwrap();
}

/// V5, V12: topups and payments need a category of a matching group.
#[test]
fn categories_must_fit_the_verb() {
    let (_dir, mut vault) = setup();
    let store = vault.doc.stores[1].id.clone();
    let mut bare = payment(&store, "1", "x");
    bare.meta_mut().categories.clear();
    assert!(
        rejected(crate::usecases::money::transactions::add_transaction(
            &mut vault, bare
        ))
        .contains("choose a category")
    );

    let mut wrong = income(&store, "1", "x");
    wrong.meta_mut().categories = vec!["groceries".into()];
    assert!(
        rejected(crate::usecases::money::transactions::add_transaction(
            &mut vault, wrong
        ))
        .contains("does not fit")
    );

    crate::usecases::household::categories::archive_category(&mut vault, "groceries", true)
        .unwrap();
    assert!(
        rejected(crate::usecases::money::transactions::add_transaction(
            &mut vault,
            payment(&store, "1", "x")
        ))
        .contains("archived")
    );
}

/// V6: a transfer needs one of the user's stores and two different sides.
#[test]
fn transfers_need_a_store_and_two_sides() {
    let (_dir, mut vault) = setup();
    let raw = |text: &str| AccountRef::Raw(text.into());
    assert!(
        rejected(crate::usecases::money::transactions::add_transaction(
            &mut vault,
            transfer_between(raw("a"), raw("b"), usd(), "1")
        ))
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
    assert!(
        rejected(crate::usecases::money::transactions::add_transaction(
            &mut vault,
            convert.clone()
        ))
        .contains("two different assets")
    );
    if let Transaction::Convert(inner) = &mut convert {
        inner.to.asset = AssetRef::fiat("EUR", "Euro");
    }
    crate::usecases::money::transactions::add_transaction(&mut vault, convert).unwrap();
}

/// V7: a contact account must belong to the contact and be active.
#[test]
fn contact_accounts_are_checked() {
    let (_dir, mut vault) = setup();
    let store = vault.doc.stores[1].id.clone();
    let contact = crate::usecases::household::contacts::create_contact(
        &mut vault,
        ContactDraft {
            group: String::new(),
            name: "Mom".into(),
            kind: ContactKind::Person,
            note: String::new(),
            accounts: vec![ContactAccount {
                value: "4276 0000".into(),
                kind: DetailKind::Card,
                ..ContactAccount::default()
            }],
        },
    )
    .unwrap();
    let account_id = contact.accounts[0].id.clone();
    let to = |account: &str| AccountRef::Contact {
        contact_id: contact.id.clone(),
        account_id: Some(account.into()),
    };
    let send = |to: AccountRef| transfer_between(AccountRef::Store(store.clone()), to, usd(), "5");
    assert!(
        rejected(crate::usecases::money::transactions::add_transaction(
            &mut vault,
            send(to("other"))
        ))
        .contains("does not belong")
    );
    crate::usecases::money::transactions::add_transaction(&mut vault, send(to(&account_id)))
        .unwrap();
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
    assert!(
        rejected(crate::usecases::money::transactions::add_transaction(
            &mut vault,
            Transaction::Topup(topup)
        ))
        .contains("too large")
    );
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
    assert!(
        rejected(crate::usecases::money::transactions::add_transaction(
            &mut vault,
            Transaction::Topup(topup.clone())
        ))
        .contains("add up")
    );
    topup.details.cash_breakdown = Some(vec![CashNote {
        denomination: amount("100"),
        count: 3,
    }]);
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        Transaction::Topup(topup.clone()),
    )
    .unwrap();
    topup.details.network = Some(Chain::Bitcoin);
    assert!(
        rejected(crate::usecases::money::transactions::add_transaction(
            &mut vault,
            Transaction::Topup(topup)
        ))
        .contains("crypto wallets")
    );
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
        rejected(crate::usecases::money::transactions::add_transaction(
            &mut vault,
            futures(FuturesAction::Open, Some("5"))
        ))
        .contains("closing")
    );
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        futures(FuturesAction::Open, None),
    )
    .unwrap();
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        futures(FuturesAction::Close, Some("5")),
    )
    .unwrap();
}

/// V11: a date too far ahead, or not a date at all.
#[test]
fn dates_are_checked() {
    let (_dir, mut vault) = setup();
    let store = vault.doc.stores[1].id.clone();
    let mut later = income(&store, "1", "x");
    at(&mut later, "2999-01-01T00:00:00Z");
    assert!(
        rejected(crate::usecases::money::transactions::add_transaction(
            &mut vault, later
        ))
        .contains("future")
    );
    let mut garbage = income(&store, "1", "x");
    at(&mut garbage, "yesterday");
    assert!(
        rejected(crate::usecases::money::transactions::add_transaction(
            &mut vault, garbage
        ))
        .contains("RFC3339")
    );
}

/// V14: the debt direction must match the sides.
#[test]
fn debt_direction_must_match_the_sides() {
    let (_dir, mut vault) = setup();
    let store = vault.doc.stores[1].id.clone();
    let contact = crate::usecases::household::contacts::create_contact(
        &mut vault,
        ContactDraft {
            name: "Ann".into(),
            ..ContactDraft::default()
        },
    )
    .unwrap();
    let ann = AccountRef::Contact {
        contact_id: contact.id,
        account_id: None,
    };
    let mut lend = transfer_between(ann.clone(), AccountRef::Store(store.clone()), usd(), "10");
    if let Transaction::Transfer(inner) = &mut lend {
        inner.debt = Some(DebtEffect::Lend);
    }
    assert!(
        rejected(crate::usecases::money::transactions::add_transaction(
            &mut vault, lend
        ))
        .contains("lending")
    );
    let mut borrow = transfer_between(ann, AccountRef::Store(store), usd(), "10");
    if let Transaction::Transfer(inner) = &mut borrow {
        inner.debt = Some(DebtEffect::Borrow);
    }
    crate::usecases::money::transactions::add_transaction(&mut vault, borrow).unwrap();
}

/// §8.1: the first matching active rule fills empty categories only.
#[test]
fn rules_fill_empty_categories_in_order() {
    let (_dir, mut vault) = setup();
    let store = vault.doc.stores[1].id.clone();
    let late = crate::usecases::household::categories::create_categorization_rule(
        &mut vault,
        "такси",
        "transport",
    )
    .unwrap();
    crate::usecases::household::categories::create_categorization_rule(&mut vault, "ТАКСИ", "taxi")
        .unwrap();
    // the second rule should run first
    let ids: Vec<String> = crate::money::categories::categorization_rules(&vault.doc)
        .into_iter()
        .rev()
        .map(|rule| rule.id)
        .collect();
    crate::usecases::household::categories::reorder_categorization_rules(&mut vault, &ids).unwrap();

    let mut ride = payment(&store, "300", "Яндекс Такси до дома");
    ride.meta_mut().categories.clear();
    let stored = crate::usecases::money::transactions::add_transaction(&mut vault, ride).unwrap();
    assert_eq!(stored.meta().categories, vec!["taxi"]);

    // a category the user chose is never replaced
    let chosen = crate::usecases::money::transactions::add_transaction(
        &mut vault,
        payment(&store, "300", "такси"),
    )
    .unwrap();
    assert_eq!(chosen.meta().categories, vec!["groceries"]);

    // an inactive rule does nothing
    crate::usecases::household::categories::update_categorization_rule(
        &mut vault,
        &late.id,
        "такси",
        "transport",
        false,
    )
    .unwrap();
    assert_eq!(
        crate::money::categories::categorization_rules(&vault.doc).len(),
        2
    );
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

    assert!(
        rejected(crate::usecases::money::transactions::add_transaction(
            &mut vault,
            income(&followed, "1", "x")
        ))
        .contains("crypto module")
    );
    assert!(
        rejected(crate::usecases::money::transactions::add_transaction(
            &mut vault,
            payment(&followed, "1", "x")
        ))
        .contains("crypto module")
    );
    // Reached as the other side of a transfer from an ordinary wallet.
    let into = transfer_between(
        AccountRef::Store(cash.clone()),
        AccountRef::Store(followed.clone()),
        usd(),
        "1",
    );
    assert!(
        rejected(crate::usecases::money::transactions::add_transaction(
            &mut vault, into
        ))
        .contains("crypto module")
    );
    let out_of = transfer_between(
        AccountRef::Store(followed.clone()),
        AccountRef::Store(cash.clone()),
        usd(),
        "1",
    );
    assert!(
        rejected(crate::usecases::money::transactions::add_transaction(
            &mut vault, out_of
        ))
        .contains("crypto module")
    );

    // An ordinary wallet is not affected, and nothing was written above.
    crate::usecases::money::transactions::add_transaction(&mut vault, income(&cash, "1", "x"))
        .unwrap();
    assert_eq!(vault.doc.transactions.len(), 1);
}

#[test]
fn the_rules_run_on_a_bare_document_and_the_delegate_agrees() {
    let (_dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    let good = payment(&card, "50", "coffee");
    let mut empty = payment(&card, "50", "coffee");
    empty.meta_mut().categories.clear();
    for tx in [&good, &empty] {
        let pure = crate::money::validate::validate_transaction_at(&vault.doc, tx, NOW_MS)
            .map_err(|err| err.to_string());
        let delegate = crate::usecases::household::validate::validate_transaction(&vault, tx)
            .map_err(|err| err.to_string());
        assert_eq!(pure, delegate);
    }
    assert!(crate::money::validate::validate_transaction_at(&vault.doc, &empty, NOW_MS).is_err());
}

#[test]
fn category_keys_are_trimmed_lowercased_and_deduplicated() {
    let (_dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    let mut tx = payment(&card, "5", "coffee");
    tx.meta_mut().categories = vec![" Coffee ".into(), "coffee".into(), "  ".into()];
    crate::money::validate::normalize_categories(&mut tx);
    assert_eq!(tx.meta().categories, vec!["coffee"]);
    assert_eq!(
        crate::service::Session::store_sides(&tx),
        vec![card.as_str()]
    );
}
