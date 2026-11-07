//! Vault protection commands. None is implemented yet: each answers
//! `Error::Unsupported` and nothing in the frontend calls them.

pub mod autolock;
pub mod biometric;
pub mod keys;
pub mod raise;
pub mod state;
