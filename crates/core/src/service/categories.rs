//! Categories (with their budget groups) and the rules that pick one
//! automatically (plan D14, §7.1, §8.1).

use crate::error::{Error, Result};
use crate::model::category::{Category, CategoryGroup};
use crate::model::planning::{BudgetTarget, CategorizationRule, TransactionDraft};

use super::UnlockedVault;

/// A key derived from a label: lowercase ASCII letters, digits and `_`.
/// Cyrillic and other scripts have no ASCII form, so they fall back to a
/// random suffix rather than an empty key.
fn slug(label: &str) -> String {
    let mut out = String::new();
    for character in label.trim().to_lowercase().chars() {
        if character.is_ascii_alphanumeric() {
            out.push(character);
        } else if !out.ends_with('_') && !out.is_empty() {
            out.push('_');
        }
    }
    let out = out.trim_end_matches('_').to_string();
    if out.is_empty() {
        format!("cat_{}", &uuid::Uuid::new_v4().simple().to_string()[..8])
    } else {
        out
    }
}

impl UnlockedVault {
    fn find_category_mut(&mut self, key: &str) -> Result<&mut Category> {
        self.doc
            .categories
            .iter_mut()
            .find(|category| category.key == key)
            .ok_or_else(|| Error::Missing(format!("category {key}")))
    }

    /// Add a category. The key is taken from `key` or derived from the name,
    /// made unique, and never changes afterwards.
    pub fn create_category(
        &mut self,
        key: Option<&str>,
        name: &str,
        group: CategoryGroup,
    ) -> Result<Category> {
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::InvalidInput("a category needs a name".into()));
        }
        let wanted = key.map(slug).unwrap_or_else(|| slug(name));
        let mut candidate = wanted.clone();
        let mut counter = 2;
        while self
            .doc
            .categories
            .iter()
            .any(|category| category.key == candidate)
        {
            candidate = format!("{wanted}_{counter}");
            counter += 1;
        }
        let category = Category {
            key: candidate,
            name: Some(name.to_string()),
            group,
            archived: false,
            essential: None,
            passive: None,
        };
        self.doc.categories.push(category.clone());
        self.persist()?;
        Ok(category)
    }

    /// Rename or regroup. Transactions store the key, so they follow along.
    /// An empty name restores the built-in label; `essential: None` and
    /// `passive: None` keep the flags as they are.
    pub fn update_category(
        &mut self,
        key: &str,
        name: Option<&str>,
        group: CategoryGroup,
        essential: Option<bool>,
        passive: Option<bool>,
    ) -> Result<Category> {
        let category = self.find_category_mut(key)?;
        if essential.is_some() {
            category.essential = essential;
        }
        if passive.is_some() {
            category.passive = passive;
        }
        category.name = name
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_string);
        category.group = group;
        let out = category.clone();
        self.persist()?;
        Ok(out)
    }

    pub fn archive_category(&mut self, key: &str, archived: bool) -> Result<()> {
        self.find_category_mut(key)?.archived = archived;
        self.persist()
    }

    fn category_in_use(&self, key: &str) -> bool {
        let in_draft = |draft: &TransactionDraft| {
            let meta = match draft {
                TransactionDraft::Topup(tx) => &tx.meta,
                TransactionDraft::Payment(tx) => &tx.meta,
                TransactionDraft::Transfer(tx) => &tx.meta,
                TransactionDraft::Convert(tx) => &tx.meta,
            };
            meta.categories.iter().any(|used| used == key)
        };
        self.doc
            .transactions
            .iter()
            .any(|tx| tx.meta().categories.iter().any(|used| used == key))
            || self
                .doc
                .categorization_rules
                .iter()
                .any(|rule| rule.category == key)
            || self
                .doc
                .budgets
                .iter()
                .any(|budget| matches!(&budget.target, BudgetTarget::Category(used) if used == key))
            || self
                .doc
                .utility_items
                .iter()
                .any(|item| item.category == key)
            || self
                .automations()
                .iter()
                .any(|automation| in_draft(&automation.template))
    }

    /// Only an unused category can be deleted; otherwise archive it.
    pub fn delete_category(&mut self, key: &str) -> Result<()> {
        if self.category_in_use(key) {
            return Err(Error::InvalidInput(
                "the category is in use; archive it instead".into(),
            ));
        }
        let before = self.doc.categories.len();
        self.doc.categories.retain(|category| category.key != key);
        if before == self.doc.categories.len() {
            return Err(Error::Missing(format!("category {key}")));
        }
        self.persist()
    }

    // -- categorization rules ----------------------------------------------

    fn check_rule(&self, pattern: &str, category: &str) -> Result<()> {
        if pattern.trim().is_empty() {
            return Err(Error::InvalidInput(
                "a rule needs some text to look for".into(),
            ));
        }
        if !self
            .doc
            .categories
            .iter()
            .any(|known| known.key == category)
        {
            return Err(Error::Missing(format!("category {category}")));
        }
        Ok(())
    }

    /// Rules in the order they are tried.
    pub fn categorization_rules(&self) -> Vec<CategorizationRule> {
        let mut out = self.doc.categorization_rules.clone();
        out.sort_by_key(|rule| rule.order);
        out
    }

    /// Add a rule at the end of the list.
    pub fn create_categorization_rule(
        &mut self,
        pattern: &str,
        category: &str,
    ) -> Result<CategorizationRule> {
        self.check_rule(pattern, category)?;
        let order = self
            .doc
            .categorization_rules
            .iter()
            .map(|rule| rule.order + 1)
            .max()
            .unwrap_or(0);
        let rule = CategorizationRule {
            id: uuid::Uuid::new_v4().to_string(),
            pattern: pattern.trim().to_string(),
            category: category.to_string(),
            active: true,
            order,
        };
        self.doc.categorization_rules.push(rule.clone());
        self.persist()?;
        Ok(rule)
    }

    pub fn update_categorization_rule(
        &mut self,
        id: &str,
        pattern: &str,
        category: &str,
        active: bool,
    ) -> Result<CategorizationRule> {
        self.check_rule(pattern, category)?;
        let rule = self
            .doc
            .categorization_rules
            .iter_mut()
            .find(|rule| rule.id == id)
            .ok_or_else(|| Error::Missing(format!("rule {id}")))?;
        rule.pattern = pattern.trim().to_string();
        rule.category = category.to_string();
        rule.active = active;
        let out = rule.clone();
        self.persist()?;
        Ok(out)
    }

    pub fn delete_categorization_rule(&mut self, id: &str) -> Result<()> {
        let before = self.doc.categorization_rules.len();
        self.doc.categorization_rules.retain(|rule| rule.id != id);
        if before == self.doc.categorization_rules.len() {
            return Err(Error::Missing(format!("rule {id}")));
        }
        self.persist()
    }

    /// Put the rules in the given order; ids not listed keep their relative
    /// order after the listed ones.
    pub fn reorder_categorization_rules(&mut self, ids: &[String]) -> Result<()> {
        let mut rules = self.categorization_rules();
        rules.sort_by_key(|rule| {
            ids.iter()
                .position(|id| id == &rule.id)
                .unwrap_or(usize::MAX)
        });
        for (index, rule) in rules.iter_mut().enumerate() {
            rule.order = index as u32;
        }
        self.doc.categorization_rules = rules;
        self.persist()
    }
}

