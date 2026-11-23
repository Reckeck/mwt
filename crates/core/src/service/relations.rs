//! Shim of the `relations` area: moved to `crate::household::relations` (spec core-strangler, D1).

pub use crate::household::relations::{
    EntityCard, EntityKind, EntityRef, RELATION_ITEMS_CAP, RelationGroup, RelationItem,
    RelationKind,
};
