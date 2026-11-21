//! Domain model stored inside the encrypted vault (`VaultDoc`).
//! All monetary/quantity values use [`crate::amount::Amount`].

pub mod attachment;
pub mod category;
pub mod chain;
pub mod coins;
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
/// Tests of model items that name a higher layer; they move to the facade crate with T7.
#[cfg(test)]
#[path = "../model_tests/mod.rs"]
mod model_tests;
pub mod notify;
pub mod planning;
pub mod record;
pub mod registry;
pub mod run_key;
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
