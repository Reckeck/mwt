//! Domain model stored inside the encrypted vault (`VaultDoc`).
//! All monetary/quantity values use [`crate::amount::Amount`].

pub mod chain;
pub mod store;
pub mod transaction;
pub mod vault;

pub use chain::{Chain, ChainInfo};
pub use store::StoreKind;
pub use transaction::Transaction;
pub use vault::VaultDoc;
