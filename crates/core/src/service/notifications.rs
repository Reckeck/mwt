//! Shim of the `notifications` area: the rules moved to `crate::planning::notifications`, the
//! scenarios to `crate::usecases::planning::notifications` (spec core-strangler, D1).

pub use crate::model::notify::{notice, notice_title};

use crate::error::Result;
use crate::model::notify::{NoticeCategory, NoticeRecord};

use super::UnlockedVault;
use super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// Whether notices of this kind are wanted at all.
        pub fn notices_enabled(&self, category: NoticeCategory) -> bool
            => crate::planning::notifications::notices_enabled @doc;
        /// Every notice shown so far, newest first.
        pub fn notifications(&self) -> Vec<NoticeRecord>
            => crate::planning::notifications::notifications @doc;
        pub fn unread_notifications(&self) -> u32
            => crate::planning::notifications::unread_notifications @doc;
        /// Mark notices read: the ones named, or all of them.
        pub fn mark_notifications_read(&mut self, keys: Option<&[String]>) -> Result<()>
            => crate::usecases::planning::notifications::mark_notifications_read;
        /// Add notices to the log, skipping known keys and switched-off kinds.
        /// Returns the ones that are new; persists once.
        pub fn log_notices(&mut self, records: Vec<NoticeRecord>) -> Result<Vec<NoticeRecord>>
            => crate::usecases::planning::notifications::log_notices;
        /// What is due to be told now that was not told before: reminders whose
        /// time came, planned money going out the balance will not cover, budgets
        /// near or over their limit. They are logged before being returned.
        pub fn due_notices(&mut self, now: i64, zone: &str, ru: bool) -> Result<Vec<NoticeRecord>>
            => crate::usecases::planning::notifications::due_notices;
        /// A notice shown elsewhere (an alert fired while locked, an automation
        /// run, a Bybit import): logged so it is not shown again.
        pub fn record_notice(&mut self, record: NoticeRecord) -> Result<bool>
            => crate::usecases::planning::notifications::record_notice;
    }
    // forward:end
}
