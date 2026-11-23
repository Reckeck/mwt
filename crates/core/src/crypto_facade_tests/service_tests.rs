//! The service rules of the module on a bare `VaultDoc` and, with the vault, one test per mutating method family that the vault's method does what the
//! scenario in `usecases::cryptotrack` does (spec core-strangler, Tests).

use crate::cryptotrack::service::apply::{
    cursor_of, has_account, next_list_after, protocol_memory, remember_protocols, reset_cursor,
    restore_cursor, set_cursor, store_prices,
};
use crate::cryptotrack::service::operations::{
    crypto_account_name, crypto_event_ref, crypto_join_chain, crypto_release_chain, crypto_symbol,
    day_of, native_symbol, other_side, secs, short,
};
use crate::cryptotrack::service::recompute::{
    CryptoBatch, batch_target, has_opening, store_adjustments,
};
use crate::cryptotrack::service::totals::{
    crypto_retired_wallets, holdings_differ, supersede_holdings,
};
use crate::cryptotrack::sources::evm::{Cursor, SourceBatch, Stream};
use crate::error::Error;
use crate::ipc_contract::cryptotrack::operations::CryptoSetOverrideInput;
use crate::ipc_contract::cryptotrack::sync::CryptoPages;
use crate::model::chain::Chain;
use crate::model::cryptotrack::{
    CryptoAccount, CryptoAsset, CryptoDirection, CryptoLeg, CryptoPricePoint, CryptoTx,
    CryptoTxKind, TrackingRef,
};
use crate::model::store::{AssetRef, Holding, Store, StoreKind};
use crate::model::vault::VaultDoc;
use crate::service::Session;
use crate::service::test_support::amount;
use crate::usecases::cryptotrack::{apply, operations, recompute};

const NOW: &str = "2026-11-10T00:00:00Z";

fn account() -> CryptoAccount {
    CryptoAccount::new(
        "acc-1",
        "Main",
        Chain::Ethereum,
        "0xbbbb000000000000000000000000000000000001",
        NOW,
    )
}

fn event(id: &str) -> CryptoTx {
    CryptoTx {
        id: id.into(),
        chain: Chain::Ethereum,
        account_id: "acc-1".into(),
        time: "2026-11-01T10:00:00Z".into(),
        block: Some(1),
        from: "0xaaaa".into(),
        to: "0xbbbb".into(),
        legs: vec![CryptoLeg {
            direction: CryptoDirection::In,
            asset: CryptoAsset::Native {
                chain: Chain::Ethereum,
            },
            amount: amount("1"),
            counterparty: None,
        }],
        gas: None,
        protocol: None,
        method: None,
        kind: CryptoTxKind::Transfer,
        raw: None,
        tracking: TrackingRef::ChainTx {
            chain: Chain::Ethereum,
            hash: format!("0x{}", "a".repeat(64)),
        },
    }
}

fn doc_with_account() -> VaultDoc {
    let mut doc = VaultDoc::default();
    doc.crypto_accounts.push(account());
    doc
}

fn holding(store_id: &str, quantity: &str) -> Holding {
    Holding {
        id: format!("h-{store_id}"),
        store_id: store_id.into(),
        asset: AssetRef::crypto("ETH", "Ether", None, Chain::Ethereum),
        quantity: amount(quantity),
        avg_cost: None,
    }
}

#[test]
fn cursors_are_kept_moved_and_forgotten() {
    let mut doc = doc_with_account();
    assert!(has_account(&doc, "acc-1"));
    assert!(!has_account(&doc, "nope"));
    assert!(cursor_of(&doc, "acc-1").is_none());
    assert!(set_cursor(&mut doc, "acc-1", Some("c1".into()), NOW));
    assert!(!set_cursor(&mut doc, "acc-1", Some("c1".into()), NOW));
    let before = cursor_of(&doc, "acc-1").cloned();
    assert!(set_cursor(&mut doc, "acc-1", Some("c2".into()), NOW));
    restore_cursor(&mut doc, "acc-1", before.clone());
    assert_eq!(cursor_of(&doc, "acc-1"), before.as_ref());
    assert!(reset_cursor(&mut doc, "acc-1"));
    assert!(!reset_cursor(&mut doc, "acc-1"));
}

