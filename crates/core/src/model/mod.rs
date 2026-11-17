//! Domain model stored inside the encrypted vault (`VaultDoc`).
//! All monetary/quantity values use [`crate::amount::Amount`].

pub mod attachment;
pub mod category;
pub mod chain;
pub mod contact;
pub mod cryptotrack;
pub mod datasource;
pub mod deposit;
pub mod enrichment;
pub mod entity_ref;
#[cfg(any(test, feature = "testkit"))]
pub mod fixtures;
pub mod ids;
pub mod inflation;
pub(crate) mod iso_date;
pub mod notify;
pub mod planning;
pub mod registry;
pub mod shared_event;
pub mod store;
pub mod sync_state;
pub mod transaction;
pub mod vault;
pub mod workflow;

pub use chain::{Chain, ChainInfo};
pub use store::StoreKind;
pub use transaction::Transaction;
pub use vault::VaultDoc;
