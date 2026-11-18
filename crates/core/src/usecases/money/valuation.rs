//! Scenarios of the valuation area: a dated valuation added or removed, a
//! revalue rule set. Each decides in `money::valuation`, then ends in one
//! `vault.transact`.

use crate::error::{Error, Result};
use crate::model::ids::StoreId;
use crate::model::store::RevalueRule;
use crate::model::vault::Valuation;
use crate::money::valuation::{ValuationInput, plan_revalue_rule, plan_valuation};
use crate::service::UnlockedVault;

/// Add a dated valuation; one on the same day replaces it.
pub fn add_valuation(vault: &mut UnlockedVault, input: ValuationInput) -> Result<Valuation> {
    let valuation = UnlockedVault::valuation_record(plan_valuation(&vault.doc, input)?);
    vault.transact(|tx| {
        let valuations = &mut tx.doc_mut().valuations;
        valuations
            .retain(|row| !(row.asset_key == valuation.asset_key && row.date == valuation.date));
        valuations.push(valuation.clone());
        Ok(valuation)
    })
}

/// Remove a valuation; returns it so the UI can put it back.
pub fn delete_valuation(vault: &mut UnlockedVault, id: &str) -> Result<Valuation> {
    vault.transact(|tx| {
        let index = tx
            .doc()
            .valuations
            .iter()
            .position(|row| row.id == id)
            .ok_or_else(|| Error::NotFound(format!("valuation {id}")))?;
        Ok(tx.doc_mut().valuations.remove(index))
    })
}

/// Set or switch off a property's rule. Switching it on where nothing was
/// valued yet first values every asset at today's price, so the value
/// continues from where it is instead of jumping (plan §11.4).
pub fn set_revalue_rule(
    vault: &mut UnlockedVault,
    store_id: &StoreId,
    rule: Option<RevalueRule>,
    zone: &str,
) -> Result<()> {
    let starts = plan_revalue_rule(&vault.books(), store_id, rule.as_ref(), zone)?;
    vault.transact(|tx| {
        let doc = tx.doc_mut();
        doc.valuations
            .extend(starts.into_iter().map(UnlockedVault::valuation_record));
        let found = doc
            .stores
            .iter_mut()
            .find(|row| row.id == store_id.as_str())
            .ok_or_else(|| Error::NotFound(format!("property {store_id}")))?;
        let info = found.property.get_or_insert_with(Default::default);
        info.revalue = rule;
        Ok(())
    })
}
