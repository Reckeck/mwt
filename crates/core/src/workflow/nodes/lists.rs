//! Lists: accounts and goals to go through, their size, a field of a record.

use std::collections::BTreeMap;

use crate::Amount;
use crate::model::transaction::AccountRef;
use crate::workflow::exec::Executor;
use crate::workflow::graph::Node;
use crate::workflow::types::WfValue;

use super::logic::{arithmetic, compare};
use super::{Outputs, single};

/// A field of a record (or of an operation); the value itself when `field` is empty.
pub(crate) fn field_of(item: &WfValue, field: &str) -> WfValue {
    if field.is_empty() {
        return item.clone();
    }
    match item {
        WfValue::Record(record) => record.get(field).cloned().unwrap_or_default(),
        WfValue::Transaction(tx) => super::triggers::tx_fields(tx)
            .remove(field)
            .unwrap_or_default(),
        _ => WfValue::Null,
    }
}

fn list_of(value: WfValue) -> Vec<WfValue> {
    match value {
        WfValue::List(items) => items,
        WfValue::Null => Vec::new(),
        other => vec![other],
    }
}

impl Executor<'_> {
    pub(crate) fn eval_list(&mut self, node: &Node) -> Result<Outputs, String> {
        match node.kind.as_str() {
            "list.accounts" => {
                let parent = match self.param(node, "store") {
                    WfValue::Account(AccountRef::Store(id)) => Some(id),
                    _ => None,
                };
                let base = self.vault.base_currency().to_string();
                let items = self
                    .vault
                    .wf_accounts(parent.as_deref())
                    .into_iter()
                    .map(|(id, name, value)| {
                        WfValue::Record(BTreeMap::from([
                            (
                                "account".to_string(),
                                WfValue::Account(AccountRef::Store(id)),
                            ),
                            ("name".to_string(), WfValue::Text(name)),
                            ("value".to_string(), WfValue::money(value, &base)),
                        ]))
                    })
                    .collect();
                Ok(single("items", WfValue::List(items)))
            }
            "list.goals" => {
                let items = self
                    .vault
                    .goal_progress(self.ctx.now, &self.ctx.zone)
                    .iter()
                    .map(|progress| {
                        let mut record: BTreeMap<String, WfValue> =
                            super::data::goal_outputs(progress)
                                .into_iter()
                                .map(|(port, value)| (port.to_string(), value))
                                .collect();
                        record.insert("name".into(), WfValue::Text(progress.goal.name.clone()));
                        record.insert("goal".into(), WfValue::Text(progress.goal.id.clone()));
                        WfValue::Record(record)
                    })
                    .collect();
                Ok(single("items", WfValue::List(items)))
            }
            "list.count" => {
                let count = match self.input(node, "items")? {
                    WfValue::List(items) => items.len(),
                    WfValue::Null => 0,
                    _ => 1,
                };
                Ok(single(
                    "count",
                    WfValue::Number(Amount::new((count as u64).into())),
                ))
            }
            "list.field" => {
                let field = self.text_param(node, "field");
                let record = self.input(node, "record")?;
                let value = if field.trim().is_empty() {
                    WfValue::Null
                } else {
                    field_of(&record, field.trim())
                };
                Ok(single("value", value))
            }
            "list.make" => {
                let mut items = Vec::new();
                for slot in ["a", "b", "c", "d"] {
                    let value = self.input(node, slot)?;
                    if !value.is_null() {
                        items.push(value);
                    }
                }
                Ok(single("items", WfValue::List(items)))
            }
            "list.at" => {
                let items = list_of(self.input(node, "items")?);
                let place = self
                    .number_input(node, "index")?
                    .and_then(|place| place.into_inner().trunc().to_string().parse::<i64>().ok())
                    .unwrap_or(1);
                let index = if place < 0 {
                    items.len().checked_sub(place.unsigned_abs() as usize)
                } else {
                    (place as usize).checked_sub(1)
                };
                let item = index.and_then(|index| items.get(index)).cloned();
                Ok(Outputs::from([
                    ("exists", WfValue::Bool(item.is_some())),
                    ("item", item.unwrap_or_default()),
                ]))
            }
            "list.sum" => {
                let field = self.text_param(node, "field");
                let values: Vec<WfValue> = list_of(self.input(node, "items")?)
                    .iter()
                    .map(|item| field_of(item, field.trim()))
                    .filter(|value| value.as_number().is_some())
                    .collect();
                let Some(first) = values.first().cloned() else {
                    let zero = WfValue::Number(Amount::ZERO);
                    return Ok(Outputs::from([
                        ("total", zero),
                        ("min", WfValue::Null),
                        ("max", WfValue::Null),
                    ]));
                };
                let (mut total, mut least, mut greatest) = (first.clone(), first.clone(), first);
                for value in &values[1..] {
                    total = arithmetic("add", &total, value, 8)?;
                    least = arithmetic("min", &least, value, 8)?;
                    greatest = arithmetic("max", &greatest, value, 8)?;
                }
                Ok(Outputs::from([
                    ("total", total),
                    ("min", least),
                    ("max", greatest),
                ]))
            }
            "list.filter" => {
                let field = self.text_param(node, "field");
                let op = self.text_param(node, "op");
                let expected = self.input(node, "value")?;
                let mut kept = Vec::new();
                for item in list_of(self.input(node, "items")?) {
                    let value = field_of(&item, field.trim());
                    if compare(&op, &value, &expected).unwrap_or(false) {
                        kept.push(item);
                    }
                }
                Ok(Outputs::from([
                    (
                        "count",
                        WfValue::Number(Amount::new((kept.len() as u64).into())),
                    ),
                    ("items", WfValue::List(kept)),
                ]))
            }
            "list.record" => {
                let mut record = BTreeMap::new();
                for (key, slot) in [("key1", "v1"), ("key2", "v2"), ("key3", "v3")] {
                    let name = self.text_param(node, key).trim().to_string();
                    if !name.is_empty() {
                        record.insert(name, self.input(node, slot)?);
                    }
                }
                Ok(single("record", WfValue::Record(record)))
            }
            other => Err(format!("unknown list node {other}")),
        }
    }
}
