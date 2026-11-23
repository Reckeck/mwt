use crate::Amount;
use crate::ipc_contract::{AttachPaymentInput, SharedPurchaseDraft};
use crate::ipc_contract::{SharedEventDraft, SharedItemDraft, SharedParticipantDraft};
use crate::model::shared_event::PriceMode;
use crate::model::shared_event::{ItemPlan, SharedEventStatus};
use crate::model::store::AssetRef;
use crate::model::store::StoreKind;
use crate::model::transaction::{PaymentTx, Transaction};
use crate::service::Session;
use crate::service::test_support::*;

struct Setup {
    _dir: tempfile::TempDir,
    vault: Session,
    wallet: String,
    event: String,
    petya: String,
}

fn rub() -> AssetRef {
    AssetRef::fiat("RUB", "RUB")
}

fn setup_event() -> Setup {
    let (dir, mut vault) = empty_vault();
    vault.set_base_currency("rub");
    let wallet = store(&mut vault, "Тинькофф", StoreKind::Bank, None);
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        income_of(&wallet, rub(), "50000", "0.0109", "salary"),
    )
    .unwrap();
    let view = crate::usecases::household::shared_events::create_shared_event(
        &mut vault,
        SharedEventDraft {
            title: "Шашлыки".into(),
            currency: "rub".into(),
            participants: vec![SharedParticipantDraft {
                name: "Петя".into(),
                ..SharedParticipantDraft::default()
            }],
            ..SharedEventDraft::default()
        },
    )
    .unwrap();
    let petya = view.people[1].id.clone();
    Setup {
        _dir: dir,
        vault,
        wallet,
        event: view.event.id,
        petya,
    }
}

fn buy(setup: &Setup, item: Option<&str>, buyer: &str, total: &str) -> SharedPurchaseDraft {
    SharedPurchaseDraft {
        item_id: item.map(str::to_string),
        new_item_title: Some("Уголь".into()),
        buyer: buyer.into(),
        qty: amount("1"),
        amount: amount(total),
        currency: "rub".into(),
        price_mode: PriceMode::Total,
        store_id: Some(setup.wallet.clone()),
        ..SharedPurchaseDraft::default()
    }
}

fn rub_balance(setup: &Setup) -> Amount {
    holding_of(&setup.vault, &setup.wallet, &rub())
}

#[test]
fn my_purchase_makes_a_payment_and_deleting_it_gives_the_money_back() {
    let mut setup = setup_event();
    let draft = buy(&setup, None, "", "1800");
    let view = crate::usecases::household::shared_events::add_shared_purchase(
        &mut setup.vault,
        &setup.event,
        draft,
    )
    .unwrap();
    assert_eq!(view.status, SharedEventStatus::InProgress);
    assert_eq!(view.totals.spent, amount("1800"));
    assert_eq!(rub_balance(&setup), amount("48200"));
    let purchase = view.event.items[0].purchases[0].clone();
    assert!(purchase.tx_owned);
    let tx_id = purchase.tx_id.clone().unwrap();
    let links = crate::household::shared_events::view::shared_tx_links(&setup.vault.doc);
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].tx_id, tx_id);
    assert_eq!(links[0].link.event_id, setup.event);
    assert_eq!(
        links[0].link.purchase_id.as_deref(),
        Some(purchase.id.as_str())
    );
    assert_eq!(view.purchases[0].my_part, amount("900"));

    let view = crate::usecases::household::shared_events::delete_shared_purchase(
        &mut setup.vault,
        &setup.event,
        &purchase.id,
    )
    .unwrap();
    assert_eq!(view.totals.spent, amount("0"));
    assert_eq!(rub_balance(&setup), amount("50000"));
    assert!(
        setup
            .vault
            .doc
            .transactions
            .iter()
            .all(|tx| tx.meta().id != tx_id)
    );
}

#[test]
fn someone_elses_purchase_leaves_my_wallets_alone() {
    let mut setup = setup_event();
    let before = setup.vault.doc.transactions.len();
    let draft = buy(&setup, None, &setup.petya.clone(), "500");
    let view = crate::usecases::household::shared_events::add_shared_purchase(
        &mut setup.vault,
        &setup.event,
        draft,
    )
    .unwrap();
    assert_eq!(setup.vault.doc.transactions.len(), before);
    assert_eq!(rub_balance(&setup), amount("50000"));
    assert!(view.event.items[0].purchases[0].tx_id.is_none());
    // Petya paid 500, half of it is mine: I owe him 250
    let petya = view
        .people
        .iter()
        .find(|row| row.id == setup.petya)
        .unwrap();
    assert_eq!(petya.saldo, amount("-250"));
}

