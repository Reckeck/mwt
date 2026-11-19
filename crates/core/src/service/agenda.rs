//! Shim of the `agenda` area: the rules moved to `crate::planning::agenda`, the scenarios to
//! `crate::usecases::planning::agenda` (spec core-strangler, D1).

pub use crate::planning::agenda::{PriceAlertDraft, UpcomingItem, UpcomingKind};
pub(crate) use crate::planning::agenda::{next_occurrence, reminder_secs};

use crate::error::Result;
use crate::model::planning::{PriceAlert, Reminder, ReminderPay, ReminderRepeat};
use crate::model::store::AssetRef;

use super::UnlockedVault;
use super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        pub fn reminders(&self) -> Vec<Reminder>
            => crate::planning::agenda::reminders @doc;
        pub fn create_reminder(&mut self, text: &str, due_at: &str, repeat: Option<ReminderRepeat>) -> Result<Reminder>
            => crate::usecases::planning::agenda::create_reminder;
        /// A reminder with "Pay" in it.
        pub fn create_reminder_paying(&mut self, text: &str, due_at: &str, repeat: Option<ReminderRepeat>, pay: Option<ReminderPay>) -> Result<Reminder>
            => crate::usecases::planning::agenda::create_reminder_paying;
        /// Edit what and when. An automatic reminder edited by hand keeps its
        /// time from then on: the sync no longer moves it.
        pub fn update_reminder(&mut self, id: &str, text: &str, due_at: &str, repeat: Option<ReminderRepeat>) -> Result<Reminder>
            => crate::usecases::planning::agenda::update_reminder;
        /// Edit what, when and what "Pay" fills in.
        pub fn update_reminder_paying(&mut self, id: &str, text: &str, due_at: &str, repeat: Option<ReminderRepeat>, pay: Option<ReminderPay>) -> Result<Reminder>
            => crate::usecases::planning::agenda::update_reminder_paying;
        /// Done. A repeating reminder is not closed but moves on to its next date
        /// after now — «paid this month» leaves next month's in place.
        pub fn set_reminder_done(&mut self, id: &str, done: bool) -> Result<Reminder>
            => crate::usecases::planning::agenda::set_reminder_done;
        /// Stop a repeating reminder: it is done for good.
        pub fn finish_reminder_series(&mut self, id: &str) -> Result<Reminder>
            => crate::usecases::planning::agenda::finish_reminder_series;
        /// Put a reminder off: it is not due before `until`.
        pub fn snooze_reminder(&mut self, id: &str, until: &str) -> Result<Reminder>
            => crate::usecases::planning::agenda::snooze_reminder;
        /// Delete a reminder. An automatic one is muted rather than removed: its
        /// source would make it again with the next sync, and a muted one stays
        /// gone (and hidden) until the source itself is gone.
        pub fn delete_reminder(&mut self, id: &str) -> Result<()>
            => crate::usecases::planning::agenda::delete_reminder;
        /// Open reminders whose time has come (a snoozed one when its snooze ends).
        pub fn due_reminders(&self, now: i64) -> Vec<Reminder>
            => crate::planning::agenda::due_reminders @doc;
        /// The alerts as the list shows them: a firing recorded for another
        /// threshold is hidden (`triggered_at` cleared) in the answer only; the
        /// stored record keeps it.
        pub fn price_alerts(&self) -> Vec<PriceAlert>
            => crate::planning::agenda::price_alerts @doc;
        pub fn create_price_alert(&mut self, draft: PriceAlertDraft) -> Result<PriceAlert>
            => crate::usecases::planning::agenda::create_price_alert;
        /// Edit an alert; an edited alert is armed again.
        pub fn update_price_alert(&mut self, id: &str, draft: PriceAlertDraft) -> Result<PriceAlert>
            => crate::usecases::planning::agenda::update_price_alert;
        pub fn delete_price_alert(&mut self, id: &str) -> Result<()>
            => crate::usecases::planning::agenda::delete_price_alert;
        /// Assets of armed alerts — what the scheduler refreshes prices for.
        pub fn alert_assets(&self) -> Vec<AssetRef>
            => crate::planning::agenda::alert_assets @doc;
        /// Fire every armed alert whose condition holds at the cached price.
        /// A one-off alert stays triggered until edited; a repeating one is
        /// armed again once the price is back on the other side.
        pub fn check_alerts(&mut self, now: i64) -> Result<Vec<PriceAlert>>
            => crate::usecases::planning::agenda::check_alerts;
        /// What is coming up in the next `days` days, oldest first, plus rejected
        /// automation runs still waiting for a decision.
        pub fn upcoming(&self, days: u32, now: i64, zone: &str) -> Vec<UpcomingItem>
            => crate::usecases::planning::agenda::upcoming;
    }
    // forward:end
}
