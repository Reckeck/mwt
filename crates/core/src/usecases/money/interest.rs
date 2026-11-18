//! Scenarios of the interest area: terms of an account, the key-rate merge and
//! the monthly posting of interest. Each decides in `money::interest`, then
//! writes (the key-rate merge, which the refresh persists once at its end,
//! writes nothing).

use chrono::NaiveDate;

use crate::error::{Error, Result};
use crate::model::deposit::{DepositTerms, KeyRatePoint};
use crate::model::ids::StoreId;
use crate::model::store::Store;
use crate::money::interest::{merge_key_rates, plan_deposit_terms, plan_due_interest};
use crate::service::UnlockedVault;

/// Set or clear the terms of a savings account, deposit or ПДС.
pub fn set_deposit_terms(
    vault: &mut UnlockedVault,
    store_id: &StoreId,
    terms: Option<DepositTerms>,
) -> Result<Store> {
    let terms = plan_deposit_terms(&vault.doc, store_id, terms)?;
    vault.transact(|tx| {
        let store = tx
            .doc_mut()
            .stores
            .iter_mut()
            .find(|store| store.id == store_id.as_str())
            .ok_or_else(|| Error::Missing(format!("store {store_id}")))?;
        store.deposit = terms;
        Ok(store.clone())
    })
}

/// Merge fetched key-rate points into the cache. Does not persist.
pub fn apply_key_rates(vault: &mut UnlockedVault, fresh: &[KeyRatePoint]) {
    vault.doc.key_rates = merge_key_rates(&vault.doc.key_rates, fresh);
}

/// Book last month's interest on every account that asks for it, once.
/// Only the month just ended is posted: older months may already be in
/// the ledger by hand, and guessing them would count them twice.
pub fn post_due_interest(vault: &mut UnlockedVault, today: NaiveDate) -> Result<u32> {
    let drafts = plan_due_interest(&vault.books(), today);
    let count = drafts.len() as u32;
    for draft in drafts {
        vault.add_transaction(draft)?;
    }
    Ok(count)
}
