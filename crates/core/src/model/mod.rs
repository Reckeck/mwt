//! Domain model stored inside the encrypted vault (`VaultDoc`).
//! All monetary/quantity values use [`crate::amount::Amount`].

pub mod attachment;
pub mod category;
pub mod chain;
pub mod contact;
pub mod datasource;
pub mod deposit;
pub mod enrichment;
pub mod entity_ref;
#[cfg(test)]
pub mod fixtures;
pub mod inflation;
pub mod notify;
pub mod planning;
pub mod shared_event;
pub mod store;
pub mod transaction;
pub mod vault;

pub use chain::{Chain, ChainInfo};
pub use store::StoreKind;
pub use transaction::Transaction;
pub use vault::VaultDoc;
