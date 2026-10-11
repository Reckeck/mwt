//! Enrichment (plan §8.2): what a block explorer says about an operation's
//! hash — the network fee, whether it confirmed — kept aside as a record
//! until the user applies or rejects it. Nothing is merged on its own.

use crate::Amount;
use crate::error::{Error, Result};
use crate::model::chain::Chain;
use crate::model::enrichment::{
    EnrichmentData, EnrichmentRecord, EnrichmentSource, EnrichmentStatus,
};
use crate::model::store::AssetRef;
use crate::model::transaction::{Fee, PriceStamp};
use crate::model::vault::now_iso;
use crate::txinfo::TxInfo;

use super::UnlockedVault;

impl UnlockedVault {
    /// The hash and network to look up for a transaction, if it has a hash.
    pub fn enrichment_request(&self, tx_id: &str) -> Option<(String, Option<Chain>)> {
        let tx = self
            .doc
            .transactions
            .iter()
            .find(|tx| tx.meta().id == tx_id)?;
        tx.on_chain_refs().into_iter().find_map(|(chain, hash)| {
            let hash = hash?.trim();
            (!hash.is_empty()).then(|| (hash.to_string(), chain))
        })
    }

    /// Records for one transaction, or all of them, newest first.
    pub fn enrichments(&self, tx_id: Option<&str>) -> Vec<EnrichmentRecord> {
        let mut out: Vec<EnrichmentRecord> = self
            .doc
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
    fn fee_asset(&self, tx_id: &str, symbol: &str, chain: Option<Chain>) -> AssetRef {
        let symbol = symbol.trim().to_uppercase();
        let known = self
            .doc
            .transactions
            .iter()
            .filter(|tx| tx.meta().id == tx_id)
            .flat_map(|tx| tx.legs())
            .map(|leg| leg.asset)
            .chain(
                self.doc
                    .holdings
                    .iter()
                    .map(|holding| holding.asset.clone()),
            )
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

    /// Keep what the explorer found as a pending record (replacing an older
    /// pending one). `None` when there is nothing worth showing. Does not
    /// persist: the caller writes once.
    pub fn apply_enrichment_result(
        &mut self,
        tx_id: &str,
        info: &TxInfo,
    ) -> Option<EnrichmentRecord> {
        let fee = match (info.fee.as_deref(), info.fee_asset.as_deref()) {
            (Some(amount), Some(symbol)) => Amount::parse(amount)
                .ok()
                .filter(|value| value.is_positive())
                .map(|amount| Fee {
                    asset: self.fee_asset(tx_id, symbol, info.network),
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
        self.doc.enrichments.retain(|record| {
            !(record.tx_id == tx_id && record.status == EnrichmentStatus::Pending)
        });
        let record = EnrichmentRecord {
            id: uuid::Uuid::new_v4().to_string(),
            tx_id: tx_id.to_string(),
            source: EnrichmentSource::Explorer(chain),
            found,
            status: EnrichmentStatus::Pending,
            created_at: now_iso(),
        };
        self.doc.enrichments.push(record.clone());
        Some(record)
    }

    fn pending_record(&self, id: &str) -> Result<EnrichmentRecord> {
        let record = self
            .doc
            .enrichments
            .iter()
            .find(|record| record.id == id)
            .cloned()
            .ok_or_else(|| Error::Missing(format!("enrichment {id}")))?;
        if record.status != EnrichmentStatus::Pending {
            return Err(Error::InvalidInput(
                "this finding was already handled".into(),
            ));
        }
        Ok(record)
    }

    fn set_enrichment_status(&mut self, id: &str, status: EnrichmentStatus) {
        if let Some(record) = self
            .doc
            .enrichments
            .iter_mut()
            .find(|record| record.id == id)
        {
            record.status = status;
        }
    }

    /// Merge a finding into its transaction — the network fee, when the
    /// operation has none yet — through the usual checks.
    pub fn apply_enrichment(&mut self, id: &str) -> Result<EnrichmentRecord> {
        let record = self.pending_record(id)?;
        let mut tx = self
            .doc
            .transactions
            .iter()
            .find(|tx| tx.meta().id == record.tx_id)
            .cloned()
            .ok_or_else(|| Error::Missing("transaction".into()))?;
        if let Some(found) = &record.found.fee {
            let slot = match &mut tx {
                crate::model::transaction::Transaction::Topup(inner) => &mut inner.fee,
                crate::model::transaction::Transaction::Payment(inner) => &mut inner.fee,
                crate::model::transaction::Transaction::Transfer(inner) => &mut inner.fee,
                crate::model::transaction::Transaction::Convert(inner) => &mut inner.fee,
                crate::model::transaction::Transaction::TradeSpot(inner) => &mut inner.fee,
                crate::model::transaction::Transaction::TradeFutures(inner) => &mut inner.fee,
                crate::model::transaction::Transaction::Defi(inner) => &mut inner.fee,
                crate::model::transaction::Transaction::AssetSell(inner) => &mut inner.fee,
                crate::model::transaction::Transaction::Adjustment(inner) => &mut inner.fee,
            };
            if slot.is_none() {
                *slot = Some(found.clone());
            }
        }
        // `update_transaction` validates, re-derives holdings and persists.
        self.set_enrichment_status(id, EnrichmentStatus::Applied);
        if let Err(err) = self.update_transaction(&record.tx_id, tx) {
            self.set_enrichment_status(id, EnrichmentStatus::Pending);
            return Err(err);
        }
        Ok(self.pending_or_any(id))
    }

    /// Dismiss a finding; the transaction stays as it is.
    pub fn reject_enrichment(&mut self, id: &str) -> Result<EnrichmentRecord> {
        self.pending_record(id)?;
        self.set_enrichment_status(id, EnrichmentStatus::Rejected);
        self.persist()?;
        Ok(self.pending_or_any(id))
    }

    fn pending_or_any(&self, id: &str) -> EnrichmentRecord {
        self.doc
            .enrichments
            .iter()
            .find(|record| record.id == id)
            .cloned()
            .expect("record exists")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::store::StoreKind;
    use crate::model::transaction::{AccountRef, Transaction};
    use crate::service::test_support::*;

    fn eth() -> AssetRef {
        AssetRef::crypto("ETH", "Ethereum", Some("ethereum"), Chain::Ethereum)
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

    fn found(fee: &str) -> TxInfo {
        TxInfo {
            network: Some(Chain::Ethereum),
            status: Some("success".into()),
            fee: Some(fee.into()),
            fee_asset: Some("ETH".into()),
            ..TxInfo::default()
        }
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
}
