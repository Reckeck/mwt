//! Scenarios of the envelopes area: each decides in `planning::envelopes`, then
//! ends in one `vault.transact` (`fund_goal` composes two, the other area's
//! first).

use crate::Amount;
use crate::error::{Error, Result};
use crate::kernel::tx::Tx;
use crate::model::ids::StoreId;
use crate::model::planning::Envelope;
use crate::model::vault::now_iso;
use crate::planning::envelopes::{
    EnvelopeDraft, check_draft_against, check_draft_name, new_envelope, plan_fund_envelope,
    plan_fund_goal, plan_move_between_envelopes, plan_update_envelope, settle_envelopes as settle,
};
use crate::service::UnlockedVault;

fn missing_envelope() -> Error {
    Error::Missing("envelope".into())
}

/// A draft is checked against the live store it names (the validate area's
/// `live_store`, called by method: OD6) and against the goals.
fn check_draft(vault: &UnlockedVault, draft: &EnvelopeDraft) -> Result<()> {
    check_draft_name(draft)?;
    let store = vault.live_store(&draft.store_id)?;
    check_draft_against(&vault.doc, store, draft)
}

pub fn create_envelope(vault: &mut UnlockedVault, draft: EnvelopeDraft) -> Result<Envelope> {
    create_envelope_as(vault, draft, uuid::Uuid::new_v4().to_string())
}

fn create_envelope_as(
    vault: &mut UnlockedVault,
    draft: EnvelopeDraft,
    id: String,
) -> Result<Envelope> {
    check_draft(vault, &draft)?;
    let envelope = new_envelope(draft, id, now_iso());
    vault.transact(|tx| {
        tx.collection::<Envelope>().insert(envelope.clone())?;
        Ok(envelope)
    })
}

/// Rename, retarget or relink an envelope. Its store and asset are fixed
/// once money is earmarked.
pub fn update_envelope(
    vault: &mut UnlockedVault,
    id: &str,
    draft: EnvelopeDraft,
) -> Result<Envelope> {
    check_draft(vault, &draft)?;
    let updated = plan_update_envelope(&vault.doc, id, draft)?;
    vault.transact(|tx| {
        tx.collection::<Envelope>()
            .update(id, |slot| {
                *slot = updated.clone();
                updated
            })
            .map_err(|_| missing_envelope())
    })
}

/// Change what an envelope holds by `delta` as an entry of its journal
/// (the only way to touch `allocated`).
fn shift_envelope(tx: &mut Tx<'_>, id: &str, delta: Amount, at: &str) -> Result<Envelope> {
    tx.collection::<Envelope>()
        .update(id, |envelope| {
            envelope.record_move(delta, at);
            envelope.clone()
        })
        .map_err(|_| missing_envelope())
}

/// Earmark more (`amount > 0`, taken from the free balance) or release
/// (`amount < 0`, back to free). Releasing below zero is refused.
pub fn fund_envelope(vault: &mut UnlockedVault, id: &str, amount: Amount) -> Result<Envelope> {
    plan_fund_envelope(&vault.doc, id, amount)?;
    vault.transact(|tx| shift_envelope(tx, id, amount, &now_iso()))
}

/// Move earmarked money between two envelopes of the same store and asset.
pub fn move_between_envelopes(
    vault: &mut UnlockedVault,
    from: &str,
    to: &str,
    amount: Amount,
) -> Result<()> {
    plan_move_between_envelopes(&vault.doc, from, to, amount)?;
    vault.transact(|tx| {
        let at = now_iso();
        shift_envelope(tx, from, -amount, &at)?;
        shift_envelope(tx, to, amount, &at)?;
        Ok(())
    })
}

/// Delete an envelope; whatever it held simply becomes free again.
pub fn delete_envelope(vault: &mut UnlockedVault, id: &str) -> Result<()> {
    vault.transact(|tx| {
        tx.collection::<Envelope>()
            .remove(id)
            .map(|_| ())
            .map_err(|_| missing_envelope())
    })
}

/// Put aside up to `amount` of the goal's currency in `store_id` for the
/// goal: its envelope there is made if missing, and only the free balance
/// is taken. Returns what was earmarked; the goal counts its envelopes.
pub fn fund_goal(
    vault: &mut UnlockedVault,
    goal_id: &str,
    store_id: &StoreId,
    amount: Amount,
) -> Result<Amount> {
    let funding = plan_fund_goal(&vault.doc, goal_id, store_id, amount)?;
    if !funding.take.is_positive() {
        return Ok(Amount::ZERO);
    }
    let id = match funding.envelope_id {
        Some(id) => id,
        None => {
            // Named by goal, wallet and asset: two devices that start
            // funding the same goal make one envelope.
            let id =
                UnlockedVault::goal_envelope_id(goal_id, store_id.as_str(), &funding.asset.key());
            let draft = EnvelopeDraft {
                store_id: store_id.to_string(),
                name: funding.goal_name,
                asset: funding.asset,
                target: None,
                goal_id: Some(goal_id.into()),
            };
            create_envelope_as(vault, draft, id)?.id
        }
    };
    fund_envelope(vault, &id, funding.take)?;
    Ok(funding.take)
}

/// Give every envelope its opening entry and re-derive `allocated` from the
/// journal (on open, and after a sync brought records in); no save of its own.
pub fn settle_envelopes(vault: &mut UnlockedVault) -> bool {
    settle(&mut vault.doc)
}
