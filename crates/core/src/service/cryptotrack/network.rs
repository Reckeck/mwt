//! Shim of the `cryptotrack/network` area: the rules and the reads moved to
//! `crate::cryptotrack::service::network`, the scenario to
//! `crate::usecases::cryptotrack::network` (spec core-strangler, D1).

pub use crate::cryptotrack::service::network::NetworkRequest;
pub use crate::usecases::cryptotrack::network::{NetworkProbe, Probe, test_endpoint};

#[cfg(test)]
mod tests;
