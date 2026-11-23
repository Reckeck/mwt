//! Tests of `cryptotrack::service::wipe` that need the fixture document of the facade (`full_doc`);
//! moved here from the crate with their assertions unchanged (crate-split T21).

use crate::cryptotrack::service::wipe::clear_module;
use crate::model::fixtures::full_doc;
use crate::model::store::{Store, StoreKind};
use crate::model::vault::ExchangeKeys;

#[test]
fn clearing_the_module_empties_it_and_archives_the_wallets_it_led() {
    let mut doc = full_doc();
    let mut led = Store::new("Cold", StoreKind::Crypto, "#000");
    led.crypto_account_id = Some("cacc-1".into());
    let led_id = led.id.clone();
    doc.stores.push(led);

    clear_module(&mut doc, false);

    assert!(doc.crypto_accounts.is_empty() && doc.crypto_events.is_empty());
    assert!(doc.crypto_overrides.is_empty() && doc.crypto_tokens.is_empty());
    assert!(doc.crypto_positions.is_empty() && doc.crypto_snapshots.is_empty());
    assert!(doc.crypto_cursors.is_empty() && doc.crypto_prices.is_empty());
    let store = doc.stores.iter().find(|store| store.id == led_id).unwrap();
    assert!(store.archived && store.crypto_account_id.is_none());
}

#[test]
fn the_exchange_index_goes_only_when_it_is_the_device_files() {
    let mut doc = full_doc();
    doc.exchanges.bybit.push(ExchangeKeys {
        store_id: "wallet".into(),
        api_key: "abcd1234wxyz".into(),
        api_secret: "test-secret-no-real-key".into(),
        spot_store_id: None,
        futures_store_id: None,
    });
    let index = doc.exchanges.bybit.len();
    clear_module(&mut doc, false);
    assert_eq!(
        doc.exchanges.bybit.len(),
        index,
        "the user's own old keys stay"
    );
    clear_module(&mut doc, true);
    assert!(doc.exchanges.bybit.is_empty());
}