#[test]
fn protocol_memory_is_remembered_and_forgotten() {
    let mut doc = doc_with_account();
    let memory = [("aave".to_string(), Some("block 7".to_string()))];
    assert!(remember_protocols(&mut doc, "acc-1", &memory, NOW));
    assert!(!remember_protocols(&mut doc, "acc-1", &memory, NOW));
    assert_eq!(protocol_memory(&doc, "acc-1")["aave"], "block 7");
    let forget = [("aave".to_string(), None)];
    assert!(remember_protocols(&mut doc, "acc-1", &forget, NOW));
    assert!(protocol_memory(&doc, "acc-1").is_empty());
}

#[test]
fn the_next_list_follows_a_history_and_never_the_tokens() {
    assert_eq!(next_list_after(None), None);
    let tokens = Cursor::at(Stream::Tokens, None).encode();
    assert_eq!(next_list_after(Some(&tokens)), None);
}

#[test]
fn prices_count_only_new_non_zero_points() {
    let mut doc = VaultDoc::default();
    let point = CryptoPricePoint {
        token_key: "native:ethereum".into(),
        value_usd: amount("2000"),
        ts: "2026-11-01T00:00:00Z".into(),
        source: "test".into(),
    };
    let zero = CryptoPricePoint {
        value_usd: amount("0"),
        ts: "2026-11-02T00:00:00Z".into(),
        ..point.clone()
    };
    assert_eq!(store_prices(&mut doc, &[point.clone(), zero]), 1);
    assert_eq!(store_prices(&mut doc, &[point]), 0);
}

#[test]
fn names_and_symbols_come_from_the_document_alone() {
    let mut doc = doc_with_account();
    assert_eq!(native_symbol(Chain::Bitcoin), "BTC");
    assert_eq!(short("0123456789abcdef"), "012345…cdef");
    assert_eq!(short("short"), "short");
    assert_eq!(secs("1970-01-01T00:00:10Z"), Some(10));
    assert_eq!(secs("garbage"), None);
    assert!(day_of("2026-11-01T10:00:00Z").is_some());
    assert_eq!(
        crypto_symbol(
            &doc,
            &CryptoAsset::Native {
                chain: Chain::Ethereum
            }
        ),
        "ETH"
    );
    assert_eq!(crypto_account_name(&doc, "acc-1"), "Main");
    assert_eq!(crypto_account_name(&doc, "nope"), "");
    assert_eq!(other_side(&event("e1")), "0xaaaa");
    doc.crypto_events.push(event("e1"));
    assert!(crypto_event_ref(&doc, "e1").is_ok());
    assert!(matches!(
        crypto_event_ref(&doc, "e2"),
        Err(Error::NotFound(_))
    ));
}

#[test]
fn joining_a_chain_needs_a_known_fiat_operation_and_release_is_quiet_without_one() {
    let mut doc = VaultDoc::default();
    assert!(matches!(
        crypto_join_chain(&mut doc, "missing", None),
        Err(Error::NotFound(_))
    ));
    crypto_release_chain(&mut doc, "flow-1", "e1");
}

#[test]
fn a_batch_lands_on_a_followed_account_only() {
    let doc = doc_with_account();
    let request = |account_id| CryptoBatch {
        account_id,
        read_at: NOW,
        start: None,
        history_done: true,
        balances_complete: true,
    };
    let target = batch_target(&doc, &request("acc-1")).unwrap();
    assert_eq!(target.chain, Chain::Ethereum);
    assert_eq!(target.start, NOW);
    assert!(matches!(
        batch_target(&doc, &request("nope")),
        Err(Error::NotFound(_))
    ));
}

#[test]
fn adjustments_are_stored_once() {
    let mut doc = VaultDoc::default();
    assert!(!has_opening(&doc, "acc-1", "native:ethereum"));
    assert!(store_adjustments(&mut doc, vec![event("e1")]));
    assert!(!store_adjustments(&mut doc, vec![event("e1")]));
    assert_eq!(doc.crypto_events.len(), 1);
}

