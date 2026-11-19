//! Shim of the `my_automations` area: the rows built from the document moved to
//! `crate::planning::my_automations`, the scenarios to `crate::usecases::planning::my_automations`
//! (spec core-strangler, D1).

pub use crate::planning::my_automations::{MineItem, MineKind};

use crate::error::Result;

use super::UnlockedVault;
use super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// Everything that fires on its own, the soonest first; alerts and
        /// schemes without a date after them, by name.
        pub fn my_automations(&self, now: i64, zone: &str) -> Vec<MineItem>
            => crate::usecases::planning::my_automations::my_automations;
        /// The "On" switch of a row: a scheme or a repeat is switched like a
        /// workflow (one with errors stays off), a reminder is muted, an alert
        /// switched back on waits for the price again.
        pub fn set_my_automation_active(&mut self, kind: MineKind, id: &str, active: bool, now: i64) -> Result<()>
            => crate::usecases::planning::my_automations::set_my_automation_active;
    }
    // forward:end
}
