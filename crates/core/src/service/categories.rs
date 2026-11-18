//! Shim of the `categories` area: moved to `crate::money::categories` (spec core-strangler, D1).

use crate::error::Result;
use crate::model::category::{Category, CategoryGroup};
use crate::model::planning::CategorizationRule;

use super::UnlockedVault;
use super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// Rules in the order they are tried.
        pub fn categorization_rules(&self) -> Vec<CategorizationRule>
            => crate::money::categories::categorization_rules @doc;
        /// Add a category. The key is taken from `key` or derived from the name,
        /// made unique, and never changes afterwards.
        pub fn create_category(&mut self, key: Option<&str>, name: &str, group: CategoryGroup) -> Result<Category>
            => crate::usecases::household::categories::create_category;
        /// Rename or regroup. Transactions store the key, so they follow along.
        /// An empty name restores the built-in label; `essential: None` and
        /// `passive: None` keep the flags as they are.
        pub fn update_category(&mut self, key: &str, name: Option<&str>, group: CategoryGroup, essential: Option<bool>, passive: Option<bool>) -> Result<Category>
            => crate::usecases::household::categories::update_category;
        pub fn archive_category(&mut self, key: &str, archived: bool) -> Result<()>
            => crate::usecases::household::categories::archive_category;
        /// Only an unused category can be deleted; otherwise archive it. The
        /// automations are the planning area's: read through the vault (OD6).
        pub fn delete_category(&mut self, key: &str) -> Result<()>
            => crate::usecases::household::categories::delete_category;
        /// Add a rule at the end of the list.
        pub fn create_categorization_rule(&mut self, pattern: &str, category: &str) -> Result<CategorizationRule>
            => crate::usecases::household::categories::create_categorization_rule;
        pub fn update_categorization_rule(&mut self, id: &str, pattern: &str, category: &str, active: bool) -> Result<CategorizationRule>
            => crate::usecases::household::categories::update_categorization_rule;
        pub fn delete_categorization_rule(&mut self, id: &str) -> Result<()>
            => crate::usecases::household::categories::delete_categorization_rule;
        /// Put the rules in the given order; ids not listed keep their relative
        /// order after the listed ones.
        pub fn reorder_categorization_rules(&mut self, ids: &[String]) -> Result<()>
            => crate::usecases::household::categories::reorder_categorization_rules;
    }
    // forward:end
}
