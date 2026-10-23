//! The address of any entity the app can open a card of (plan J1), and what
//! the sidebar keeps pinned (plan J2).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A kind of thing the app can open a card of.
#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    #[default]
    Store,
    Tx,
    Category,
    Contact,
    Goal,
    Budget,
    Automation,
    Workflow,
    Reminder,
    Alert,
    /// `id` is `AssetRef::key()`.
    Asset,
    /// A saved calculator scenario.
    Scenario,
}

/// The address of one entity: what the `?open=kind:id` link carries.
#[derive(
    Debug, Default, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS, JsonSchema,
)]
pub struct EntityRef {
    pub kind: EntityKind,
    pub id: String,
}

impl EntityRef {
    pub fn new(kind: EntityKind, id: impl Into<String>) -> Self {
        Self {
            kind,
            id: id.into(),
        }
    }
}

/// One line of the sidebar's «Pinned» block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Pinned {
    /// Any entity: opens its card (or its page).
    Entity { target: EntityRef },
    /// A saved filter of the operations list: `query` is the URL query string.
    Filter {
        id: String,
        name: String,
        query: String,
    },
}
