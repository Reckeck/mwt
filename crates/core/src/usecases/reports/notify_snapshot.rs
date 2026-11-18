//! The one writer of the notification snapshot: take in what the locked app
//! showed. It ends in one `vault.transact`: the fired alerts and the notice
//! log change together, so a crash never leaves one without the other.

use std::collections::{HashMap, HashSet};

use crate::model::notify::{MAX_NOTICE_LOG, NoticeRecord};
use crate::reports::notify_snapshot::AlertBase;
use crate::service::UnlockedVault;

/// [`apply_locked_deliveries_seen`] with no record of what the alerts looked
/// like: every firing applies.
pub fn apply_locked_deliveries(
    vault: &mut UnlockedVault,
    records: Vec<NoticeRecord>,
    fired_alerts: &[(String, String)],
) -> crate::Result<()> {
    apply_locked_deliveries_seen(vault, records, fired_alerts, &HashMap::new())
}

/// Take in what was shown while the vault was locked: logged so it never
/// repeats; alerts that fired are marked triggered. `seen` holds each
/// fired alert's threshold as the locked app saw it: an alert that was
/// changed since (a newer threshold came by sync) or switched off is left
/// alone, so the firing does not silence the new threshold. A firing with
/// no entry in `seen` (a record kept by an older build) applies as before.
pub fn apply_locked_deliveries_seen(
    vault: &mut UnlockedVault,
    records: Vec<NoticeRecord>,
    fired_alerts: &[(String, String)],
    seen: &HashMap<String, AlertBase>,
) -> crate::Result<()> {
    vault.transact(|tx| {
        let doc = tx.doc_mut();
        for (alert_id, at) in fired_alerts {
            if let Some(alert) = doc
                .price_alerts
                .iter_mut()
                .find(|alert| &alert.id == alert_id && !alert.is_triggered())
            {
                let unchanged = seen.get(alert_id).is_none_or(|base| {
                    alert.active
                        && alert.condition == base.condition
                        && alert.price == base.price
                        && alert.currency.eq_ignore_ascii_case(&base.currency)
                });
                if unchanged {
                    alert.triggered_at = Some(at.clone());
                    alert.triggered_for = Some(alert.fingerprint());
                }
            }
        }
        let mut known: HashSet<String> = doc
            .notification_log
            .iter()
            .map(|record| record.key.clone())
            .collect();
        let notifications = &doc.settings.notifications;
        let fresh: Vec<NoticeRecord> = records
            .into_iter()
            .filter(|record| notifications.category(record.category).enabled)
            .filter(|record| known.insert(record.key.clone()))
            .collect();
        doc.notification_log.extend(fresh);
        let overflow = doc.notification_log.len().saturating_sub(MAX_NOTICE_LOG);
        doc.notification_log.drain(..overflow);
        tx.touch();
        Ok(())
    })
}
