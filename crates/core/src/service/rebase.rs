//! Edit forms send only what the user changed.
//!
//! A form loads an entity and keeps that snapshot (`base`). When the user
//! saves, the form sends its whole input; if sync changed another field in the
//! meantime, applying the input as is would overwrite that remote change with
//! the stale value the form loaded. [`three_way`] keeps the current value of
//! every field the user did not touch and takes the edited value of every field
//! they did. Nothing here changes the stored vault schema.

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::error::Result;
use crate::ipc_contract::{
    UpdateAutomationInput, UpdateBudgetInput, UpdateCategoryInput, UpdateContactInput,
    UpdateEnvelopeInput, UpdateGoalInput, UpdatePriceAlertInput, UpdateReminderInput,
    UpdateStoreInput, UpdateUtilityInput,
};
use crate::service::UnlockedVault;

/// One value of the three-way merge. Equal to `base` means the user did not
/// touch it: `current` stays. Objects are merged key by key (sync flattens them
/// per field too), arrays and scalars are taken whole.
fn merge_value(base: Option<&Value>, edited: &Value, current: Option<&Value>) -> Value {
    if base == Some(edited) {
        return current.cloned().unwrap_or_else(|| edited.clone());
    }
    match (base, edited, current) {
        (Some(Value::Object(base)), Value::Object(edited), Some(Value::Object(current))) => {
            let mut out = current.clone();
            for (key, value) in edited {
                out.insert(
                    key.clone(),
                    merge_value(base.get(key), value, current.get(key)),
                );
            }
            for key in base.keys() {
                if !edited.contains_key(key) {
                    out.remove(key);
                }
            }
            Value::Object(out)
        }
        _ => edited.clone(),
    }
}

/// [`three_way`] where the current entity has another type than the input (a
/// `Goal` against a `GoalInput`): only the keys of the input are touched. A key
/// whose current value does not fit the input type (the shapes differ) keeps
/// the edited value, so the result is always a valid `Input`.
pub fn rebase<Input, Current>(base: &Input, edited: &Input, current: &Current) -> Result<Input>
where
    Input: Serialize + DeserializeOwned,
    Current: Serialize,
{
    let base = serde_json::to_value(base)?;
    let edited_value = serde_json::to_value(edited)?;
    let current = serde_json::to_value(current)?;
    let (Value::Object(base), Value::Object(edited_map), Value::Object(current)) =
        (base, &edited_value, current)
    else {
        return Ok(serde_json::from_value(edited_value)?);
    };
    let mut out = edited_map.clone();
    for (key, value) in edited_map {
        let merged = merge_value(base.get(key), value, current.get(key));
        if &merged == value {
            continue;
        }
        let previous = out.insert(key.clone(), merged);
        if serde_json::from_value::<Input>(Value::Object(out.clone())).is_err()
            && let Some(previous) = previous
        {
            out.insert(key.clone(), previous);
        }
    }
    Ok(serde_json::from_value(Value::Object(out))?)
}

/// `current` = `edited` with every field the user left alone taken from
/// `current`. Where both changed a field, the edit wins.
pub fn three_way<Entity>(base: &Entity, edited: &Entity, current: &Entity) -> Result<Entity>
where
    Entity: Serialize + DeserializeOwned,
{
    rebase(base, edited, current)
}

/// Pick one plain field: the edit when it differs from `base`, else `current`.
fn pick<Value: PartialEq + Clone>(base: &Value, edited: &Value, current: &Value) -> Value {
    if edited == base {
        current.clone()
    } else {
        edited.clone()
    }
}

fn rebased<Input, Current>(
    base: Option<&Input>,
    edited: &Input,
    current: Option<&Current>,
) -> Result<Input>
where
    Input: Serialize + DeserializeOwned + Clone,
    Current: Serialize,
{
    match (base, current) {
        (Some(base), Some(current)) => rebase(base, edited, current),
        _ => Ok(edited.clone()),
    }
}

