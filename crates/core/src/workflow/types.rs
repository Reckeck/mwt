//! Port types and the values that travel along data edges.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::Amount;
use crate::model::store::AssetRef;
use crate::model::transaction::{AccountRef, Transaction};

/// What a port carries. Each has its colour in the editor; a wire joins only
/// compatible ports (see [`PortType::accepts`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum PortType {
    /// Execution order, not a value.
    Flow,
    /// Takes any value.
    Any,
    Number,
    /// An amount with its currency or asset code.
    Money,
    Asset,
    Currency,
    Account,
    Category,
    Bool,
    Text,
    Date,
    Duration,
    Transaction,
    List,
    Record,
}

impl PortType {
    /// Whether an output of type `from` may feed an input of this type.
    pub fn accepts(self, from: PortType) -> bool {
        use PortType::*;
        if self == Flow || from == Flow {
            return self == from;
        }
        self == from
            || self == Any
            || from == Any
            // An amount is a number; text takes anything for a message.
            || (self == Number && from == Money)
            || (self == Currency && from == Asset)
            || self == Text
    }
}

/// A value on a data edge. Money keeps its currency so different currencies
/// are never added up by accident.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum WfValue {
    #[default]
    Null,
    Number(Amount),
    Money {
        amount: Amount,
        /// Lowercase code: `rub`, `usdt`.
        currency: String,
    },
    Asset(AssetRef),
    Currency(String),
    Account(AccountRef),
    Category(String),
    Bool(bool),
    Text(String),
    /// RFC3339.
    Date(String),
    /// Seconds.
    Duration(i64),
    Transaction(Box<Transaction>),
    List(Vec<WfValue>),
    Record(BTreeMap<String, WfValue>),
}

impl WfValue {
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    /// The number in a number or an amount.
    pub fn as_number(&self) -> Option<Amount> {
        match self {
            Self::Number(value) | Self::Money { amount: value, .. } => Some(*value),
            Self::Text(text) => Amount::parse(text.trim()).ok(),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            Self::Number(value) => Some(!value.is_zero()),
            Self::Text(text) => Some(!text.is_empty()),
            Self::Null => Some(false),
            _ => None,
        }
    }

    pub fn as_text(&self) -> String {
        match self {
            Self::Null => String::new(),
            Self::Number(value) => value.to_string(),
            Self::Money { amount, currency } => format!("{amount} {}", currency.to_uppercase()),
            Self::Asset(asset) => asset.symbol.clone(),
            Self::Currency(code) => code.to_uppercase(),
            Self::Account(account) => match account {
                AccountRef::Store(id) => id.clone(),
                AccountRef::Contact { contact_id, .. } => contact_id.clone(),
                AccountRef::Raw(text) => text.clone(),
            },
            Self::Category(key) => key.clone(),
            Self::Bool(value) => value.to_string(),
            Self::Text(text) | Self::Date(text) => text.clone(),
            Self::Duration(secs) => format!("{secs}s"),
            Self::Transaction(tx) => tx.meta().comment.clone(),
            Self::List(items) => items
                .iter()
                .map(Self::as_text)
                .collect::<Vec<_>>()
                .join(", "),
            Self::Record(map) => map
                .iter()
                .map(|(key, value)| format!("{key}: {}", value.as_text()))
                .collect::<Vec<_>>()
                .join(", "),
        }
    }

    /// The currency of money (or the code of a currency / an asset).
    pub fn currency(&self) -> Option<String> {
        match self {
            Self::Money { currency, .. } | Self::Currency(currency) => {
                Some(currency.to_lowercase())
            }
            Self::Asset(asset) => Some(asset.symbol.to_lowercase()),
            _ => None,
        }
    }

    pub fn money(amount: Amount, currency: &str) -> Self {
        Self::Money {
            amount,
            currency: currency.trim().to_lowercase(),
        }
    }

    /// The port type this value fits.
    pub fn port_type(&self) -> PortType {
        match self {
            Self::Null => PortType::Any,
            Self::Number(_) => PortType::Number,
            Self::Money { .. } => PortType::Money,
            Self::Asset(_) => PortType::Asset,
            Self::Currency(_) => PortType::Currency,
            Self::Account(_) => PortType::Account,
            Self::Category(_) => PortType::Category,
            Self::Bool(_) => PortType::Bool,
            Self::Text(_) => PortType::Text,
            Self::Date(_) => PortType::Date,
            Self::Duration(_) => PortType::Duration,
            Self::Transaction(_) => PortType::Transaction,
            Self::List(_) => PortType::List,
            Self::Record(_) => PortType::Record,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ports_join_by_type() {
        assert!(PortType::Number.accepts(PortType::Money));
        assert!(!PortType::Money.accepts(PortType::Number));
        assert!(PortType::Text.accepts(PortType::Money));
        assert!(!PortType::Flow.accepts(PortType::Any));
        assert!(PortType::Any.accepts(PortType::Account));
        assert!(PortType::Currency.accepts(PortType::Asset));
    }

    #[test]
    fn a_value_round_trips_in_json() {
        let value = WfValue::money(Amount::parse("10.5").unwrap(), "RUB");
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(serde_json::from_str::<WfValue>(&json).unwrap(), value);
        assert_eq!(value.as_text(), "10.5 RUB");
    }
}
