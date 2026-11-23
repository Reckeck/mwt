//! Scenarios of the automatic reminders: the list in the reader's language
//! and the sync that keeps the reminders in step with the data (one
//! `transact`). The rules are in `planning::auto_reminders`.

use crate::Amount;
use crate::calendar::Calendar;
use crate::model::contact::DebtTerms;
use crate::model::planning::Reminder;
use crate::planning::auto_reminders as rules;
use crate::service::Session;

/// What is still open on a debt's contact and asset: what they owe minus
/// what is owed to them. The balances are read once, then looked up per debt.
pub(crate) fn open_debt(vault: &Session) -> impl Fn(&DebtTerms) -> Amount {
    let balances = crate::usecases::household::contacts::contact_balances(vault);
    move |terms| {
        balances
            .iter()
            .filter(|row| {
                row.contact_id == terms.contact_id
                    && row.asset.symbol.eq_ignore_ascii_case(&terms.asset)
            })
            .fold(Amount::ZERO, |acc, row| acc + row.receivable - row.payable)
    }
}

/// The reminders as a reader in `ru` or English sees them. The stored
/// text of an automatic reminder is in the language of whichever device
/// wrote it; when it is one of the two renderings of its source it is
/// shown in the reader's language, otherwise (the user typed it, or the
/// data moved on) as it is. Nothing is written.
pub fn reminders_in(vault: &Session, now: i64, zone: &str, ru: bool) -> Vec<Reminder> {
    let today = Calendar::new(zone).date_of(now);
    let renderings = rules::auto_renderings(&vault.doc, open_debt(vault), today, ru);
    rules::localized(crate::planning::agenda::reminders(&vault.doc), &renderings)
}

/// Create, move and remove the automatic reminders so they match the data.
/// Returns whether anything changed (and was written).
pub fn sync_auto_reminders(
    vault: &mut Session,
    now: i64,
    zone: &str,
    ru: bool,
) -> crate::Result<bool> {
    let Some(reminders) = rules::synced_reminders(&vault.doc, open_debt(vault), now, zone, ru)
    else {
        return Ok(false);
    };
    vault.transact(|tx| {
        tx.doc_mut().reminders = reminders;
        Ok(())
    })?;
    Ok(true)
}
