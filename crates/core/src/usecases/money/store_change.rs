//! The scenarios of the store change area: what a change of kind, place or
//! merge would do (nothing written), and the change itself. The change is
//! worked out in `money::store_change` on a copy of the document; applying it
//! writes the `.bak`, replaces the document and ends in one commit.

use std::collections::HashMap;

use crate::error::{Error, Result};
use crate::model::vault::VaultDoc;
use crate::money::store_change::{
    ChangeOutcome, CreatedAt, StoreChangeInput, StoreChangePlan, refusal, work_out,
};
use crate::service::UnlockedVault;
use crate::storage::VaultFile;
use crate::sync::view::full_key;
use crate::usecases::money::stores::rebase_and_commit;

/// Creation order of every transaction, from its sync stamp (money must not
/// depend on sync, so it is read here).
fn created_at(doc: &VaultDoc) -> HashMap<String, CreatedAt> {
    doc.transactions
        .iter()
        .filter_map(|tx| {
            let id = &tx.meta().id;
            let stamp = &doc.sync.stamps.get(&full_key("transactions", id))?.created;
            Some((
                id.clone(),
                CreatedAt((stamp.wall, stamp.count, stamp.device.clone())),
            ))
        })
        .collect()
}

/// What a change would do; nothing is written.
pub fn plan_store_change(vault: &UnlockedVault, input: &StoreChangeInput) -> StoreChangePlan {
    work_out(&vault.doc, input, &created_at(&vault.doc)).plan
}

/// Make the change: refused while it has an error or a conflict. The vault
/// file is copied to `.bak` first.
pub fn apply_store_change(
    vault: &mut UnlockedVault,
    input: &StoreChangeInput,
) -> Result<StoreChangePlan> {
    let ChangeOutcome { plan, after } = work_out(&vault.doc, input, &created_at(&vault.doc));
    if let Some(message) = refusal(&plan) {
        return Err(Error::InvalidInput(message));
    }
    // The backup is of the file: batched changes go to it first.
    vault.flush_pending()?;
    VaultFile::backup(&vault.path)?;
    let before = std::mem::replace(&mut vault.doc, after);
    if let Err(err) = rebase_and_commit(vault) {
        vault.doc = before;
        crate::usecases::money::portfolio::recompute_holdings(vault);
        return Err(err);
    }
    Ok(plan)
}