#[test]
fn holdings_are_superseded_by_row_and_by_retired_wallet() {
    let mut doc = VaultDoc {
        holdings: vec![holding("a", "1"), holding("retired", "5")],
        ..Default::default()
    };
    let retired = ["retired".to_string()].into_iter().collect();
    supersede_holdings(&mut doc, vec![holding("a", "2")], &retired);
    assert_eq!(doc.holdings.len(), 1);
    assert_eq!(doc.holdings[0].quantity, amount("2"));
    assert!(!holdings_differ(&doc.holdings, &[holding("a", "2")]));
    assert!(holdings_differ(&doc.holdings, &[holding("a", "3")]));
}

#[test]
fn a_retired_wallet_is_an_archived_crypto_store_no_address_drives() {
    let mut doc = VaultDoc::default();
    let retired = Store {
        id: "old".into(),
        archived: true,
        kind: StoreKind::Crypto,
        ..Default::default()
    };
    let live = Store {
        id: "live".into(),
        archived: false,
        ..retired.clone()
    };
    doc.stores = vec![retired, live];
    let set = crypto_retired_wallets(&doc);
    assert!(set.contains("old"));
    assert!(!set.contains("live"));
}

fn twin() -> (Session, Session) {
    let mut doc = doc_with_account();
    doc.crypto_events.push(event("e1"));
    (Session::in_memory(doc.clone()), Session::in_memory(doc))
}

#[test]
fn apply_batch_delegates_to_the_scenario() {
    let (mut method, mut scenario) = twin();
    let request = CryptoBatch {
        account_id: "acc-1",
        read_at: NOW,
        start: None,
        history_done: false,
        balances_complete: false,
    };
    let batch = SourceBatch::default();
    let left =
        crate::usecases::cryptotrack::recompute::crypto_apply_batch(&mut method, &request, &batch)
            .unwrap();
    let right = recompute::crypto_apply_batch(&mut scenario, &request, &batch).unwrap();
    assert_eq!(left, right);
    assert_eq!(method.doc.crypto_events, scenario.doc.crypto_events);
    assert_eq!(method.doc.holdings.len(), scenario.doc.holdings.len());
}

#[test]
fn apply_pages_delegates_to_the_scenario() {
    let (mut method, mut scenario) = twin();
    let request = CryptoPages {
        account_id: "acc-1",
        read_at: NOW,
        finished: false,
        refresh: false,
    };
    let batch = SourceBatch::default();
    let left =
        crate::usecases::cryptotrack::apply::crypto_apply_pages(&mut method, &request, &batch)
            .unwrap();
    let right = apply::crypto_apply_pages(&mut scenario, &request, &batch).unwrap();
    assert_eq!(left, right);
    assert_eq!(
        cursor_of(&method.doc, "acc-1").map(|record| record.cursor.clone()),
        cursor_of(&scenario.doc, "acc-1").map(|record| record.cursor.clone())
    );
}

#[test]
fn sync_holdings_delegates_to_the_scenario() {
    let (mut method, mut scenario) = twin();
    crate::usecases::cryptotrack::recompute::crypto_sync_holdings(&mut method).unwrap();
    recompute::crypto_sync_holdings(&mut scenario).unwrap();
    assert_eq!(method.doc.holdings.len(), scenario.doc.holdings.len());
}

#[test]
fn set_override_delegates_to_the_scenario() {
    let (mut method, mut scenario) = twin();
    let input = CryptoSetOverrideInput {
        event_id: "e1".into(),
        hidden: Some(true),
        ..Default::default()
    };
    let left =
        crate::usecases::cryptotrack::operations::crypto_set_override(&mut method, &input).unwrap();
    let right = operations::crypto_set_override(&mut scenario, &input).unwrap();
    assert_eq!(left, right);
    assert_eq!(method.doc.crypto_overrides.len(), 1);
    assert_eq!(
        method.doc.crypto_overrides[0].hidden,
        scenario.doc.crypto_overrides[0].hidden
    );
}
