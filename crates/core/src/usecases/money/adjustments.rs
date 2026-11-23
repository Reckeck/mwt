//! The adjustments area over the open vault: the correction screen takes the
//! request of the form, and the corrections are worked out again over the
//! document the vault holds. Nothing here writes: the caller that changed the
//! history persists, as it did when these were vault methods.

use crate::error::Result;
use crate::ipc_contract::AdjustmentDraftInput;
use crate::model::transaction::{AdjustmentLine, PriceStamp};
use crate::model::vault::{VaultDoc, now_iso};
use crate::money::adjustments::{AdjustmentDraft, balances_at, rebase_adjustment_lines};
use crate::money::validate::live_store;
use crate::service::Session;

/// Work out again what the ledger held before every correction.
pub(crate) fn rebase_adjustments(vault: &mut Session) {
    rebase_adjustment_lines(&mut vault.doc);
}

/// The correction screen of one account: its assets and their ledger
/// balances at the moment asked (now by default).
pub fn adjustment_draft(doc: &VaultDoc, input: &AdjustmentDraftInput) -> Result<AdjustmentDraft> {
    live_store(doc, &input.store_id)?;
    let at = input
        .at
        .clone()
        .filter(|raw| !raw.trim().is_empty())
        .unwrap_or_else(now_iso);
    let lines = balances_at(doc, &input.store_id, &at, input.replaces.as_deref())
        .into_iter()
        .map(|(asset, balance)| AdjustmentLine {
            asset,
            before: balance,
            after: balance,
            price: PriceStamp::default(),
        })
        .collect();
    Ok(AdjustmentDraft {
        store_id: input.store_id.clone(),
        at,
        lines,
    })
}
