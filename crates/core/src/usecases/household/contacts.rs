//! Scenarios of the contacts area: each decides in `household::contacts`, then
//! ends in one `vault.transact`; the reads gather what the not-yet-moved shared
//! events add and hand it to the pure functions.

use crate::error::Result;
use crate::household::contacts::{
    ContactBalance, ContactDraft, ContactSummary, ContactTurnover, DebtTermsDraft,
    contact_balances_until, debt_terms_of, debt_terms_replaced, event_debts, plan_create_contact,
    plan_debt_terms, plan_delete_contact, plan_update_contact, require_contact, summaries,
    turnover,
};
use crate::kernel::tx::Tx;
use crate::model::contact::{Contact, DebtTerms};
use crate::model::vault::now_iso;
use crate::service::UnlockedVault;
use crate::sync::sync_ids::debt_terms_id;

pub fn create_contact(vault: &mut UnlockedVault, draft: ContactDraft) -> Result<Contact> {
    let contact = plan_create_contact(draft, &now_iso())?;
    vault.transact(|tx| {
        tx.collection::<Contact>().insert(contact.clone())?;
        Ok(contact)
    })
}

/// Replace a contact's fields and accounts. An account that disappeared
/// from the draft but is used by a transaction is kept, deactivated.
pub fn update_contact(vault: &mut UnlockedVault, id: &str, draft: ContactDraft) -> Result<Contact> {
    let edit = plan_update_contact(&vault.doc, id, draft)?;
    vault.transact(|tx| {
        tx.collection::<Contact>().update(id, |contact| {
            contact.name = edit.name;
            contact.kind = edit.kind;
            contact.note = edit.note;
            contact.accounts = edit.accounts;
            contact.group = edit.group;
            contact.clone()
        })
    })
}

pub fn archive_contact(vault: &mut UnlockedVault, id: &str, archived: bool) -> Result<()> {
    require_contact(&vault.doc, id)?;
    vault.transact(|tx| {
        tx.collection::<Contact>()
            .update(id, |contact| contact.archived = archived)
    })
}

/// Only a contact nothing refers to can be deleted; otherwise archive it. Its
/// debt terms go with it, in the same transaction.
pub fn delete_contact(vault: &mut UnlockedVault, id: &str) -> Result<()> {
    plan_delete_contact(&vault.doc, id)?;
    let terms = debt_terms_of(&vault.doc, id);
    vault.transact(|tx| {
        for terms_id in &terms {
            tx.collection::<DebtTerms>().remove(terms_id)?;
        }
        tx.collection::<Contact>().remove(id).map(|_| ())
    })
}

/// What was sent to and received from a contact, per asset. Internal
/// moves never count: they did not leave the user's hands.
pub fn contact_turnover(
    vault: &UnlockedVault,
    contact_id: &str,
    account_id: Option<&str>,
) -> Result<ContactTurnover> {
    turnover(&vault.books(), contact_id, account_id)
}

/// One summary per contact: operation count, the latest operation and the
/// debts converted to the base currency with the cached rates (no network).
pub fn contact_summaries(vault: &UnlockedVault) -> Result<Vec<ContactSummary>> {
    Ok(summaries(
        &vault.books(),
        &event_debts(&vault.doc, i64::MAX),
    ))
}

/// Personal debts per contact and asset, from transfers marked as debts only.
pub fn contact_balances(vault: &UnlockedVault) -> Vec<ContactBalance> {
    contact_balances_until(&vault.books(), i64::MAX)
}

/// Set (or replace) the terms of one contact's debt in one asset; an
/// empty date and note removes them.
pub fn set_debt_terms(
    vault: &mut UnlockedVault,
    draft: DebtTermsDraft,
) -> Result<Option<DebtTerms>> {
    let plan = plan_debt_terms(&vault.doc, draft)?;
    let replaced = debt_terms_replaced(&vault.doc, &plan.contact_id, &plan.asset);
    let terms = (!plan.clears()).then(|| DebtTerms {
        id: debt_terms_id(&plan.contact_id, &plan.asset),
        contact_id: plan.contact_id,
        asset: plan.asset,
        due_at: plan.due_at,
        note: plan.note,
        created_at: now_iso(),
    });
    vault.transact(|tx| write_terms(tx, &replaced, terms))
}

fn write_terms(
    tx: &mut Tx<'_>,
    replaced: &[String],
    terms: Option<DebtTerms>,
) -> Result<Option<DebtTerms>> {
    for terms_id in replaced {
        tx.collection::<DebtTerms>().remove(terms_id)?;
    }
    if let Some(terms) = &terms {
        tx.collection::<DebtTerms>().insert(terms.clone())?;
    }
    Ok(terms)
}