#[test]
fn a_refused_payment_leaves_the_event_untouched() {
    let mut setup = setup_event();
    let mut draft = buy(&setup, None, "", "100");
    draft.store_id = Some("no-such-wallet".into());
    let result = crate::usecases::household::shared_events::add_shared_purchase(
        &mut setup.vault,
        &setup.event,
        draft,
    );
    assert!(result.is_err());
    let view =
        crate::household::shared_events::view::shared_event_view(&setup.vault.doc, &setup.event)
            .unwrap();
    assert!(view.event.items.is_empty());
    // I cannot buy without a wallet
    let mut no_wallet = buy(&setup, None, "", "100");
    no_wallet.store_id = None;
    assert!(
        crate::usecases::household::shared_events::add_shared_purchase(
            &mut setup.vault,
            &setup.event,
            no_wallet
        )
        .is_err()
    );
}

#[test]
fn a_purchase_in_another_currency_gets_a_rate_from_the_cache_or_by_hand() {
    let mut setup = setup_event();
    let mut draft = buy(&setup, None, &setup.petya.clone(), "10");
    draft.currency = "usd".into();
    // no rate known for usd -> rub
    let err = crate::usecases::household::shared_events::add_shared_purchase(
        &mut setup.vault,
        &setup.event,
        draft.clone(),
    )
    .unwrap_err();
    assert_eq!(err.code(), "FX_MISSING");
    // by hand
    draft.rate_to_event = Some(amount("91.37"));
    let view = crate::usecases::household::shared_events::add_shared_purchase(
        &mut setup.vault,
        &setup.event,
        draft,
    )
    .unwrap();
    let purchase = &view.event.items[0].purchases[0];
    assert!(purchase.rate_manual);
    assert_eq!(view.totals.spent, amount("913.70"));
    // from the cache: 1 USD = 90 RUB
    setup.vault.doc.fx_cache.push(crate::model::vault::FxPoint {
        currency: "rub".into(),
        per_usd: amount("90"),
        ts: "2000-01-01T00:00:00Z".into(),
        source: "test".into(),
    });
    let mut cached = buy(&setup, None, &setup.petya.clone(), "10");
    cached.currency = "usd".into();
    let view = crate::usecases::household::shared_events::add_shared_purchase(
        &mut setup.vault,
        &setup.event,
        cached,
    )
    .unwrap();
    let second = view.event.items[1].purchases[0].clone();
    assert!(!second.rate_manual);
    assert_eq!(second.rate_to_event, amount("90"));
    // changing the cache afterwards does not rewrite the purchase
    setup.vault.doc.fx_cache[0].per_usd = amount("100");
    let again =
        crate::household::shared_events::view::shared_event_view(&setup.vault.doc, &setup.event)
            .unwrap();
    assert_eq!(again.totals.spent, amount("1813.70"));
}

#[test]
fn editing_a_purchase_follows_into_the_payment() {
    let mut setup = setup_event();
    let draft = buy(&setup, None, "", "1000");
    let view = crate::usecases::household::shared_events::add_shared_purchase(
        &mut setup.vault,
        &setup.event,
        draft,
    )
    .unwrap();
    let purchase = view.event.items[0].purchases[0].clone();
    let mut edit = buy(&setup, None, "", "1500");
    edit.new_item_title = None;
    let view = crate::usecases::household::shared_events::update_shared_purchase(
        &mut setup.vault,
        &setup.event,
        &purchase.id,
        edit,
    )
    .unwrap();
    assert_eq!(view.totals.spent, amount("1500"));
    assert_eq!(rub_balance(&setup), amount("48500"));
    // the payment keeps its id, so the link survives the edit
    assert_eq!(
        crate::household::shared_events::view::shared_tx_links(&setup.vault.doc)[0].tx_id,
        purchase.tx_id.clone().unwrap()
    );
    // handing the purchase to Petya takes my payment away
    let mut handed = buy(&setup, None, &setup.petya.clone(), "1500");
    handed.new_item_title = None;
    let view = crate::usecases::household::shared_events::update_shared_purchase(
        &mut setup.vault,
        &setup.event,
        &purchase.id,
        handed,
    )
    .unwrap();
    assert_eq!(rub_balance(&setup), amount("50000"));
    assert!(view.event.items[0].purchases[0].tx_id.is_none());
}

