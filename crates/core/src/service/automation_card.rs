//! Shim of the `automation_card` area: the facts built from the document moved to
//! `crate::planning::automation_card`, the scenarios that read workflow graphs to
//! `crate::usecases::planning::automation_card` (spec core-strangler, D1).

pub use crate::planning::automation_card::{
    AutomationCard, CardAction, CardMoney, CardTrigger, JournalEntry, JournalStatus,
};

use crate::error::Result;

use super::UnlockedVault;
use super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// Every run and every reminder or alert shown, the newest first.
        pub fn automation_journal(&self, limit: usize) -> Vec<JournalEntry>
            => crate::usecases::planning::automation_card::automation_journal;
        /// The card of a scheme or a repeat (both are workflows).
        pub fn automation_card(&self, id: &str, now: i64, zone: &str) -> Result<AutomationCard>
            => crate::usecases::planning::automation_card::automation_card;
    }
    // forward:end
}
