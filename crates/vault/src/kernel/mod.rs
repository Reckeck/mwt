//! The kernel of the core: the one place for how a change of the open vault
//! is committed. It grows by the strangler rule; the first step only pins the
//! write path as it is, so the later moves can be checked against it.

pub mod collection;
pub mod crypto_data;
pub mod events;
pub mod persistence;
/// The record trait and the record-kind tables: the home of the old path `kernel::record`.
pub mod record {
    pub use mwt_model::{Record, RecordKind, level3_collections, record_kinds};
}
pub mod session;
pub mod tx;
