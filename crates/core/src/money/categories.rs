//! Categories (with their budget groups) and the rules that pick one
//! automatically (plan D14, §7.1, §8.1).
//!
//! Pure part of the area: decisions over a document. The writes are the
//! scenarios in `usecases::household::categories`.

use crate::error::{Error, Result};
use crate::model::category::{Category, CategoryGroup};
use crate::model::planning::{Automation, BudgetTarget, CategorizationRule, TransactionDraft};
use crate::model::vault::VaultDoc;

const SYSTEM_CATEGORY: &str = "system categories are set by the app and cannot be changed";

/// A key derived from a label: lowercase ASCII letters, digits and `_`.
/// Cyrillic and other scripts have no ASCII form, so they fall back to a
/// random suffix rather than an empty key.
pub(super) fn slug(label: &str) -> String {
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

fn find_category<'doc>(doc: &'doc VaultDoc, key: &str) -> Result<&'doc Category> {
    doc.categories
        .iter()
        .find(|category| category.key == key)
        .ok_or_else(|| Error::Missing(format!("category {key}")))
}

/// The app's own categories (group `Service`) back reports, exchanges and
/// imports: they are never renamed, regrouped, archived or deleted.
pub fn ensure_editable(doc: &VaultDoc, key: &str) -> Result<()> {
    let system = doc
        .categories
        .iter()
        .any(|category| category.key == key && category.group == CategoryGroup::Service);
    if system {
        return Err(Error::InvalidInput(SYSTEM_CATEGORY.into()));
    }
    Ok(())
}

/// Add a category: the one to write. The key is taken from `key` or derived
/// from the name, made unique, and never changes afterwards.
pub fn plan_create_category(
    doc: &VaultDoc,
    key: Option<&str>,
    name: &str,
    group: CategoryGroup,
) -> Result<Category> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::InvalidInput("a category needs a name".into()));
    }
    if group == CategoryGroup::Service {
        return Err(Error::InvalidInput(SYSTEM_CATEGORY.into()));
    }
    let wanted = key.map(slug).unwrap_or_else(|| slug(name));
    let mut candidate = wanted.clone();
    let mut counter = 2;
    while doc
        .categories
        .iter()
        .any(|category| category.key == candidate)
    {
        candidate = format!("{wanted}_{counter}");
        counter += 1;
    }
    Ok(Category {
        key: candidate,
        name: Some(name.to_string()),
        group,
        archived: false,
        essential: None,
        passive: None,
    })
}

/// Rename or regroup: the category to write. Transactions store the key, so
/// they follow along. An empty name restores the built-in label;
/// `essential: None` and `passive: None` keep the flags as they are.
pub fn plan_update_category(
    doc: &VaultDoc,
    key: &str,
    name: Option<&str>,
    group: CategoryGroup,
    essential: Option<bool>,
    passive: Option<bool>,
) -> Result<Category> {
    let mut category = find_category(doc, key)?.clone();
    ensure_editable(doc, key)?;
    if group == CategoryGroup::Service {
        return Err(Error::InvalidInput(SYSTEM_CATEGORY.into()));
    }
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
    Ok(category)
}

/// Archive or restore: the category to write.
pub fn plan_archive_category(doc: &VaultDoc, key: &str, archived: bool) -> Result<Category> {
    let mut category = find_category(doc, key)?.clone();
    ensure_editable(doc, key)?;
    category.archived = archived;
    Ok(category)
}

fn category_in_use(doc: &VaultDoc, key: &str, automations: &[Automation]) -> bool {
    let in_draft = |draft: &TransactionDraft| {
        let meta = match draft {
            TransactionDraft::Topup(tx) => &tx.meta,
            TransactionDraft::Payment(tx) => &tx.meta,
            TransactionDraft::Transfer(tx) => &tx.meta,
            TransactionDraft::Convert(tx) => &tx.meta,
        };
        meta.categories.iter().any(|used| used == key)
    };
    doc.transactions
        .iter()
        .any(|tx| tx.meta().categories.iter().any(|used| used == key))
        || doc
            .categorization_rules
            .iter()
            .any(|rule| rule.category == key)
        || doc
            .budgets
            .iter()
            .any(|budget| matches!(&budget.target, BudgetTarget::Category(used) if used == key))
        || doc.utility_items.iter().any(|item| item.category == key)
        || automations
            .iter()
            .any(|automation| in_draft(&automation.template))
}

/// Only an unused category can be deleted; otherwise archive it.
/// `automations` are the vault's, which the planning area reads.
pub fn plan_delete_category(doc: &VaultDoc, key: &str, automations: &[Automation]) -> Result<()> {
    ensure_editable(doc, key)?;
    if category_in_use(doc, key, automations) {
        return Err(Error::InvalidInput(
            "the category is in use; archive it instead".into(),
        ));
    }
    find_category(doc, key).map(|_| ())
}

// -- categorization rules ---------------------------------------------------

pub fn check_rule(doc: &VaultDoc, pattern: &str, category: &str) -> Result<()> {
    if pattern.trim().is_empty() {
        return Err(Error::InvalidInput(
            "a rule needs some text to look for".into(),
        ));
    }
    if !doc.categories.iter().any(|known| known.key == category) {
        return Err(Error::Missing(format!("category {category}")));
    }
    Ok(())
}

/// Rules in the order they are tried.
pub fn categorization_rules(doc: &VaultDoc) -> Vec<CategorizationRule> {
    let mut out = doc.categorization_rules.clone();
    out.sort_by_key(|rule| rule.order);
    out
}

/// The `order` of each rule after putting `ids` first; ids not listed keep
/// their relative order after the listed ones.
pub fn plan_rule_order(doc: &VaultDoc, ids: &[String]) -> Vec<(String, u32)> {
    let mut rules = categorization_rules(doc);
    rules.sort_by_key(|rule| {
        ids.iter()
            .position(|id| id == &rule.id)
            .unwrap_or(usize::MAX)
    });
    rules
        .into_iter()
        .enumerate()
        .map(|(index, rule)| (rule.id, index as u32))
        .collect()
}

/// A new rule at the end of the list.
pub fn new_rule(doc: &VaultDoc, id: String, pattern: &str, category: &str) -> CategorizationRule {
    let order = doc
        .categorization_rules
        .iter()
        .map(|rule| rule.order + 1)
        .max()
        .unwrap_or(0);
    CategorizationRule {
        id,
        pattern: pattern.trim().to_string(),
        category: category.to_string(),
        active: true,
        order,
    }
}
