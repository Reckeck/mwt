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

use mwt_base::error::Result;
use mwt_model::VaultDoc;

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
pub fn pick<Value: PartialEq + Clone>(base: &Value, edited: &Value, current: &Value) -> Value {
    if edited == base {
        current.clone()
    } else {
        edited.clone()
    }
}

pub fn rebased<Input, Current>(
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

/// The edited transaction merged onto the stored one (same type on all
/// three sides). Call with the transaction built from the form; `base` is
/// the stored version the form loaded (`UpdateTransactionInput.base`).
pub fn rebase_transaction(
    doc: &VaultDoc,
    id: &str,
    base: Option<&mwt_model::Transaction>,
    edited: mwt_model::Transaction,
) -> Result<mwt_model::Transaction> {
    let current = doc.transactions.iter().find(|known| known.meta().id == id);
    match (base, current) {
        (Some(base), Some(current)) => three_way(base, &edited, current),
        _ => Ok(edited),
    }
}
