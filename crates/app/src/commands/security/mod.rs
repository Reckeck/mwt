//! Vault protection commands. The state, the raise to level 3 and the
//! password strength work (H5A); biometrics answer `Error::Unsupported`
//! until their stage.

pub mod autolock;
pub mod biometric;
pub mod keys;
pub mod raise;
pub mod state;
