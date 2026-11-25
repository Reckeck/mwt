//! Scenarios of the categories area: each decides in `money::categories`, then
//! ends in one `vault.transact`. Rules go through `tx.collection`; a category
//! is keyed by `key`, not `id`, so it is written on the document itself.

use crate::error::{Error, Result};
use crate::kernel::tx::Tx;
use crate::model::category::{Category, CategoryGroup};
use crate::model::planning::CategorizationRule;
use crate::money::categories::{
    check_rule, new_rule, plan_archive_category, plan_create_category, plan_delete_category,
    plan_rule_order, plan_update_category,
};
use crate::service::Session;
use mwt_model::PeriodRule;

/// Put `category` in place of the one with its key.
fn replace_category(tx: &mut Tx<'_>, category: Category) -> Result<()> {
    let slot = tx
        .doc_mut()
        .categories
        .iter_mut()
        .find(|known| known.key == category.key)
        .ok_or_else(|| Error::Missing(format!("category {}", category.key)))?;
    *slot = category;
    Ok(())
}

/// Add a category. The key is taken from `key` or derived from the name,
/// made unique, and never changes afterwards.
pub fn create_category(
    vault: &mut Session,
    key: Option<&str>,
    name: &str,
    group: CategoryGroup,
) -> Result<Category> {
    create_category_ruled(vault, key, name, group, None)
}

/// [`create_category`] with the period rule its operations get when they have none (spec D3).
pub fn create_category_ruled(
    vault: &mut Session,
    key: Option<&str>,
    name: &str,
    group: CategoryGroup,
    period_rule: Option<PeriodRule>,
) -> Result<Category> {
    let mut category = plan_create_category(&vault.doc, key, name, group)?;
    category.period_rule = period_rule;
    vault.transact(|tx| {
        tx.doc_mut().categories.push(category.clone());
        Ok(category)
    })
}

/// Rename or regroup. Transactions store the key, so they follow along.
/// An empty name restores the built-in label; `essential: None` and
/// `passive: None` keep the flags as they are.
pub fn update_category(
    vault: &mut Session,
    key: &str,
    name: Option<&str>,
    group: CategoryGroup,
    essential: Option<bool>,
    passive: Option<bool>,
) -> Result<Category> {
    let category = plan_update_category(&vault.doc, key, name, group, essential, passive)?;
    vault.transact(|tx| {
        replace_category(tx, category.clone())?;
        Ok(category)
    })
}

/// [`update_category`] that also sets the period rule as given; `None` clears it.
pub fn update_category_ruled(
    vault: &mut Session,
    key: &str,
    name: Option<&str>,
    group: CategoryGroup,
    flags: (Option<bool>, Option<bool>),
    period_rule: Option<PeriodRule>,
) -> Result<Category> {
    let mut category = plan_update_category(&vault.doc, key, name, group, flags.0, flags.1)?;
    category.period_rule = period_rule;
    vault.transact(|tx| {
        replace_category(tx, category.clone())?;
        Ok(category)
    })
}

pub fn archive_category(vault: &mut Session, key: &str, archived: bool) -> Result<()> {
    let category = plan_archive_category(&vault.doc, key, archived)?;
    vault.transact(|tx| replace_category(tx, category))
}

/// Only an unused category can be deleted; otherwise archive it. The
/// automations are the planning area's: read through the vault (OD6).
pub fn delete_category(vault: &mut Session, key: &str) -> Result<()> {
    let automations = crate::usecases::planning::automations::automations(vault);
    plan_delete_category(&vault.doc, key, &automations)?;
    vault.transact(|tx| {
        tx.doc_mut()
            .categories
            .retain(|category| category.key != key);
        Ok(())
    })
}

/// Add a rule at the end of the list.
pub fn create_categorization_rule(
    vault: &mut Session,
    pattern: &str,
    category: &str,
) -> Result<CategorizationRule> {
    check_rule(&vault.doc, pattern, category)?;
    vault.transact(|tx| {
        let rule = new_rule(
            tx.doc(),
            uuid::Uuid::new_v4().to_string(),
            pattern,
            category,
        );
        tx.collection::<CategorizationRule>().insert(rule.clone())?;
        Ok(rule)
    })
}

pub fn update_categorization_rule(
    vault: &mut Session,
    id: &str,
    pattern: &str,
    category: &str,
    active: bool,
) -> Result<CategorizationRule> {
    check_rule(&vault.doc, pattern, category)?;
    vault.transact(|tx| {
        tx.collection::<CategorizationRule>()
            .update(id, |rule| {
                rule.pattern = pattern.trim().to_string();
                rule.category = category.to_string();
                rule.active = active;
                rule.clone()
            })
            .map_err(|_| Error::Missing(format!("rule {id}")))
    })
}

pub fn delete_categorization_rule(vault: &mut Session, id: &str) -> Result<()> {
    vault.transact(|tx| {
        tx.collection::<CategorizationRule>()
            .remove(id)
            .map(|_| ())
            .map_err(|_| Error::Missing(format!("rule {id}")))
    })
}

/// Put the rules in the given order; ids not listed keep their relative
/// order after the listed ones.
pub fn reorder_categorization_rules(vault: &mut Session, ids: &[String]) -> Result<()> {
    let order = plan_rule_order(&vault.doc, ids);
    vault.transact(|tx| {
        let mut rules = tx.collection::<CategorizationRule>();
        for (id, position) in order {
            rules.update(&id, |rule| rule.order = position)?;
        }
        Ok(())
    })
}
