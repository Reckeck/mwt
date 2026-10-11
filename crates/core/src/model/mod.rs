//! Domain model stored inside the encrypted vault (`VaultDoc`).
//! All monetary/quantity values use [`crate::amount::Amount`].

pub mod category;
pub mod chain;
pub mod contact;
pub mod deposit;
pub mod enrichment;
#[cfg(test)]
pub mod fixtures;
pub mod planning;
pub mod store;
pub mod transaction;
pub mod vault;

pub use chain::{Chain, ChainInfo};
pub use store::StoreKind;
pub use transaction::Transaction;
pub use vault::VaultDoc;
