//! The crypto module's service layer.
//!
//! E0 only opens the module: the vault's crypto collections, their sync rules
//! and the rule that a document holding them cannot be protected below level
//! 3. Every operation on them lives in a file of its own here, one per domain
//! (accounts, operations, tokens, positions, …), so two specs never edit the
//! same file.

#[cfg(test)]
mod tests;
#[cfg(test)]
mod volume;

mod accounts;
mod apply;
mod legacy_bybit;
mod links;
pub mod network;
mod operations;
pub mod positions;
mod recompute;
mod relations;
mod tokens;
mod totals;
mod transition;
pub mod wipe;
