//! Device-to-device sync of one vault (docs/sync.md).
//!
//! Everything here is pure: no network, no files. The vault side lives in
//! `service::sync`, the transport in the app crate.

pub mod manifest;
pub mod merge;
pub mod plan;
pub mod protocol;
pub mod registry;
pub mod session;
pub mod stamp;
pub mod state;
pub mod track;
pub mod view;

pub use plan::{
    ChangeItem, ChangeKind, CollectionChanges, OverwrittenField, SyncPlan, SyncSummary,
};
pub use stamp::{Clock, Stamp};
pub use state::{BatchEntry, RecordStamps, RecordState, SyncBatch, SyncState, TrustedPeer};
