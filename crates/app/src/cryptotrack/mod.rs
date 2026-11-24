//! The application side of the crypto module: the background work that reads
//! the followed addresses (stage 2, spec E0 opens it, spec E6 fills it).
//!
//! What is stored lives in `mwt_model::cryptotrack`, the engine's
//! traits in `mwt_cryptotrack`, and the commands in
//! `crate::commands::cryptotrack`.

pub mod jobs;
