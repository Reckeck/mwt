//! Shim of the `cryptotrack/totals` area: the rules and queries moved to
//! `crate::cryptotrack::service::totals`, the scenarios to
//! `crate::usecases::cryptotrack::recompute` (spec core-strangler, D1).

// The positions tests name it through the shim.
#[cfg(test)]
pub use crate::cryptotrack::service::totals::CryptoLeftOut;

#[cfg(test)]
mod tests;
