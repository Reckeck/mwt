//! Enrichment (plan §8.2): what a block explorer says about an operation's
//! hash — the network fee, whether it confirmed — kept aside as a record
//! until the user applies or rejects it. Nothing is merged on its own.
//! The rules live here; the scenarios that store a finding, apply or reject
//! it are in `usecases::money::enrichment`.

use crate::Amount;
use crate::error::{Error, Result};
use crate::model::chain::Chain;
use crate::model::enrichment::{
    EnrichmentData, EnrichmentRecord, EnrichmentSource, EnrichmentStatus,
};
use crate::model::store::AssetRef;
use crate::model::transaction::{Fee, PriceStamp, Transaction};
use crate::model::vault::VaultDoc;
use crate::money::txinfo::TxInfo;

/// The hash and network to look up for a transaction, if it has a hash.
pub fn enrichment_request(doc: &VaultDoc, tx_id: &str) -> Option<(String, Option<Chain>)> {
    let tx = doc.transactions.iter().find(|tx| tx.meta().id == tx_id)?;
    tx.on_chain_refs().into_iter().find_map(|(chain, hash)| {
        let hash = hash?.trim();
        (!hash.is_empty()).then(|| (hash.to_string(), chain))
    })
}

/// Records for one transaction, or all of them, newest first.
pub fn enrichments(doc: &VaultDoc, tx_id: Option<&str>) -> Vec<EnrichmentRecord> {
    let mut out: Vec<EnrichmentRecord> = doc
        .enrichments
        .iter()
        .filter(|record| tx_id.is_none_or(|id| record.tx_id == id))
        .cloned()
        .collect();
    out.sort_by(|left, right| right.created_at.cmp(&left.created_at));
    out
}

/// The asset a fee symbol names: one the transaction or the vault already
/// knows, else the network's own coin.
fn fee_asset(doc: &VaultDoc, tx_id: &str, symbol: &str, chain: Option<Chain>) -> AssetRef {
    let symbol = symbol.trim().to_uppercase();
    let known = doc
        .transactions
        .iter()
        .filter(|tx| tx.meta().id == tx_id)
        .flat_map(|tx| tx.legs())
        .map(|leg| leg.asset)
        .chain(doc.holdings.iter().map(|holding| holding.asset.clone()))
        .find(|asset| asset.symbol == symbol && !asset.is_derivative());
    known.unwrap_or_else(|| match chain {
        Some(chain) => AssetRef::crypto(&symbol, &symbol, chain.info().cg_coin_id, chain),
        None => AssetRef {
            quote: symbol.clone(),
            name: symbol.clone(),
            symbol,
            coingecko_id: None,
            chain: None,
            contract: None,
        },
    })
}

/// What the explorer found as a pending record, stamped with the `id` and the
/// moment the caller gives. `None` when there is nothing worth showing.
pub fn plan_enrichment_result(
    doc: &VaultDoc,
    tx_id: &str,
    info: &TxInfo,
    id: String,
    created_at: String,
) -> Option<EnrichmentRecord> {
    let fee = match (info.fee.as_deref(), info.fee_asset.as_deref()) {
        (Some(amount), Some(symbol)) => Amount::parse(amount)
            .ok()
            .filter(|value| value.is_positive())
            .map(|amount| Fee {
                asset: fee_asset(doc, tx_id, symbol, info.network),
                amount,
                price: PriceStamp::default(),
            }),
        _ => None,
    };
    let confirmed = info.status.as_deref().map(|status| {
        matches!(
            status.to_ascii_lowercase().as_str(),
            "confirmed" | "success" | "finalized" | "ok"
        )
    });
    let found = EnrichmentData {
        fee,
        confirmed,
        block_time: info.occurred_at.clone(),
        ..EnrichmentData::default()
    };
    if found.fee.is_none() && found.confirmed.is_none() && found.block_time.is_none() {
        return None;
    }
    let chain = info.network?;
    Some(EnrichmentRecord {
        id,
        tx_id: tx_id.to_string(),
        source: EnrichmentSource::Explorer(chain),
        found,
        status: EnrichmentStatus::Pending,
        created_at,
    })
}

/// The record `id`, if it still waits for an answer.
pub fn pending_record(doc: &VaultDoc, id: &str) -> Result<EnrichmentRecord> {
    let record = record_of(doc, id)
        .cloned()
        .ok_or_else(|| Error::Missing(format!("enrichment {id}")))?;
    if record.status != EnrichmentStatus::Pending {
        return Err(Error::InvalidInput(
            "this finding was already handled".into(),
        ));
    }
    Ok(record)
}

/// The record `id` in whatever state it is.
pub fn record_of<'doc>(doc: &'doc VaultDoc, id: &str) -> Option<&'doc EnrichmentRecord> {
    doc.enrichments.iter().find(|record| record.id == id)
}

/// The pending finding `id` and the transaction it would amend: the network
/// fee goes in when the operation has none yet.
pub fn plan_apply_enrichment(doc: &VaultDoc, id: &str) -> Result<(EnrichmentRecord, Transaction)> {
    let record = pending_record(doc, id)?;
    let mut tx = doc
        .transactions
        .iter()
        .find(|tx| tx.meta().id == record.tx_id)
        .cloned()
        .ok_or_else(|| Error::Missing("transaction".into()))?;
    if let Some(found) = &record.found.fee {
        let slot = match &mut tx {
            Transaction::Topup(inner) => &mut inner.fee,
            Transaction::Payment(inner) => &mut inner.fee,
            Transaction::Transfer(inner) => &mut inner.fee,
            Transaction::Convert(inner) => &mut inner.fee,
            Transaction::TradeSpot(inner) => &mut inner.fee,
            Transaction::TradeFutures(inner) => &mut inner.fee,
            Transaction::Defi(inner) => &mut inner.fee,
            Transaction::AssetSell(inner) => &mut inner.fee,
            Transaction::Adjustment(inner) => &mut inner.fee,
        };
        if slot.is_none() {
            *slot = Some(found.clone());
        }
    }
    Ok((record, tx))
}