impl UnlockedVault {
    /// With `input.base` set, replace the form's fields by the merge of the
    /// edit onto the current store; without it the input is returned as is.
    /// An entity that is gone is left to the update itself (`Missing`).
    pub fn rebase_store_input(&self, mut input: UpdateStoreInput) -> Result<UpdateStoreInput> {
        let (Some(base), Some(current)) = (input.base.take(), self.find_store(&input.id)) else {
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

    /// The edited transaction merged onto the stored one (same type on all
    /// three sides). Call with the transaction built from the form; `base` is
    /// the stored version the form loaded (`UpdateTransactionInput.base`).
    pub fn rebase_transaction(
        &self,
        id: &str,
        base: Option<&crate::model::transaction::Transaction>,
        edited: crate::model::transaction::Transaction,
    ) -> Result<crate::model::transaction::Transaction> {
        let current = self
            .doc
            .transactions
            .iter()
            .find(|known| known.meta().id == id);
        match (base, current) {
            (Some(base), Some(current)) => three_way(base, &edited, current),
            _ => Ok(edited),
        }
    }

    pub fn rebase_contact_input(&self, input: UpdateContactInput) -> Result<UpdateContactInput> {
        let current = self.doc.contacts.iter().find(|item| item.id == input.id);
        let contact = rebased(input.base.as_ref(), &input.contact, current)?;
        Ok(UpdateContactInput {
            contact,
            base: None,
            ..input
        })
    }

    pub fn rebase_category_input(
        &self,
        mut input: UpdateCategoryInput,
    ) -> Result<UpdateCategoryInput> {
        let (Some(base), Some(current)) = (
            input.base.take(),
            self.doc
                .categories
                .iter()
                .find(|item| item.key == input.key),
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
        Ok(input)
    }

    pub fn rebase_automation_input(
        &self,
        input: UpdateAutomationInput,
    ) -> Result<UpdateAutomationInput> {
        let current = self.automation(&input.id).ok();
        let automation = rebased(input.base.as_ref(), &input.automation, current.as_ref())?;
        Ok(UpdateAutomationInput {
            automation,
            base: None,
            ..input
        })
    }

    pub fn rebase_budget_input(&self, input: UpdateBudgetInput) -> Result<UpdateBudgetInput> {
        let current = self.doc.budgets.iter().find(|item| item.id == input.id);
        let budget = rebased(input.base.as_ref(), &input.budget, current)?;
        Ok(UpdateBudgetInput {
            budget,
            base: None,
            ..input
        })
    }

    pub fn rebase_goal_input(&self, input: UpdateGoalInput) -> Result<UpdateGoalInput> {
        let current = self.doc.goals.iter().find(|item| item.id == input.id);
        let goal = rebased(input.base.as_ref(), &input.goal, current)?;
        Ok(UpdateGoalInput {
            goal,
            base: None,
            ..input
        })
    }

    pub fn rebase_reminder_input(&self, input: UpdateReminderInput) -> Result<UpdateReminderInput> {
        let current = self.doc.reminders.iter().find(|item| item.id == input.id);
        let reminder = rebased(input.base.as_ref(), &input.reminder, current)?;
        Ok(UpdateReminderInput {
            reminder,
            base: None,
            ..input
        })
    }

    pub fn rebase_price_alert_input(
        &self,
        input: UpdatePriceAlertInput,
    ) -> Result<UpdatePriceAlertInput> {
        let current = self
            .doc
            .price_alerts
            .iter()
            .find(|item| item.id == input.id);
        let alert = rebased(input.base.as_ref(), &input.alert, current)?;
        Ok(UpdatePriceAlertInput {
            alert,
            base: None,
            ..input
        })
    }

    pub fn rebase_utility_input(&self, input: UpdateUtilityInput) -> Result<UpdateUtilityInput> {
        let current = self
            .doc
            .utility_items
            .iter()
            .find(|item| item.id == input.id);
        let item = rebased(input.base.as_ref(), &input.item, current)?;
        Ok(UpdateUtilityInput {
            item,
            base: None,
            ..input
        })
    }

    pub fn rebase_envelope_input(&self, input: UpdateEnvelopeInput) -> Result<UpdateEnvelopeInput> {
        let current = self.doc.envelopes.iter().find(|item| item.id == input.id);
        let draft = rebased(input.base.as_ref(), &input.draft, current)?;
        Ok(UpdateEnvelopeInput {
            draft,
            base: None,
            ..input
        })
    }
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::ipc_contract::{GoalInput, UpdateGoalInput};
    use crate::model::contact::ContactKind;
    use crate::model::store::StoreKind;
    use crate::model::store::StoreScope;
    use crate::model::transaction::Transaction;
    use crate::service::ContactDraft;
    use crate::service::test_support::*;

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct Inner {
        provider: String,
        import: String,
    }

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct Sample {
        name: String,
        note: String,
        tags: Vec<String>,
        inner: Inner,
    }

    fn sample() -> Sample {
        Sample {
            name: "a".into(),
            note: "n".into(),
            tags: vec!["x".into()],
            inner: Inner {
                provider: "p".into(),
                import: "i".into(),
            },
        }
    }

    #[test]
    fn three_way_takes_a_changed_field() {
        let base = sample();
        let edited = Sample {
            name: "b".into(),
            ..base.clone()
        };
        let merged = three_way(&base, &edited, &base).unwrap();
        assert_eq!(merged.name, "b");
    }

    #[test]
    fn three_way_keeps_current_for_an_unchanged_field() {
        let base = sample();
        let current = Sample {
            note: "remote".into(),
            tags: vec!["y".into()],
            ..base.clone()
        };
        let edited = Sample {
            name: "b".into(),
            ..base.clone()
        };
        let merged = three_way(&base, &edited, &current).unwrap();
        assert_eq!(merged.name, "b");
        assert_eq!(merged.note, "remote");
        assert_eq!(merged.tags, vec!["y".to_string()]);
    }

    #[test]
    fn three_way_recurses_into_nested_objects() {
        let base = sample();
        let mut current = base.clone();
        current.inner.import = "remote".into();
        let mut edited = base.clone();
        edited.inner.provider = "mine".into();
        let merged = three_way(&base, &edited, &current).unwrap();
        assert_eq!(merged.inner.provider, "mine");
        assert_eq!(merged.inner.import, "remote");
    }

    #[test]
    fn three_way_edit_wins_when_both_sides_changed_a_field() {
        let base = sample();
        let current = Sample {
            name: "remote".into(),
            ..base.clone()
        };
        let edited = Sample {
            name: "mine".into(),
            ..base.clone()
        };
        assert_eq!(three_way(&base, &edited, &current).unwrap().name, "mine");
    }

    fn goal_input(wallet: &str, name: &str, priority: u8) -> GoalInput {
        GoalInput {
            name: name.into(),
            active: true,
            target_amount: amount("1000"),
            currency: "usd".into(),
            starts_at: "2026-01-01T00:00:00Z".into(),
            deadline: Some("2026-12-31T00:00:00Z".into()),
            priority,
            monthly_commitment: None,
            scope: StoreScope {
                store_ids: vec![wallet.to_string()],
                include_children: true,
            },
            scenario_id: None,
            expected_return: None,
        }
    }

    #[test]
    fn patch_update_keeps_remote_field_goal() {
        let (_dir, mut vault) = empty_vault();
        let wallet = store(&mut vault, "Savings", StoreKind::Bank, None);
        let created = vault
            .create_goal(goal_input(&wallet, "Trip", 3).into())
            .unwrap();
        let base = goal_input(&wallet, "Trip", 3);
        // Sync applies a remote edit of the priority while the form is open.
        let mut remote = goal_input(&wallet, "Trip", 5);
        remote.target_amount = amount("1000");
        vault.update_goal(&created.id, remote.into()).unwrap();
        let input = vault
            .rebase_goal_input(UpdateGoalInput {
                id: created.id.clone(),
                goal: goal_input(&wallet, "Trip 2026", 3),
                base: Some(base),
            })
            .unwrap();
        let saved = vault.update_goal(&input.id, input.goal.into()).unwrap();
        assert_eq!(saved.name, "Trip 2026");
        assert_eq!(saved.priority, 5);
    }

    #[test]
    fn update_without_base_overwrites_like_before() {
        let (_dir, mut vault) = empty_vault();
        let wallet = store(&mut vault, "Savings", StoreKind::Bank, None);
        let created = vault
            .create_goal(goal_input(&wallet, "Trip", 3).into())
            .unwrap();
        vault
            .update_goal(&created.id, goal_input(&wallet, "Trip", 5).into())
            .unwrap();
        let input = vault
            .rebase_goal_input(UpdateGoalInput {
                id: created.id.clone(),
                goal: goal_input(&wallet, "Trip 2026", 3),
                base: None,
            })
            .unwrap();
        let saved = vault.update_goal(&input.id, input.goal.into()).unwrap();
        assert_eq!(saved.priority, 3);
    }

    #[test]
    fn patch_update_keeps_remote_field_contact() {
        let (_dir, mut vault) = empty_vault();
        let draft = ContactDraft {
            name: "Anna".into(),
            kind: ContactKind::Person,
            note: "old".into(),
            accounts: Vec::new(),
            group: String::new(),
        };
        let created = vault.create_contact(draft.clone()).unwrap();
        let remote = ContactDraft {
            note: "remote note".into(),
            ..draft.clone()
        };
        vault.update_contact(&created.id, remote).unwrap();
        let input = vault
            .rebase_contact_input(UpdateContactInput {
                id: created.id.clone(),
                contact: ContactDraft {
                    name: "Anna K".into(),
                    ..draft.clone()
                },
                base: Some(draft),
            })
            .unwrap();
        let saved = vault.update_contact(&input.id, input.contact).unwrap();
        assert_eq!(saved.name, "Anna K");
        assert_eq!(saved.note, "remote note");
    }

    #[test]
    fn patch_update_keeps_remote_field_store() {
        let (_dir, mut vault) = empty_vault();
        let id = store(&mut vault, "Bank", StoreKind::Bank, None);
        let base = vault.find_store(&id).unwrap().clone();
        vault
            .doc
            .stores
            .iter_mut()
            .find(|item| item.id == id)
            .unwrap()
            .note = "remote note".into();
        let input = vault
            .rebase_store_input(UpdateStoreInput {
                id: id.clone(),
                name: "Bank 2".into(),
                color: base.color.clone(),
                note: base.note.clone(),
                base: Some(base),
                ..Default::default()
            })
            .unwrap();
        let saved = vault
            .update_store(
                &input.id,
                &input.name,
                &input.color,
                &input.note,
                Default::default(),
            )
            .unwrap();
        assert_eq!(saved.name, "Bank 2");
        assert_eq!(saved.note, "remote note");
    }

    #[test]
    fn patch_update_keeps_remote_field_transaction() {
        let (_dir, mut vault) = empty_vault();
        let wallet = store(&mut vault, "Bank", StoreKind::Bank, None);
        let first = vault
            .add_transaction(income(&wallet, "100", "salary"))
            .unwrap();
        let id = first.meta().id.clone();
        let base = first.clone();
        // Remote edit of the comment.
        let mut remote = base.clone();
        remote.meta_mut().comment = "remote comment".into();
        vault.update_transaction(&id, remote).unwrap();
        // The form changed only the amount.
        let mut edited = base.clone();
        if let Transaction::Topup(tx) = &mut edited {
            tx.amount = amount("150");
        }
        let merged = vault.rebase_transaction(&id, Some(&base), edited).unwrap();
        let saved = vault.update_transaction(&id, merged).unwrap();
        assert_eq!(saved.meta().comment, "remote comment");
        let Transaction::Topup(tx) = saved else {
            panic!("a top-up stays a top-up")
        };
        assert_eq!(tx.amount, amount("150"));
    }
}
