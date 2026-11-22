//! The request inputs of the shared events commands: what the user types in the dialogs and what the
//! plan functions take. They are part of the IPC contract (`trade_core::ipc_contract` re-exports them).

use mwt_base::Amount;
use serde::{Deserialize, Serialize};

/// Everything a shared event is made of that the user types in its dialog.
/// `participants` is read on create only; later people come and go through
/// their own commands.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SharedEventDraft {
    pub title: String,
    #[serde(default)]
    pub note: String,
    /// `YYYY-MM-DD`; a day, or the first day of a range.
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(type = "LocalDate | null"))]
    pub starts_on: Option<String>,
    /// `YYYY-MM-DD`; the last day of a range.
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(type = "LocalDate | null"))]
    pub ends_on: Option<String>,
    /// Empty: the base currency.
    #[serde(default)]
    pub currency: String,
    /// One of 0.01, 1, 10, 50, 100; empty: 1.
    #[serde(default)]
    pub rounding_step: Option<Amount>,
    #[serde(default)]
    pub category_id: Option<String>,
    /// `me` (or empty) or a participant id.
    #[serde(default)]
    pub organizer: String,
    pub i_participate: bool,
    #[serde(default)]
    pub participants: Vec<SharedParticipantDraft>,
}

impl Default for SharedEventDraft {
    fn default() -> Self {
        Self {
            title: String::new(),
            note: String::new(),
            starts_on: None,
            ends_on: None,
            currency: String::new(),
            rounding_step: None,
            category_id: None,
            organizer: String::new(),
            i_participate: true,
            participants: Vec::new(),
        }
    }
}

/// A person to add: a contact (`contact_id`) or a guest (a name, maybe details).
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SharedParticipantDraft {
    /// The client may pick the id (to name this person as the organizer in the
    /// same call); empty: the vault makes one.
    #[serde(default)]
    pub id: Option<String>,
    /// A guest's name; a contact's own name is used when empty.
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub contact_id: Option<String>,
    #[serde(default)]
    pub details: Option<mwt_model::GuestDetails>,
}

/// An item of the list: planned, or added as it was bought.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SharedItemDraft {
    pub title: String,
    /// "pcs", "kg", "l", "pack" or any text; empty: "pcs".
    #[serde(default)]
    pub unit: String,
    #[serde(default)]
    pub category_id: Option<String>,
    #[serde(default)]
    pub plan: Option<mwt_model::ItemPlan>,
    #[serde(default)]
    pub cancelled: bool,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SetSharedItemSplitInput {
    pub event_id: String,
    pub item_id: String,
    pub split: mwt_model::ItemSplit,
}

/// A real purchase. Without `item_id` a new item is made from `new_item_title`.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SharedPurchaseDraft {
    #[serde(default)]
    pub item_id: Option<String>,
    #[serde(default)]
    pub new_item_title: Option<String>,
    #[serde(default)]
    pub new_item_unit: Option<String>,
    /// `me` (or empty) or a participant id.
    #[serde(default)]
    pub buyer: String,
    pub qty: Amount,
    /// Per unit or in total, see `price_mode`.
    pub amount: Amount,
    /// Empty: the event currency.
    #[serde(default)]
    pub currency: String,
    #[serde(default)]
    pub price_mode: mwt_model::PriceMode,
    /// Typed in by hand; empty: taken from the rate cache at `at`.
    #[serde(default)]
    pub rate_to_event: Option<Amount>,
    /// RFC3339 or `YYYY-MM-DD`; empty: now.
    #[serde(default)]
    pub at: Option<String>,
    /// The wallet the payment comes from; required when I am the buyer.
    #[serde(default)]
    pub store_id: Option<String>,
}

/// Take a payment that is already in the vault as a purchase of the event.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AttachPaymentInput {
    pub event_id: String,
    pub tx_id: String,
    #[serde(default)]
    pub item_id: Option<String>,
    #[serde(default)]
    pub new_item_title: Option<String>,
    /// Units bought; empty: 1.
    #[serde(default)]
    pub qty: Option<Amount>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SetShareOverrideInput {
    pub event_id: String,
    pub participant_id: String,
    /// `None` takes the hand-set total back.
    #[serde(default)]
    pub total: Option<Amount>,
}

/// Money (or a set-off) between a participant and the organizer.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SettleSharedInput {
    pub event_id: String,
    /// `me` or a participant id; never the organizer.
    pub participant_id: String,
    pub direction: mwt_model::SettleDirection,
    pub amount: Amount,
    /// My wallet the money went to or came from. Empty: a set-off without an
    /// operation (the only way to settle with a guest).
    #[serde(default)]
    pub store_id: Option<String>,
    /// RFC3339 or `YYYY-MM-DD`; empty: now.
    #[serde(default)]
    pub at: Option<String>,
}

/// Write off, or put on the others, what a participant has not returned.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SharedRemainderInput {
    pub event_id: String,
    pub participant_id: String,
    /// Empty: the whole rest.
    #[serde(default)]
    pub amount: Option<Amount>,
    /// `reassign` only: who takes it; empty: everybody else.
    #[serde(default)]
    pub to: Vec<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SetSharedDueInput {
    pub event_id: String,
    pub participant_id: String,
    /// `YYYY-MM-DD`; empty clears the term.
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(type = "LocalDate | null"))]
    pub due_at: Option<String>,
}