#[cfg(test)]
mod tests {
    use super::slug;

    #[test]
    fn slugs_are_ascii_keys() {
        assert_eq!(slug("Coffee & Tea"), "coffee_tea");
        assert_eq!(slug("  Rent 2024 "), "rent_2024");
        assert!(slug("Кофе").starts_with("cat_"));
    }
}

#[cfg(test)]
mod vault_tests {
    use super::super::test_support::*;
    use crate::model::category::CategoryGroup;

    #[test]
    fn a_new_vault_has_the_default_categories() {
        let (_dir, vault) = empty_vault();
        let group = |key: &str| {
            vault
                .doc
                .categories
                .iter()
                .find(|category| category.key == key)
                .map(|category| category.group)
        };
        assert_eq!(group("salary"), Some(CategoryGroup::Income));
        assert_eq!(group("rent_mortgage"), Some(CategoryGroup::Fixed));
        assert_eq!(group("taxi"), Some(CategoryGroup::Daily));
        assert_eq!(group("savings"), Some(CategoryGroup::Savings));
        assert_eq!(group("trading"), Some(CategoryGroup::Service));
    }

    #[test]
    fn renaming_keeps_transactions_and_deleting_needs_it_unused() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[1].id.clone();
        let coffee = vault
            .create_category(None, "Coffee", CategoryGroup::Daily)
            .unwrap();
        assert_eq!(coffee.key, "coffee");
        let again = vault
            .create_category(None, "coffee", CategoryGroup::Daily)
            .unwrap();
        assert_eq!(again.key, "coffee_2");

        let mut tx = payment(&store, "3", "latte");
        tx.meta_mut().categories = vec!["coffee".into()];
        vault.add_transaction(tx).unwrap();
        vault
            .update_category("coffee", Some("Кофе"), CategoryGroup::Fixed, None, None)
            .unwrap();
        assert_eq!(vault.doc.transactions[0].meta().categories, vec!["coffee"]);

        assert!(vault.delete_category("coffee").is_err());
        vault.delete_category("coffee_2").unwrap();
        // an empty name returns to the built-in label
        let reset = vault
            .update_category("taxi", Some("  "), CategoryGroup::Daily, None, None)
            .unwrap();
        assert_eq!(reset.name, None);
    }
}
