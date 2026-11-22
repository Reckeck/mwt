//! Tests of the enrichment area that need the vault (`UnlockedVault`): moved from
//! `money/enrichment_tests.rs` with their assertions unchanged (crate-split T13).

use crate::model::chain::Chain;
use crate::model::enrichment::EnrichmentStatus;
use crate::model::store::AssetRef;
use crate::model::store::StoreKind;
use crate::model::transaction::{AccountRef, Transaction};
use crate::money::enrichment::*;
use crate::service::UnlockedVault;
use crate::service::test_support::*;
use crate::txinfo::TxInfo;

fn eth() -> AssetRef {
    AssetRef::crypto("ETH", "Ethereum", Some("ethereum"), Chain::Ethereum)
}

fn found(fee: &str) -> TxInfo {
    TxInfo {
        network: Some(Chain::Ethereum),
        status: Some("success".into()),
        fee: Some(fee.into()),
        fee_asset: Some("ETH".into()),
        ..TxInfo::default()
    }
}

/// A wallet funded with ETH and a transfer out with a hash, no fee.
fn setup_transfer() -> (tempfile::TempDir, UnlockedVault, String) {
    let (dir, mut vault) = empty_vault();
    let wallet = store(&mut vault, "Hot", StoreKind::Crypto, None);
    vault
        .add_transaction(income_of(&wallet, eth(), "2", "3000", "in"))
        .unwrap();
    let mut out = transfer_between(
        AccountRef::Store(wallet.clone()),
        AccountRef::Raw("0xabc".into()),
        eth(),
        "1",
    );
    if let Transaction::Transfer(inner) = &mut out {
        inner.tx_hash = Some(format!("0x{}", "a".repeat(64)));
        inner.network = Some(Chain::Ethereum);
    }
    let id = vault.add_transaction(out).unwrap().meta().id.clone();
    (dir, vault, id)
}

#[test]
fn applying_merges_the_fee_and_passes_validation() {
    let (_dir, mut vault, id) = setup_transfer();
    assert_eq!(
        vault.enrichment_request(&id).unwrap().1,
        Some(Chain::Ethereum)
    );
    let record = vault.apply_enrichment_result(&id, &found("0.002")).unwrap();
    assert_eq!(record.found.confirmed, Some(true));
    let applied = vault.apply_enrichment(&record.id).unwrap();
    assert_eq!(applied.status, EnrichmentStatus::Applied);
    let tx = vault
        .doc
        .transactions
        .iter()
        .find(|tx| tx.meta().id == id)
        .unwrap();
    assert_eq!(tx.fee().unwrap().amount, amount("0.002"));
    assert_eq!(tx.fee().unwrap().asset.key(), eth().key());
    // The fee came off the wallet.
    let wallet = tx.legs()[0].store_id.clone();
    assert_eq!(holding_of(&vault, &wallet, &eth()), amount("0.998"));
    assert!(vault.apply_enrichment(&record.id).is_err());
}

#[test]
fn rejecting_leaves_the_transaction_alone() {
    let (_dir, mut vault, id) = setup_transfer();
    let before = vault.doc.transactions.clone();
    let record = vault.apply_enrichment_result(&id, &found("0.002")).unwrap();
    let rejected = vault.reject_enrichment(&record.id).unwrap();
    assert_eq!(rejected.status, EnrichmentStatus::Rejected);
    assert_eq!(vault.doc.transactions, before);
}

#[test]
fn a_new_finding_replaces_the_pending_one_and_nothing_found_records_nothing() {
    let (_dir, mut vault, id) = setup_transfer();
    vault.apply_enrichment_result(&id, &found("0.002")).unwrap();
    vault.apply_enrichment_result(&id, &found("0.003")).unwrap();
    assert_eq!(vault.enrichments(Some(&id)).len(), 1);
    assert!(
        vault
            .apply_enrichment_result(
                &id,
                &TxInfo {
                    network: Some(Chain::Ethereum),
                    ..TxInfo::default()
                }
            )
            .is_none()
    );
}

// -- the pure parts, on a bare document ---------------------------------------

#[test]
fn enrichments_read_a_bare_document_newest_first() {
    use crate::model::enrichment::{EnrichmentData, EnrichmentRecord, EnrichmentSource};
    let record = |id: &str, tx_id: &str, created_at: &str| EnrichmentRecord {
        id: id.into(),
        tx_id: tx_id.into(),
        source: EnrichmentSource::Explorer(Chain::Ethereum),
        found: EnrichmentData::default(),
        status: EnrichmentStatus::Pending,
        created_at: created_at.into(),
    };
    let doc = crate::model::vault::VaultDoc {
        enrichments: vec![
            record("old", "t1", "2026-01-01T00:00:00Z"),
            record("new", "t1", "2026-02-01T00:00:00Z"),
            record("other", "t2", "2026-03-01T00:00:00Z"),
        ],
        ..crate::model::vault::VaultDoc::default()
    };
    let ids = |found: Vec<EnrichmentRecord>| -> Vec<String> {
        found.into_iter().map(|record| record.id).collect()
    };
    assert_eq!(ids(enrichments(&doc, Some("t1"))), ["new", "old"]);
    assert_eq!(ids(enrichments(&doc, None)), ["other", "new", "old"]);
    assert!(pending_record(&doc, "new").is_ok());
    assert!(pending_record(&doc, "nope").is_err());
}

#[test]
fn a_finding_is_planned_without_touching_the_document() {
    let doc = crate::model::vault::VaultDoc::default();
    let record = plan_enrichment_result(
        &doc,
        "t1",
        &found("0.002"),
        "r1".into(),
        "2026-01-01T00:00:00Z".into(),
    )
    .unwrap();
    assert_eq!(record.id, "r1");
    assert_eq!(record.status, EnrichmentStatus::Pending);
    assert_eq!(record.found.fee.unwrap().asset.symbol, "ETH");
    assert!(doc.enrichments.is_empty());
    let nothing = TxInfo {
        network: Some(Chain::Ethereum),
        ..TxInfo::default()
    };
    assert!(plan_enrichment_result(&doc, "t1", &nothing, "r2".into(), "x".into()).is_none());
}

// -- commits ------------------------------------------------------------------

/// Keeping a finding leaves the write to the caller; rejecting one is a
/// commit of its own.
#[test]
fn keeping_a_finding_writes_nothing_and_rejecting_one_writes_once() {
    let (_dir, mut vault, id) = setup_transfer();
    let commits = super::bulk::Commits::watch(&mut vault);
    let record = vault.apply_enrichment_result(&id, &found("0.002")).unwrap();
    assert_eq!(commits.count(), 0);
    vault.reject_enrichment(&record.id).unwrap();
    assert_eq!(commits.count(), 1);
    assert!(
        vault.reject_enrichment(&record.id).is_err(),
        "already handled"
    );
    assert_eq!(commits.count(), 1);
}
