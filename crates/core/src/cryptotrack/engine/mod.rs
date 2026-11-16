//! The engine of the crypto module (stage 2, spec E5): raw facts of a network
//! in, holdings, cost basis, profit and the differences against the network
//! out.
//!
//! Everything here is pure. It reads what it is given and returns what it
//! made; it opens no socket and writes no document. The one place that
//! touches the vault is [`crate::service`]'s `cryptotrack::recompute`, which
//! is also the only place that knows what "now" is. That is what makes the
//! engine testable at all: the whole chain, from an explorer's answer to a
//! profit figure, is a function.
//!
//! The way through it:
//!
//! 1. [`normalize`] turns a source's raw facts into operations
//!    ([`crate::model::cryptotrack::CryptoTx`]), one per transaction per
//!    followed address, with deterministic ids ([`ids`]).
//! 2. [`overrides`] lays the user's corrections over those operations. The
//!    operations themselves are never edited: a refetch rewrites them, and
//!    the user's work must outlive that.
//! 3. [`basis`] folds the corrected operations into holdings, average cost,
//!    realized profit, gas and income — and names every gap it had to leave.
//! 4. [`reconcile`] compares the result with what the node says the address
//!    holds and writes the difference down as an operation of its own.
//! 5. [`snapshot`] writes what the module was worth, because balances are
//!    computed and a price of a day long past cannot be fetched again.
//!
//! [`math`] is underneath all of it: the arithmetic that cannot panic on a
//! junk token and rounds in exactly one place.
//!
//! **Nothing derived is stored.** Holdings, averages and profit are folded
//! again on every read. What is stored is only what cannot be recomputed: the
//! operations, the user's corrections, the adjustments against the network,
//! and the snapshots.
//!
//! **Determinism is a feature, not an accident.** Every id is a function of
//! the facts, every map is a `BTreeMap`, and the fold sorts its input itself.
//! Two devices that have read the same address hold the same bytes in
//! `crypto_events`, which is what lets the sync treat an operation as one
//! record instead of two.

pub mod basis;
#[cfg(test)]
mod basis_tests;
pub mod ids;
pub mod math;
pub mod normalize;
pub mod overrides;
pub mod reconcile;
pub mod snapshot;

pub use basis::{AssetState, Doubt, Inputs, Ledger, Prices, Quote, fold};
pub use ids::Adjustment;
pub use normalize::{Arrival, Followed, Normalized, Unread, combine, normalize};
pub use overrides::EventView;
pub use reconcile::{Reading, Reconciled, adjust};
pub use snapshot::Snapshots;
