//! Scenarios of the emergency fund: each decides in `planning::emergency`,
//! then ends in one `vault.transact`. The status read is in
//! `planning::emergency`.

use crate::error::Result;
use crate::model::planning::EmergencyFund;
use crate::planning::emergency::{
    plan_set_emergency_fund, prune_emergency_fund as prune, toggled_fund,
};
use crate::service::Session;

/// Replace the fund. Only live savings accounts fit (decision 29); an
/// empty list removes the fund.
pub fn set_emergency_fund(vault: &mut Session, fund: EmergencyFund) -> Result<EmergencyFund> {
    let next = plan_set_emergency_fund(&vault.doc, fund)?;
    vault.transact(|tx| {
        tx.doc_mut().emergency_fund = next.clone();
        Ok(next)
    })
}

/// Drop accounts that stopped being live savings (decision 36). The caller
/// saves: this runs inside the writes that archive or re-purpose a store.
pub(crate) fn prune_emergency_fund(vault: &mut Session) {
    prune(&mut vault.doc);
}

/// Add or remove one account, the switch in the store form.
pub fn emergency_fund_toggle(
    vault: &mut Session,
    store_id: &str,
    on: bool,
) -> Result<EmergencyFund> {
    let fund = toggled_fund(&vault.doc, store_id, on);
    set_emergency_fund(vault, fund)
}
