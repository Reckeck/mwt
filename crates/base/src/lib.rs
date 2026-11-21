#![warn(unreachable_pub)]
//! `mwt-base`: the leaf of the workspace. `Amount`, `Error`, limits, time and
//! calendar helpers, the calculators, `Secret` and the UI route names. It depends
//! on no other workspace crate; `trade-core` re-exports every module under its
//! old path.

pub mod amount;
pub mod calc;
pub mod calendar;
pub mod error;
pub mod limits;
pub mod routes;
pub mod secret;
pub mod time;
pub mod utilities_formula;

pub use amount::Amount;
pub use error::{Error, Result};
