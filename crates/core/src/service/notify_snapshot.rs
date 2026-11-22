//! Shim of the `notify_snapshot` area: moved to `mwt_reports::notify_snapshot` (spec core-strangler, D1).

pub use mwt_reports::notify_snapshot::{
    AlertBase, NotificationSnapshot, SNAPSHOT_DAYS, ScheduledNotice, SnapshotAlert,
};

use std::collections::HashMap;

use crate::model::notify::NoticeRecord;

use super::UnlockedVault;
use super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// Every notice ahead for the next [`SNAPSHOT_DAYS`], to be told while
        /// the vault is locked.
        pub fn notification_snapshot(&self, now: i64, zone: &str, ru: bool) -> NotificationSnapshot
            => mwt_reports::notify_snapshot::notification_snapshot @books;
        /// [`apply_locked_deliveries_seen`] with no record of what the alerts looked
        /// like: every firing applies.
        pub fn apply_locked_deliveries(&mut self, records: Vec<NoticeRecord>, fired_alerts: &[(String, String)]) -> crate::Result<()>
            => crate::usecases::reports::notify_snapshot::apply_locked_deliveries;
        /// Take in what was shown while the vault was locked: logged so it never
        /// repeats; alerts that fired are marked triggered. `seen` holds each
        /// fired alert's threshold as the locked app saw it: an alert that was
        /// changed since (a newer threshold came by sync) or switched off is left
        /// alone, so the firing does not silence the new threshold. A firing with
        /// no entry in `seen` (a record kept by an older build) applies as before.
        pub fn apply_locked_deliveries_seen(&mut self, records: Vec<NoticeRecord>, fired_alerts: &[(String, String)], seen: &HashMap<String, AlertBase>) -> crate::Result<()>
            => crate::usecases::reports::notify_snapshot::apply_locked_deliveries_seen;
    }
    // forward:end
}