fn payment_rub(store: &str, raw: &str) -> Transaction {
    match payment(store, raw, "shop") {
        Transaction::Payment(inner) => Transaction::Payment(PaymentTx {
            asset: rub(),
            ..inner
        }),
        other => other,
    }
}

#[test]
fn an_existing_payment_is_adopted_and_only_unlinked_on_delete() {
    let mut setup = setup_event();
    let payment = crate::usecases::money::transactions::add_transaction(
        &mut setup.vault,
        payment_rub(&setup.wallet, "700"),
    )
    .unwrap();
    let tx_id = payment.meta().id.clone();
    let attach = |setup: &mut Setup| {
        crate::usecases::household::shared_events::attach_payment_to_shared_event(
            &mut setup.vault,
            AttachPaymentInput {
                event_id: setup.event.clone(),
                tx_id: tx_id.clone(),
                new_item_title: Some("Мангал".into()),
                ..AttachPaymentInput::default()
            },
        )
    };
    let view = attach(&mut setup).unwrap();
    assert_eq!(view.totals.spent, amount("700"));
    assert!(!view.event.items[0].purchases[0].tx_owned);
    // the same payment cannot be taken twice
    assert!(attach(&mut setup).is_err());
    let purchase_id = view.event.items[0].purchases[0].id.clone();
    crate::usecases::household::shared_events::delete_shared_purchase(
        &mut setup.vault,
        &setup.event,
        &purchase_id,
    )
    .unwrap();
    assert!(crate::household::shared_events::view::shared_tx_links(&setup.vault.doc).is_empty());
    assert!(
        setup
            .vault
            .doc
            .transactions
            .iter()
            .any(|tx| tx.meta().id == tx_id),
        "the payment is still there"
    );
    // deleting the event with a linked payment keeps it too
    attach(&mut setup).unwrap();
    crate::usecases::household::shared_events::delete_shared_event(&mut setup.vault, &setup.event)
        .unwrap();
    assert!(
        setup
            .vault
            .doc
            .transactions
            .iter()
            .any(|tx| tx.meta().id == tx_id)
    );
}

#[test]
fn deleting_the_event_keeps_every_real_operation() {
    let mut setup = setup_event();
    let draft = buy(&setup, None, "", "1200");
    crate::usecases::household::shared_events::add_shared_purchase(
        &mut setup.vault,
        &setup.event,
        draft,
    )
    .unwrap();
    assert_eq!(rub_balance(&setup), amount("48800"));
    crate::usecases::household::shared_events::delete_shared_event(&mut setup.vault, &setup.event)
        .unwrap();
    assert_eq!(rub_balance(&setup), amount("48800"));
    assert!(crate::household::shared_events::view::shared_tx_links(&setup.vault.doc).is_empty());
}

#[test]
fn totals_compare_the_plan_with_what_was_bought() {
    let mut setup = setup_event();
    let item = crate::usecases::household::shared_events::add_shared_item(
        &mut setup.vault,
        &setup.event,
        SharedItemDraft {
            title: "Пиво".into(),
            plan: Some(ItemPlan {
                qty: amount("6"),
                price: amount("1200"),
                price_mode: PriceMode::Total,
            }),
            ..SharedItemDraft::default()
        },
    )
    .unwrap()
    .event
    .items[0]
        .id
        .clone();
    // Not bought yet: in the plan, but not in the difference.
    crate::usecases::household::shared_events::add_shared_item(
        &mut setup.vault,
        &setup.event,
        SharedItemDraft {
            title: "Соус".into(),
            plan: Some(ItemPlan {
                qty: amount("2"),
                price: amount("400"),
                price_mode: PriceMode::Total,
            }),
            ..SharedItemDraft::default()
        },
    )
    .unwrap();
    let mut draft = buy(&setup, Some(&item), "", "1800");
    draft.qty = amount("8");
    crate::usecases::household::shared_events::add_shared_purchase(
        &mut setup.vault,
        &setup.event,
        draft,
    )
    .unwrap();
    // Off the plan: spent, but not in the difference either.
    let hoisted = buy(&setup, None, "", "500");
    let view = crate::usecases::household::shared_events::add_shared_purchase(
        &mut setup.vault,
        &setup.event,
        hoisted,
    )
    .unwrap();
    assert_eq!(view.totals.planned, amount("1600"));
    assert_eq!(view.totals.spent, amount("2300"));
    assert_eq!(view.totals.diff, amount("600"));
    assert_eq!(view.items[0].diff_money, amount("600"));
    assert_eq!(view.items[0].diff_qty, amount("2"));
}
