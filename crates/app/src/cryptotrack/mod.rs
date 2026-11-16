//! The application side of the crypto module: the background work that reads
//! the followed addresses (stage 2, spec E0 opens it, spec E6 fills it).
//!
//! What is stored lives in `trade_core::model::cryptotrack`, the engine's
//! traits in `trade_core::cryptotrack`, and the commands in
//! `crate::commands::cryptotrack`.

pub mod jobs;
