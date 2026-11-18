//! Shim of the `contacts` area: moved to `crate::household::contacts` (spec core-strangler, D1).

pub use crate::household::contacts::{
    ContactBalance, ContactDraft, ContactSummary, ContactTurnover, DebtTermsDraft, TurnoverRow,
};

use crate::error::Result;
use crate::model::contact::Contact;

use super::UnlockedVault;
use super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        pub fn create_contact(&mut self, draft: ContactDraft) -> Result<Contact>
            => crate::usecases::household::contacts::create_contact;
        /// Replace a contact's fields and accounts. An account that disappeared
        /// from the draft but is used by a transaction is kept, deactivated.
        pub fn update_contact(&mut self, id: &str, draft: ContactDraft) -> Result<Contact>
            => crate::usecases::household::contacts::update_contact;
        pub fn archive_contact(&mut self, id: &str, archived: bool) -> Result<()>
            => crate::usecases::household::contacts::archive_contact;
        /// Only a contact nothing refers to can be deleted; otherwise archive it. Its
        /// debt terms go with it, in the same transaction.
        pub fn delete_contact(&mut self, id: &str) -> Result<()>
            => crate::usecases::household::contacts::delete_contact;
        /// What was sent to and received from a contact, per asset. Internal
        /// moves never count: they did not leave the user's hands.
        pub fn contact_turnover(&self, contact_id: &str, account_id: Option<&str>) -> Result<ContactTurnover>
            => crate::usecases::household::contacts::contact_turnover;
        /// One summary per contact: operation count, the latest operation and the
        /// debts converted to the base currency with the cached rates (no network).
        pub fn contact_summaries(&self) -> Result<Vec<ContactSummary>>
            => crate::usecases::household::contacts::contact_summaries;
        /// Personal debts per contact and asset, from transfers marked as debts only.
        pub fn contact_balances(&self) -> Vec<ContactBalance>
            => crate::usecases::household::contacts::contact_balances;
        /// Personal debts from debt transfers made up to `secs`, plus what the
        /// shared events stood at then.
        pub(crate) fn contact_balances_until(&self, secs: i64) -> Vec<ContactBalance>
            => crate::usecases::household::contacts::contact_balances_until;
        pub fn debt_terms(&self, contact_id: Option<&str>) -> Vec<crate::model::contact::DebtTerms>
            => crate::household::contacts::debt_terms @doc;
        /// Set (or replace) the terms of one contact's debt in one asset; an
        /// empty date and note removes them.
        pub fn set_debt_terms(&mut self, draft: DebtTermsDraft) -> Result<Option<crate::model::contact::DebtTerms>>
            => crate::usecases::household::contacts::set_debt_terms;
    }
    // forward:end
}
