//! Contacts: the outside parties money moves to and from, with any number of
//! saved accounts each.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::model::chain::Chain;

#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ContactKind {
    #[default]
    Person,
    Company,
    /// A currency exchanger (skycapital and the like): conversions go through it.
    Exchanger,
}

#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum DetailKind {
    Iban,
    Account,
    CryptoAddress,
    Card,
    Phone,
    Url,
    #[default]
    Other,
}

/// One saved account of a contact: an IBAN, a card, a USDT address.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct ContactAccount {
    pub id: String,
    /// Grouping label in the UI («Мой банк», «Биржа»).
    pub group: Option<String>,
    /// «Основной счёт», «USDT TRC-20».
    pub label: String,
    pub kind: DetailKind,
    pub value: String,
    /// Currency code the account holds, lowercase.
    pub currency: Option<String>,
    /// Only for `CryptoAddress`.
    pub network: Option<Chain>,
    /// An account referenced by transactions is deactivated instead of deleted.
    pub active: bool,
}

/// When and why a debt with a contact is due. The amount is never stored: it
/// is what the debt operations with the contact add up to (`DebtEffect`).
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct DebtTerms {
    pub id: String,
    pub contact_id: String,
    /// Ticker of the debt's asset (`USD`, `USDT`); one terms entry per asset.
    pub asset: String,
    /// `YYYY-MM-DD`.
    pub due_at: Option<String>,
    pub note: String,
    pub created_at: String,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct Contact {
    pub id: String,
    pub name: String,
    pub kind: ContactKind,
    pub accounts: Vec<ContactAccount>,
    pub note: String,
    pub archived: bool,
    pub created_at: String,
    /// Group the directory lists it under («Семья», «P2P»); empty = by kind.
    #[serde(default)]
    pub group: String,
}
