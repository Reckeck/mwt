//! What each node does. `run_flow_node` runs a node on the flow and says where
//! to go next; `eval_data_node` works out every output of a pure data node.
//! Fields are read here from the node's JSON parameters.

mod actions;
mod data;
mod lists;
mod logic;
pub mod triggers;

use std::collections::BTreeMap;

use serde_json::Value;

use crate::Amount;
use crate::model::store::AssetRef;
use crate::model::transaction::AccountRef;

use super::catalog::{NodeDef, ParamKind, node_def};
use super::exec::{Executor, Fault, Step};
use super::graph::Node;
use super::types::WfValue;

pub(crate) type Outputs = BTreeMap<&'static str, WfValue>;

/// A number from a field: a JSON number or a string with one.
pub(crate) fn json_number(value: &Value) -> Option<Amount> {
    match value {
        Value::Number(number) => Amount::parse(&number.to_string()).ok(),
        Value::String(text) => Amount::parse(text.trim().replace(',', ".").as_str()).ok(),
        _ => None,
    }
}

pub(crate) fn json_text(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// An account from a field: `{ "store": id }`, a contact, or a bare store id.
pub(crate) fn json_account(value: &Value) -> Option<AccountRef> {
    match value {
        Value::String(id) if !id.trim().is_empty() => {
            Some(AccountRef::Store(id.trim().to_string()))
        }
        Value::Object(_) => serde_json::from_value(value.clone()).ok(),
        _ => None,
    }
}

impl Executor<'_> {
    /// A field's value as the node has it, or its default.
    pub(crate) fn raw_param(&self, node: &Node, name: &str) -> Value {
        if let Some(value) = node.params.get(name)
            && !value.is_null()
        {
            return value.clone();
        }
        node_def(&node.kind)
            .and_then(|def| def.param(name))
            .map(|param| param.default.clone())
            .unwrap_or(Value::Null)
    }

    pub(crate) fn text_param(&self, node: &Node, name: &str) -> String {
        json_text(&self.raw_param(node, name))
    }

    /// An asset from a field: the stored asset, or a symbol the vault knows.
    pub(crate) fn json_asset(&self, value: &Value) -> Option<AssetRef> {
        match value {
            Value::Object(_) => serde_json::from_value(value.clone()).ok(),
            Value::String(symbol) if !symbol.trim().is_empty() => Some(self.vault.wf_asset(symbol)),
            _ => None,
        }
    }

    /// A field's value as a typed value, by the kind of its editor.
    pub(crate) fn param(&self, node: &Node, name: &str) -> WfValue {
        let Some(param) = node_def(&node.kind).and_then(|def| def.param(name)) else {
            return WfValue::Null;
        };
        let raw = self.raw_param(node, name);
        if raw.is_null() {
            return WfValue::Null;
        }
        match &param.kind {
            ParamKind::Number | ParamKind::Day | ParamKind::Weekday => {
                json_number(&raw).map_or(WfValue::Null, WfValue::Number)
            }
            ParamKind::Bool => WfValue::Bool(raw.as_bool().unwrap_or(false)),
            ParamKind::Store | ParamKind::Account => {
                json_account(&raw).map_or(WfValue::Null, WfValue::Account)
            }
            ParamKind::Asset => self.json_asset(&raw).map_or(WfValue::Null, WfValue::Asset),
            ParamKind::Currency => {
                let code = json_text(&raw).trim().to_lowercase();
                if code.is_empty() {
                    WfValue::Null
                } else {
                    WfValue::Currency(code)
                }
            }
            ParamKind::Category => {
                let key = json_text(&raw);
                if key.trim().is_empty() {
                    WfValue::Null
                } else {
                    WfValue::Category(key)
                }
            }
            ParamKind::Date => WfValue::Date(json_text(&raw)),
            ParamKind::Text
            | ParamKind::LongText
            | ParamKind::Time
            | ParamKind::Budget
            | ParamKind::Goal
            | ParamKind::Workflow
            | ParamKind::Select { .. } => WfValue::Text(json_text(&raw)),
            ParamKind::Targets => WfValue::Null,
        }
    }

    /// A number input (an amount counts as its number).
    pub(crate) fn number_input(
        &mut self,
        node: &Node,
        name: &str,
    ) -> Result<Option<Amount>, String> {
        let value = self.input(node, name)?;
        if value.is_null() {
            return Ok(None);
        }
        value
            .as_number()
            .map(Some)
            .ok_or_else(|| format!("«{name}» is not a number: {}", value.as_text()))
    }

    pub(crate) fn account_input(
        &mut self,
        node: &Node,
        name: &str,
    ) -> Result<Option<AccountRef>, String> {
        Ok(match self.input(node, name)? {
            WfValue::Account(account) => Some(account),
            WfValue::Text(text) if !text.trim().is_empty() => {
                Some(AccountRef::Store(text.trim().to_string()))
            }
            WfValue::Record(record) => match record.get("account") {
                Some(WfValue::Account(account)) => Some(account.clone()),
                _ => None,
            },
            _ => None,
        })
    }

    pub(crate) fn asset_input(
        &mut self,
        node: &Node,
        name: &str,
    ) -> Result<Option<AssetRef>, String> {
        Ok(match self.input(node, name)? {
            WfValue::Asset(asset) => Some(asset),
            WfValue::Currency(code) | WfValue::Text(code) if !code.trim().is_empty() => {
                Some(self.vault.wf_asset(&code))
            }
            WfValue::Money { currency, .. } => Some(self.vault.wf_asset(&currency)),
            _ => None,
        })
    }

    pub(crate) fn currency_input(
        &mut self,
        node: &Node,
        name: &str,
    ) -> Result<Option<String>, String> {
        let value = self.input(node, name)?;
        Ok(match value {
            WfValue::Text(code) => Some(code.trim().to_lowercase()).filter(|code| !code.is_empty()),
            other => other.currency(),
        })
    }

    pub(crate) fn run_flow_node(&mut self, node: &Node, def: &NodeDef) -> Result<Step, Fault> {
        match def.group {
            super::catalog::NodeGroup::Trigger => Ok(Step::next(
                "out",
                format!("«{}»", def.title.ru),
                format!("«{}»", def.title.en),
            )),
            _ => match node.kind.as_str() {
                "logic.branch" | "logic.guard" | "logic.throttle" => self.run_logic(node, def),
                "var.set" | "var.add" => self.run_var(node, def),
                _ => self.run_action(node, def),
            },
        }
    }

    pub(crate) fn eval_data_node(&mut self, node: &Node, def: &NodeDef) -> Result<Outputs, String> {
        match def.group {
            super::catalog::NodeGroup::Data => self.eval_data(node),
            super::catalog::NodeGroup::Logic => self.eval_logic(node),
            super::catalog::NodeGroup::Variable => self.eval_var(node),
            super::catalog::NodeGroup::List => self.eval_list(node),
            _ => Ok(Outputs::new()),
        }
    }
}

/// One output.
pub(crate) fn single(port: &'static str, value: WfValue) -> Outputs {
    Outputs::from([(port, value)])
}
