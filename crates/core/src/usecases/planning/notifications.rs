//! Scenarios of the notifications area: logging notices (one `transact`) and
//! the sweep that decides what is due now. The rules are in
//! `planning::notifications`; only this file reads the clock.

use crate::error::{Error, Result};
use crate::model::notify::NoticeRecord;
pub use crate::model::notify::notice;
use crate::model::vault::now_iso;
use crate::planning::notifications as rules;
use crate::planning::upcoming::upcoming;
use crate::service::UnlockedVault;

/// Mark notices read: the ones named, or all of them.
pub fn mark_notifications_read(vault: &mut UnlockedVault, keys: Option<&[String]>) -> Result<()> {
    vault.transact(|tx| {
        rules::mark_read(&mut tx.doc_mut().notification_log, keys);
        Ok(())
    })
}

/// Add notices to the log, skipping known keys and switched-off kinds.
/// Returns the ones that are new; writes once.
pub fn log_notices(
    vault: &mut UnlockedVault,
    records: Vec<NoticeRecord>,
) -> Result<Vec<NoticeRecord>> {
    let fresh = rules::fresh_notices(&vault.doc, records);
    if fresh.is_empty() {
        return Ok(fresh);
    }
    vault.transact(|tx| {
        rules::append_notices(&mut tx.doc_mut().notification_log, &fresh);
        Ok(())
    })?;
    Ok(fresh)
}

/// What is due to be told now that was not told before: reminders whose
/// time came, planned money going out the balance will not cover, budgets
/// near or over their limit. They are logged before being returned.
pub fn due_notices(
    vault: &mut UnlockedVault,
    now: i64,
    zone: &str,
    ru: bool,
) -> Result<Vec<NoticeRecord>> {
    let delivered_at = now_iso();
    let mut out =
        rules::reminder_notices(vault.reminders_in(now, zone, ru), now, ru, &delivered_at);
    let lead = rules::shortfall_lead_days(&vault.doc);
    out.extend(rules::shortfall_notices(
        upcoming(&vault.books(), lead, now, zone),
        ru,
        &delivered_at,
    ));
    out.extend(rules::budget_notices(
        vault.budget_status(now, zone),
        now,
        ru,
        &delivered_at,
    ));
    out.extend(vault.health_notice(now, zone, ru)?);
    log_notices(vault, out)
}

/// A notice shown elsewhere (an alert fired while locked, an automation
/// run, a Bybit import): logged so it is not shown again.
pub fn record_notice(vault: &mut UnlockedVault, record: NoticeRecord) -> Result<bool> {
    if record.key.trim().is_empty() {
        return Err(Error::InvalidInput("a notice needs a key".into()));
    }
    Ok(!log_notices(vault, vec![record])?.is_empty())
}
