//! Lists: accounts and goals to go through, their size, a field of a record.

use std::collections::BTreeMap;

use crate::Amount;
use crate::model::transaction::AccountRef;
use crate::workflow::exec::Executor;
use crate::workflow::graph::Node;
use crate::workflow::types::WfValue;

use super::{Outputs, single};

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
                let value = match self.input(node, "record")? {
                    WfValue::Record(record) => {
                        record.get(field.trim()).cloned().unwrap_or_default()
                    }
                    WfValue::Transaction(tx) => super::triggers::tx_fields(&tx)
                        .remove(field.trim())
                        .unwrap_or_default(),
                    _ => WfValue::Null,
                };
                Ok(single("value", value))
            }
            other => Err(format!("unknown list node {other}")),
        }
    }
}
