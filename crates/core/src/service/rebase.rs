//! Shim of the `rebase` area: moved to `crate::money::rebase` (spec
//! core-strangler, D1). Every method is a query: the merge of an edit onto the
//! stored entity, with nothing written.

use crate::error::Result;
use crate::ipc_contract::{
    UpdateAutomationInput, UpdateBudgetInput, UpdateCategoryInput, UpdateContactInput,
    UpdateEnvelopeInput, UpdateGoalInput, UpdatePriceAlertInput, UpdateReminderInput,
    UpdateStoreInput, UpdateUtilityInput,
};

use super::UnlockedVault;
use super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// The edited transaction merged onto the stored one (same type on all
        /// three sides). Call with the transaction built from the form; `base` is
        /// the stored version the form loaded (`UpdateTransactionInput.base`).
        pub fn rebase_transaction(&self, id: &str, base: Option<&crate::model::transaction::Transaction>, edited: crate::model::transaction::Transaction) -> Result<crate::model::transaction::Transaction>
            => crate::money::rebase::rebase_transaction @doc;
        /// With `input.base` set, replace the form's fields by the merge of the
        /// edit onto the current store; without it the input is returned as is.
        /// An entity that is gone is left to the update itself (`Missing`).
        pub fn rebase_store_input(&self, mut input: UpdateStoreInput) -> Result<UpdateStoreInput>
            => crate::usecases::money::rebase::rebase_store_input @doc;
        pub fn rebase_contact_input(&self, input: UpdateContactInput) -> Result<UpdateContactInput>
            => crate::usecases::money::rebase::rebase_contact_input @doc;
        pub fn rebase_category_input(&self, mut input: UpdateCategoryInput) -> Result<UpdateCategoryInput>
            => crate::usecases::money::rebase::rebase_category_input @doc;
        pub fn rebase_automation_input(&self, input: UpdateAutomationInput) -> Result<UpdateAutomationInput>
            => crate::usecases::money::rebase::rebase_automation_input;
        pub fn rebase_budget_input(&self, input: UpdateBudgetInput) -> Result<UpdateBudgetInput>
            => crate::usecases::money::rebase::rebase_budget_input @doc;
        pub fn rebase_goal_input(&self, input: UpdateGoalInput) -> Result<UpdateGoalInput>
            => crate::usecases::money::rebase::rebase_goal_input @doc;
        pub fn rebase_reminder_input(&self, input: UpdateReminderInput) -> Result<UpdateReminderInput>
            => crate::usecases::money::rebase::rebase_reminder_input @doc;
        pub fn rebase_price_alert_input(&self, input: UpdatePriceAlertInput) -> Result<UpdatePriceAlertInput>
            => crate::usecases::money::rebase::rebase_price_alert_input @doc;
        pub fn rebase_utility_input(&self, input: UpdateUtilityInput) -> Result<UpdateUtilityInput>
            => crate::usecases::money::rebase::rebase_utility_input @doc;
        pub fn rebase_envelope_input(&self, input: UpdateEnvelopeInput) -> Result<UpdateEnvelopeInput>
            => crate::usecases::money::rebase::rebase_envelope_input @doc;
    }
    // forward:end
}
