//! The edit forms of the rebase area: a form that loaded an entity and sends
//! its whole input has the input merged onto what the document holds now, so a
//! remote change to a field the user left alone is not overwritten. The merge
//! itself (`three_way`, `rebase`) is in `money::rebase`; here it is applied to
//! the input of each entity. Every function is a query: nothing is written.

use crate::error::Result;
use crate::ipc_contract::{
    UpdateAutomationInput, UpdateBudgetInput, UpdateCategoryInput, UpdateContactInput,
    UpdateEnvelopeInput, UpdateGoalInput, UpdatePriceAlertInput, UpdateReminderInput,
    UpdateStoreInput, UpdateUtilityInput,
};
use crate::model::vault::VaultDoc;
use crate::money::rebase::{pick, rebased, three_way};
use crate::money::tree::find_store;
use crate::service::Session;

use crate::usecases::planning::automations::automation;

/// With `input.base` set, replace the form's fields by the merge of the
/// edit onto the current store; without it the input is returned as is.
/// An entity that is gone is left to the update itself (`Missing`).
pub fn rebase_store_input(doc: &VaultDoc, mut input: UpdateStoreInput) -> Result<UpdateStoreInput> {
    let (Some(base), Some(current)) = (input.base.take(), find_store(doc, &input.id)) else {
        input.base = None;
        return Ok(input);
    };
    input.name = pick(&base.name, &input.name, &current.name);
    input.color = pick(&base.color, &input.color, &current.color);
    input.note = pick(&base.note, &input.note, &current.note);
    // `None` keeps the stored value, so only a sent value is merged.
    input.purpose = input
        .purpose
        .map(|purpose| pick(&base.purpose, &purpose, &current.purpose));
    input.binding = input
        .binding
        .map(|binding| pick(&base.binding, &binding, &current.binding));
    input.liability = pick(&base.liability, &input.liability, &current.liability);
    input.contact_id = pick(&base.contact_id, &input.contact_id, &current.contact_id);
    input.secured_by = pick(&base.secured_by, &input.secured_by, &current.secured_by);
    if let Some(custody) = input.custody.take() {
        let merged = three_way(&base.custody, &Some(custody), &current.custody)?;
        input.custody = merged;
    }
    if let Some(property) = input.property.take() {
        let merged = three_way(&base.property, &Some(property), &current.property)?;
        input.property = merged;
    }
    if let Some(provider) = input.provider.take() {
        let provider_of = |store: &crate::model::store::Store| {
            store.exchange.as_ref().map(|profile| profile.provider)
        };
        input.provider = pick(&provider_of(&base), &Some(provider), &provider_of(current));
    }
    Ok(input)
}

pub fn rebase_contact_input(
    doc: &VaultDoc,
    input: UpdateContactInput,
) -> Result<UpdateContactInput> {
    let current = doc.contacts.iter().find(|item| item.id == input.id);
    let contact = rebased(input.base.as_ref(), &input.contact, current)?;
    Ok(UpdateContactInput {
        contact,
        base: None,
        ..input
    })
}

pub fn rebase_category_input(
    doc: &VaultDoc,
    mut input: UpdateCategoryInput,
) -> Result<UpdateCategoryInput> {
    let (Some(base), Some(current)) = (
        input.base.take(),
        doc.categories.iter().find(|item| item.key == input.key),
    ) else {
        input.base = None;
        return Ok(input);
    };
    let clean = |name: &Option<String>| {
        name.as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_string)
    };
    let name = pick(&base.name, &clean(&input.name), &current.name);
    input.name = Some(name.unwrap_or_default());
    input.group = pick(&base.group, &input.group, &current.group);
    input.essential = input
        .essential
        .and_then(|flag| pick(&base.essential, &Some(flag), &current.essential));
    input.passive = input
        .passive
        .and_then(|flag| pick(&base.passive, &Some(flag), &current.passive));
    input.period_rule = pick(&base.period_rule, &input.period_rule, &current.period_rule);
    Ok(input)
}

pub fn rebase_automation_input(
    vault: &Session,
    input: UpdateAutomationInput,
) -> Result<UpdateAutomationInput> {
    let current = automation(vault, &input.id).ok();
    let automation = rebased(input.base.as_ref(), &input.automation, current.as_ref())?;
    Ok(UpdateAutomationInput {
        automation,
        base: None,
        ..input
    })
}

pub fn rebase_budget_input(doc: &VaultDoc, input: UpdateBudgetInput) -> Result<UpdateBudgetInput> {
    let current = doc.budgets.iter().find(|item| item.id == input.id);
    let budget = rebased(input.base.as_ref(), &input.budget, current)?;
    Ok(UpdateBudgetInput {
        budget,
        base: None,
        ..input
    })
}

pub fn rebase_goal_input(doc: &VaultDoc, input: UpdateGoalInput) -> Result<UpdateGoalInput> {
    let current = doc.goals.iter().find(|item| item.id == input.id);
    let goal = rebased(input.base.as_ref(), &input.goal, current)?;
    Ok(UpdateGoalInput {
        goal,
        base: None,
        ..input
    })
}

pub fn rebase_reminder_input(
    doc: &VaultDoc,
    input: UpdateReminderInput,
) -> Result<UpdateReminderInput> {
    let current = doc.reminders.iter().find(|item| item.id == input.id);
    let reminder = rebased(input.base.as_ref(), &input.reminder, current)?;
    Ok(UpdateReminderInput {
        reminder,
        base: None,
        ..input
    })
}

pub fn rebase_price_alert_input(
    doc: &VaultDoc,
    input: UpdatePriceAlertInput,
) -> Result<UpdatePriceAlertInput> {
    let current = doc.price_alerts.iter().find(|item| item.id == input.id);
    let alert = rebased(input.base.as_ref(), &input.alert, current)?;
    Ok(UpdatePriceAlertInput {
        alert,
        base: None,
        ..input
    })
}

pub fn rebase_utility_input(
    doc: &VaultDoc,
    input: UpdateUtilityInput,
) -> Result<UpdateUtilityInput> {
    let current = doc.utility_items.iter().find(|item| item.id == input.id);
    let item = rebased(input.base.as_ref(), &input.item, current)?;
    Ok(UpdateUtilityInput {
        item,
        base: None,
        ..input
    })
}

pub fn rebase_envelope_input(
    doc: &VaultDoc,
    input: UpdateEnvelopeInput,
) -> Result<UpdateEnvelopeInput> {
    let current = doc.envelopes.iter().find(|item| item.id == input.id);
    let draft = rebased(input.base.as_ref(), &input.draft, current)?;
    Ok(UpdateEnvelopeInput {
        draft,
        base: None,
        ..input
    })
}
