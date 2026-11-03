//! Device-to-device sync of one vault (docs/sync.md).
//!
//! Everything here is pure: no network, no files. The vault side lives in
//! `service::sync`, the transport in the app crate.

pub mod alias;
pub mod fields;
pub mod flatten;
pub mod journal;
pub mod manifest;
pub mod merge;
pub mod plan;
pub mod protocol;
pub mod registry;
pub mod session;
pub mod stamp;
pub mod state;
pub mod track;
pub mod transfer;
pub mod view;

pub use plan::{
    ChangeItem, ChangeKind, CollectionChanges, FieldChange, OverwrittenField, SyncPlan, SyncSummary,
};
pub use stamp::{Clock, Stamp};
pub use state::{BatchEntry, RecordStamps, RecordState, SyncBatch, SyncState, TrustedPeer};

#[cfg(test)]
mod flatten_tests;
