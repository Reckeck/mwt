//! How much the crypto module's events cost, measured instead of guessed
//! (decision P4 of 2026-11-10).
//!
//! `sync::track` serializes the whole document and hashes every field of
//! every record on each save and on each piece of a sync, so the price of
//! keeping raw events in the vault has to be known before the limit on their
//! number is decided (spec E5).
//!
//! The measurement is not part of the gate; run it on purpose:
//!
//! ```text
//! cargo test -p trade-core crypto_events_volume -- --ignored --nocapture
//! ```

use std::time::Instant;

use super::super::test_support::empty_vault;
use crate::Amount;
use crate::model::chain::Chain;
use crate::model::cryptotrack::{
    CryptoAsset, CryptoDirection, CryptoFee, CryptoLeg, CryptoTx, CryptoTxKind, TokenId,
    TrackingRef,
};
use crate::storage::{SnapshotFormat, encode};

/// An event of the size a swap really has: two legs, gas, a method and the
/// provider's answer kept for a re-read.
fn event(index: usize) -> CryptoTx {
    let token = TokenId::new(
        Chain::Ethereum,
        "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
    );
    let hash = format!("0x{index:064x}");
    CryptoTx {
        id: format!("cev-{index}"),
        chain: Chain::Ethereum,
        account_id: "cacc-1".into(),
        time: "2026-11-10T10:00:00Z".into(),
        block: Some(21_000_000 + index as u64),
        from: "0xabcd00000000000000000000000000000000ef01".into(),
        to: "0xbeef00000000000000000000000000000000ef01".into(),
        legs: vec![
            CryptoLeg {
                direction: CryptoDirection::Out,
                asset: CryptoAsset::Native {
                    chain: Chain::Ethereum,
                },
                amount: Amount::parse("0.5").unwrap(),
                counterparty: Some("0xbeef00000000000000000000000000000000ef01".into()),
            },
            CryptoLeg {
                direction: CryptoDirection::In,
                asset: CryptoAsset::Token(token),
                amount: Amount::parse("1800.25").unwrap(),
                counterparty: None,
            },
        ],
        gas: Some(CryptoFee {
            asset: CryptoAsset::Native {
                chain: Chain::Ethereum,
            },
            amount: Amount::parse("0.0021").unwrap(),
        }),
        protocol: Some("uniswap-v3".into()),
        method: Some("exactInputSingle".into()),
        kind: CryptoTxKind::Swap,
        raw: Some(format!("{{\"hash\":\"{hash}\",\"status\":\"1\"}}")),
        tracking: TrackingRef::ChainTx {
            chain: Chain::Ethereum,
            hash,
        },
    }
}

#[test]
#[ignore = "a measurement, not a check: run it with --ignored --nocapture"]
fn crypto_events_volume() {
    println!(
        "{:>7} {:>12} {:>12} {:>12} {:>12} {:>12}",
        "events", "capture", "stamp", "persist", "json", "pack"
    );
    for count in [1_000usize, 10_000, 50_000] {
        let (_dir, mut vault) = empty_vault();
        vault.doc.crypto_events = (0..count).map(event).collect();

        // The first pass only remembers the document (what `open` does), the
        // second is the diff every save pays.
        let started = Instant::now();
        vault.track_changes().unwrap();
        let capture = started.elapsed();
        let started = Instant::now();
        vault.track_changes().unwrap();
        let stamp = started.elapsed();

        let started = Instant::now();
        vault.persist_now().unwrap();
        let persist = started.elapsed();

        let file = std::fs::metadata(&vault.path).unwrap().len();
        let json = encode(&vault.doc, SnapshotFormat::Json).unwrap().len();
        let pack = encode(&vault.doc, SnapshotFormat::Pack).unwrap().len();

        println!(
            "{count:>7} {:>9.0?} {:>9.0?} {:>9.0?} {:>9} KiB {:>9} KiB  (file {} KiB)",
            capture,
            stamp,
            persist,
            json / 1024,
            pack / 1024,
            file / 1024
        );
        // A sanity floor, so the measurement cannot silently measure nothing.
        assert_eq!(vault.doc.crypto_events.len(), count);
        assert!(pack < json, "MessagePack must be the smaller of the two");
    }
}
