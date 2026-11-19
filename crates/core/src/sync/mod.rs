//! Device-to-device sync of one vault (docs/sync.md).
//!
//! Everything here is pure: no network, no files. The vault side lives in
//! `service::sync`, the transport in the app crate.

pub mod alias;
pub(crate) mod apply;
#[cfg(test)]
mod apply_tests;
pub mod crypto_gate;
pub mod derived;
pub mod fields;
pub mod flatten;
pub mod history;
pub mod journal;
pub mod manifest;
pub mod manifest_view;
pub mod merge;
pub mod plan;
pub mod protocol;
pub mod registry;
pub mod session;
pub mod stamp;
pub mod state;
pub mod track;
pub mod transfer;
pub mod trust;
pub mod view;

pub use plan::{
    ChangeItem, ChangeKind, CollectionChanges, FieldChange, OverwrittenField, SyncPlan, SyncSummary,
};
pub use stamp::{Clock, Stamp};
pub use state::{BatchEntry, RecordStamps, RecordState, SyncBatch, SyncState, TrustedPeer};

#[cfg(test)]
mod flatten_tests;
pub mod peer_sync;
#[cfg(test)]
mod peer_sync_tests;
pub mod repair;
pub mod sync_ids;
#[cfg(test)]
mod sync_ids_tests;
pub mod sync_journal;
#[cfg(test)]
mod sync_journal_tests;
#[cfg(test)]
mod sync_pins_tests;
