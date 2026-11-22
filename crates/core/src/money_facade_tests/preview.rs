//! Tests of the preview area that need the vault (`UnlockedVault`): moved from
//! `money/preview_tests.rs` with their assertions unchanged (crate-split T13).

use crate::model::store::{AssetRef, StoreKind};
use crate::model::transaction::Transaction;
use crate::money::preview::*;
use crate::money::worth::FlowClass;
use crate::service::test_support::{amount, income, payment, store};

#[test]
fn shows_before_and_after_without_writing() {
    let (_dir, mut vault) = crate::service::test_support::empty_vault();
    let cash = store(&mut vault, "Wallet", StoreKind::Cash, None);
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        income(&cash, "100", "salary"),
    )
    .unwrap();
    let count = vault.doc.transactions.len();

    let preview = crate::money::preview::preview_transaction(
        &vault.books(),
        payment(&cash, "30", "food"),
        None,
    );
    assert_eq!(preview.error, None);
    assert_eq!(preview.changes.len(), 1);
    assert_eq!(preview.changes[0].before, amount("100"));
    assert_eq!(preview.changes[0].after, amount("70"));
    assert!(preview.warnings.is_empty());
    assert_eq!(preview.flow, FlowClass::Daily);
    assert_eq!(vault.doc.transactions.len(), count);
}

#[test]
fn a_sale_shows_the_book_value_and_the_gain() {
    use crate::model::transaction::{AssetSellTx, KindDetails, TopupTx, TxMeta};
    use crate::service::test_support::{meta, usd};
    let (_dir, mut vault) = crate::service::test_support::empty_vault();
    vault.set_base_currency("usd");
    let things = store(&mut vault, "Things", StoreKind::Property, None);
    let car = AssetRef {
        symbol: "CAR".into(),
        name: "Car".into(),
        coingecko_id: None,
        chain: None,
        contract: None,
        quote: "CAR".into(),
    };
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        Transaction::Topup(TopupTx {
            meta: meta("bought", &["income_other"]),
            store_id: things.clone(),
            asset: car.clone(),
            amount: amount("1"),
            fee: None,
            from: None,
            details: KindDetails {
                unit_price: Some(amount("10000")),
                ..KindDetails::default()
            },
            price: Default::default(),
        }),
    )
    .unwrap();
    let sell = Transaction::AssetSell(AssetSellTx {
        meta: TxMeta::new("car"),
        store_id: things,
        asset: car,
        unit_price: amount("12000"),
        quantity: amount("1"),
        currency: usd(),
        proceeds_to: None,
        fee: None,
        price: Default::default(),
    });
    let sale = vault.sale_preview(&sell).unwrap();
    assert_eq!(sale.currency, "usd");
    assert_eq!(sale.book_value, amount("10000"));
    assert_eq!(sale.gain, amount("2000"));
    assert_eq!(vault.sale_preview(&payment("cash", "1", "food")), None);
}

#[test]
fn warns_about_going_negative() {
    let (_dir, mut vault) = crate::service::test_support::empty_vault();
    let cash = store(&mut vault, "Wallet", StoreKind::Cash, None);
    let preview = crate::money::preview::preview_transaction(
        &vault.books(),
        payment(&cash, "5", "food"),
        None,
    );
    assert!(matches!(
        preview.warnings.as_slice(),
        [PreviewWarning::NegativeBalance { .. }]
    ));
}

#[test]
fn edit_shows_the_net_change() {
    let (_dir, mut vault) = crate::service::test_support::empty_vault();
    let cash = store(&mut vault, "Wallet", StoreKind::Cash, None);
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        income(&cash, "100", "salary"),
    )
    .unwrap();
    let spent = crate::usecases::money::transactions::add_transaction(
        &mut vault,
        payment(&cash, "30", "food"),
    )
    .unwrap();

    let mut edited = payment(&cash, "50", "food");
    edited.meta_mut().id = spent.meta().id.clone();
    let preview =
        crate::money::preview::preview_transaction(&vault.books(), edited, Some(&spent.meta().id));
    assert_eq!(preview.changes[0].before, amount("70"));
    assert_eq!(preview.changes[0].after, amount("50"));
}

#[test]
fn invalid_draft_reports_the_error() {
    let (_dir, vault) = crate::service::test_support::empty_vault();
    let preview = crate::money::preview::preview_transaction(
        &vault.books(),
        payment("missing", "5", "food"),
        None,
    );
    assert!(preview.error.is_some());
}

/// A preview is a query over the books: the document and the revision are as
/// they were, whether the draft is accepted or refused.
#[test]
fn a_preview_over_the_books_writes_nothing() {
    let (_dir, mut vault) = crate::service::test_support::empty_vault();
    let cash = store(&mut vault, "Wallet", StoreKind::Cash, None);
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        income(&cash, "100", "salary"),
    )
    .unwrap();
    let before = serde_json::to_string(&vault.doc).unwrap();
    let commits = super::bulk::Commits::watch(&mut vault);

    let accepted = preview_transaction(&vault.books(), payment(&cash, "30", "food"), None);
    assert_eq!(accepted.error, None);
    let refused = preview_transaction(&vault.books(), payment("nowhere", "30", "food"), None);
    assert!(refused.error.is_some());

    assert_eq!(serde_json::to_string(&vault.doc).unwrap(), before);
    assert_eq!(commits.count(), 0);
    assert!(!vault.has_unsaved_changes());
}

#[test]
fn the_free_function_and_the_delegate_agree() {
    let (_dir, mut vault) = crate::service::test_support::empty_vault();
    let cash = store(&mut vault, "Wallet", StoreKind::Cash, None);
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        income(&cash, "100", "salary"),
    )
    .unwrap();
    let draft = payment(&cash, "30", "food");
    assert_eq!(
        preview_transaction(&vault.books(), draft.clone(), None),
        crate::money::preview::preview_transaction(&vault.books(), draft, None)
    );
}
