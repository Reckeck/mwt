//! The utility items: what a bill is, and the checks behind creating, editing
//! and deleting one. The writes are the scenarios in
//! `usecases::household::utilities`.

use crate::error::{Error, Result};
use crate::model::VaultDoc;
use crate::model::planning::{UtilityItem, UtilityKind};
use crate::model::store::StoreKind;
use crate::money::tree::find_store;
use crate::utilities_formula;

use super::types::UtilityDraft;

pub fn utility_items(doc: &VaultDoc) -> Vec<UtilityItem> {
    let mut out = doc.utility_items.clone();
    out.sort_by_key(|item| item.name.to_lowercase());
    out
}

pub fn utility_item<'doc>(doc: &'doc VaultDoc, id: &str) -> Result<&'doc UtilityItem> {
    doc.utility_items
        .iter()
        .find(|item| item.id == id)
        .ok_or_else(|| Error::Missing(format!("utility {id}")))
}

/// A meter as stored: zones trimmed, an empty formula dropped.
fn tidy_meter(kind: &mut UtilityKind) {
    if let UtilityKind::Metered { formula, zones, .. } = kind {
        *formula = formula
            .as_deref()
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_string);
        *zones = zones
            .iter()
            .map(|zone| zone.trim().to_string())
            .filter(|zone| !zone.is_empty())
            .collect();
    }
}

/// The checks behind a draft. `known_automation` says whether an automation
/// with this id exists (the automations are not part of this area).
pub fn check_utility(
    doc: &VaultDoc,
    draft: &UtilityDraft,
    known_automation: &dyn Fn(&str) -> bool,
) -> Result<()> {
    if draft.name.trim().is_empty() {
        return Err(Error::InvalidInput("name the bill".into()));
    }
    if draft.currency.trim().is_empty() {
        return Err(Error::InvalidInput("choose a currency".into()));
    }
    if !doc
        .categories
        .iter()
        .any(|category| category.key == draft.category)
    {
        return Err(Error::Missing(format!("category {}", draft.category)));
    }
    let is_property = find_store(doc, &draft.property_id)
        .is_some_and(|store| store.kind == StoreKind::Property && store.parent_id.is_none());
    if !is_property {
        return Err(Error::Missing(format!("property {}", draft.property_id)));
    }
    if let Some(id) = &draft.store_id
        && find_store(doc, id).is_none()
    {
        return Err(Error::Missing(format!("store {id}")));
    }
    match &draft.kind {
        UtilityKind::Metered {
            unit,
            tariff,
            formula,
            zones,
        } => {
            let formula = formula
                .as_deref()
                .map(str::trim)
                .filter(|text| !text.is_empty());
            // With a formula the tariff is just one of its names, and may be 0.
            if unit.trim().is_empty()
                || tariff.is_negative()
                || (formula.is_none() && !tariff.is_positive())
            {
                return Err(Error::InvalidInput(
                    "a meter needs a unit and a tariff".into(),
                ));
            }
            let keys: Vec<String> = zones
                .iter()
                .map(|zone| utilities_formula::zone_key(zone))
                .filter(|key| !key.is_empty())
                .collect();
            let unique: std::collections::BTreeSet<&String> = keys.iter().collect();
            if unique.len() != keys.len() || keys.len() > 4 {
                return Err(Error::InvalidInput(
                    "zones: up to 4, each named once".into(),
                ));
            }
            if let Some(text) = formula
                && let Err(issue) = utilities_formula::check(text, zones)
            {
                return Err(Error::InvalidInput(format!(
                    "formula, character {}: {}",
                    issue.position, issue.text_en
                )));
            }
        }
        UtilityKind::Static { amount } if !amount.is_positive() => {
            return Err(Error::InvalidInput("the amount must be positive".into()));
        }
        UtilityKind::Housing {
            amount,
            deposit,
            due_day,
            automation_id,
        } => {
            if !amount.is_positive() || deposit.is_some_and(|value| value.is_negative()) {
                return Err(Error::InvalidInput("rent must be positive".into()));
            }
            if !(1..=31).contains(due_day) {
                return Err(Error::InvalidInput("the due day is 1–31".into()));
            }
            if let Some(id) = automation_id
                && !known_automation(id)
            {
                return Err(Error::Missing(format!("automation {id}")));
            }
        }
        UtilityKind::Static { .. } => {}
    }
    Ok(())
}

/// The item a draft makes under `id`: name and currency tidied, a meter's
/// zones and formula as stored.
fn item_of(id: String, mut draft: UtilityDraft) -> UtilityItem {
    tidy_meter(&mut draft.kind);
    UtilityItem {
        id,
        name: draft.name.trim().to_string(),
        active: draft.active,
        kind: draft.kind,
        currency: draft.currency.trim().to_lowercase(),
        property_id: draft.property_id,
        store_id: draft.store_id,
        category: draft.category,
    }
}

/// The new item a draft makes, once it passes the checks.
pub fn plan_create_utility(
    doc: &VaultDoc,
    draft: UtilityDraft,
    known_automation: &dyn Fn(&str) -> bool,
) -> Result<UtilityItem> {
    check_utility(doc, &draft, known_automation)?;
    Ok(item_of(uuid::Uuid::new_v4().to_string(), draft))
}

/// What item `id` becomes under a draft: the checks first, then the item must exist.
pub fn plan_update_utility(
    doc: &VaultDoc,
    id: &str,
    draft: UtilityDraft,
    known_automation: &dyn Fn(&str) -> bool,
) -> Result<UtilityItem> {
    check_utility(doc, &draft, known_automation)?;
    utility_item(doc, id)?;
    Ok(item_of(id.to_string(), draft))
}

/// The ids of the readings that go with item `id` when it is deleted;
/// payments already made stay.
pub fn plan_delete_utility(doc: &VaultDoc, id: &str) -> Result<Vec<String>> {
    utility_item(doc, id)?;
    Ok(doc
        .meter_readings
        .iter()
        .filter(|reading| reading.item_id == id)
        .map(|reading| reading.id.clone())
        .collect())
}
