//! The scenarios of the enrichment area: a finding is kept, applied to its
//! transaction or dismissed. What a finding holds and what applying it would
//! change are decided in `money::enrichment`.

use crate::error::Result;
use crate::model::enrichment::{EnrichmentRecord, EnrichmentStatus};
use crate::model::vault::now_iso;
use crate::money::enrichment::{
    pending_record, plan_apply_enrichment, plan_enrichment_result, record_of,
};
use crate::money::txinfo::TxInfo;
use crate::service::Session;

use super::transactions::update_transaction;

/// Keep what the explorer found as a pending record (replacing an older
/// pending one). `None` when there is nothing worth showing. Does not
/// persist: the caller writes once.
pub fn apply_enrichment_result(
    vault: &mut Session,
    tx_id: &str,
    info: &TxInfo,
) -> Option<EnrichmentRecord> {
    let record = plan_enrichment_result(
        &vault.doc,
        tx_id,
        info,
        uuid::Uuid::new_v4().to_string(),
        now_iso(),
    )?;
    vault
        .doc
        .enrichments
        .retain(|known| !(known.tx_id == tx_id && known.status == EnrichmentStatus::Pending));
    vault.doc.enrichments.push(record.clone());
    Some(record)
}

/// Merge a finding into its transaction — the network fee, when the
/// operation has none yet — through the usual checks.
pub fn apply_enrichment(vault: &mut Session, id: &str) -> Result<EnrichmentRecord> {
    let (record, tx) = plan_apply_enrichment(&vault.doc, id)?;
    // `update_transaction` validates, re-derives holdings and persists, so the
    // status goes in first and comes back if it refuses.
    set_status(vault, id, EnrichmentStatus::Applied);
    if let Err(err) = update_transaction(vault, &record.tx_id, tx) {
        set_status(vault, id, EnrichmentStatus::Pending);
        return Err(err);
    }
    Ok(stored(vault, id))
}

/// Dismiss a finding; the transaction stays as it is.
pub fn reject_enrichment(vault: &mut Session, id: &str) -> Result<EnrichmentRecord> {
    pending_record(&vault.doc, id)?;
    vault.transact(|tx| {
        tx.collection::<EnrichmentRecord>()
            .update(id, |record| record.status = EnrichmentStatus::Rejected)
    })?;
    Ok(stored(vault, id))
}

fn set_status(vault: &mut Session, id: &str, status: EnrichmentStatus) {
    if let Some(record) = vault
        .doc
        .enrichments
        .iter_mut()
        .find(|record| record.id == id)
    {
        record.status = status;
    }
}

fn stored(vault: &Session, id: &str) -> EnrichmentRecord {
    record_of(&vault.doc, id).cloned().expect("record exists")
}
