//! Shared events («посиделки»): a living list of items bought together, split
//! between people, with the debts it leaves worked out from the list itself.
//! Nothing here is derived data: shares, balances and statuses are computed by
//! `service::shared_events` and never stored.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::Amount;
use crate::model::contact::ContactAccount;

/// The id that stands for the vault owner wherever a participant id is
/// expected (buyer, organizer, share owner). Participant ids are UUIDs, so it
/// never clashes with one.
pub const ME: &str = "me";

/// Rounding steps an event may use.
pub const ROUNDING_STEPS: [&str; 5] = ["0.01", "1", "10", "50", "100"];

/// What a guest (a participant without a contact) is reachable by.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct GuestDetails {
    #[serde(default)]
    pub phone: String,
    #[serde(default)]
    pub messenger: String,
    /// Same shape as a contact's saved accounts.
    #[serde(default)]
    pub requisites: Vec<ContactAccount>,
    #[serde(default)]
    pub note: String,
}

impl GuestDetails {
    pub fn is_empty(&self) -> bool {
        self.phone.trim().is_empty()
            && self.messenger.trim().is_empty()
            && self.requisites.is_empty()
            && self.note.trim().is_empty()
    }
}

/// A person in the event: a contact of the directory, or a guest (no
/// `contact_id`). The owner is not listed; they are [`ME`].
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct Participant {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub contact_id: Option<String>,
    /// Only a guest keeps details here.
    #[serde(default)]
    pub details: Option<GuestDetails>,
}

#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum PriceMode {
    #[default]
    PerUnit,
    Total,
}

/// What was meant to be bought. The price is in the event currency.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct ItemPlan {
    pub qty: Amount,
    pub price: Amount,
    pub price_mode: PriceMode,
}

impl ItemPlan {
    /// The planned cost of the item, in the event currency.
    pub fn total(&self) -> Amount {
        match self.price_mode {
            PriceMode::PerUnit => self.qty * self.price,
            PriceMode::Total => self.price,
        }
    }
}

/// Who shares an item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ItemSplit {
    /// Equally between the listed people; an empty list means everyone.
    Equal { among: Vec<String> },
    /// In proportion to the units each person took.
    ByUnits { units: BTreeMap<String, Amount> },
}

impl Default for ItemSplit {
    fn default() -> Self {
        Self::Equal { among: Vec::new() }
    }
}

/// One real purchase of an item.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct Purchase {
    pub id: String,
    /// [`ME`] or a participant id.
    pub buyer: String,
    pub qty: Amount,
    /// As entered: per unit or in total, see `price_mode`.
    pub amount: Amount,
    /// Lower-case currency code the amount was paid in.
    pub currency: String,
    pub price_mode: PriceMode,
    /// Event-currency units per unit of `currency` at the purchase date. Taken
    /// once from the rate cache (or typed in) and never recalculated.
    pub rate_to_event: Amount,
    pub rate_manual: bool,
    /// My payment in the vault, when the buyer is me.
    #[serde(default)]
    pub tx_id: Option<String>,
    /// The event created that payment (so it also deletes it); false when an
    /// existing payment was only linked.
    #[serde(default)]
    pub tx_owned: bool,
    /// RFC3339.
    pub at: String,
}

impl Purchase {
    /// What was paid, in `currency`.
    pub fn total(&self) -> Amount {
        match self.price_mode {
            PriceMode::PerUnit => self.qty * self.amount,
            PriceMode::Total => self.amount,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct EventItem {
    pub id: String,
    pub title: String,
    /// «шт», «кг», «л», «уп» or free text.
    pub unit: String,
    #[serde(default)]
    pub category_id: Option<String>,
    #[serde(default)]
    pub plan: Option<ItemPlan>,
    #[serde(default)]
    pub purchases: Vec<Purchase>,
    #[serde(default)]
    pub cancelled: bool,
    #[serde(default)]
    pub split: ItemSplit,
}

/// A person's total fixed by hand; the rest is spread over the others.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct ShareOverride {
    pub participant_id: String,
    pub total: Amount,
}

#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SettlementKind {
    /// Money moved between the participant and the organizer (a debt
    /// transfer in the vault when it touches my wallet).
    #[default]
    Transfer,
    /// Set off without money moving.
    Offset,
    /// The organizer wrote the rest off: it became their own expense.
    WriteOff,
    /// The participant's rest was put on the other participants.
    Reassign,
}

/// Which way the money of a settlement went, relative to the organizer.
#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SettleDirection {
    /// The participant gave the organizer money (reduces what they owe).
    #[default]
    ToOrganizer,
    /// The organizer gave the participant money.
    FromOrganizer,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct Settlement {
    pub id: String,
    pub kind: SettlementKind,
    /// [`ME`] or a participant id; never the organizer.
    pub participant_id: String,
    #[serde(default)]
    pub direction: SettleDirection,
    pub amount: Amount,
    /// RFC3339.
    pub at: String,
    /// The debt transfer in the vault, if the settlement moved money of mine.
    #[serde(default)]
    pub tx_id: Option<String>,
    /// `Reassign`: who took the amount.
    #[serde(default)]
    pub to: Vec<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct SharedEvent {
    pub id: String,
    pub title: String,
    pub created_at: String,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub archived: bool,
    /// `YYYY-MM-DD`: the day, or the first day of a range.
    #[serde(default)]
    pub starts_on: Option<String>,
    /// `YYYY-MM-DD`: the last day of a range; empty or equal to `starts_on`
    /// is a single day.
    #[serde(default)]
    pub ends_on: Option<String>,
    /// Lower-case currency code the event is settled in.
    pub currency: String,
    /// One of [`ROUNDING_STEPS`].
    pub rounding_step: Amount,
    #[serde(default)]
    pub category_id: Option<String>,
    /// [`ME`] or a participant id; debts are settled through them.
    pub organizer: String,
    pub i_participate: bool,
    #[serde(default)]
    pub participants: Vec<Participant>,
    #[serde(default)]
    pub items: Vec<EventItem>,
    #[serde(default)]
    pub settlements: Vec<Settlement>,
    #[serde(default)]
    pub adjustments: Vec<ShareOverride>,
}

/// The link from a vault payment or debt transfer back to the event that made
/// (or adopted) it.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct SharedLink {
    pub event_id: String,
    #[serde(default)]
    pub purchase_id: Option<String>,
    #[serde(default)]
    pub settlement_id: Option<String>,
}

/// Where an event stands; worked out, never stored.
#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SharedEventStatus {
    /// Nothing bought yet.
    #[default]
    Planning,
    /// Bought, nobody has settled yet.
    InProgress,
    /// Settling has begun and someone still owes.
    Settling,
    /// Settled: everything returned or written off.
    Closed,
}
