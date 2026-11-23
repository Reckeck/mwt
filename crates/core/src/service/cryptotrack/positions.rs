//! Shim of the `cryptotrack/positions` area: the rules and the reads moved to
//! `crate::cryptotrack::service::positions`, the scenario to
//! `crate::usecases::cryptotrack::positions` (spec core-strangler, D1).

#[cfg(test)]
#[path = "positions_tests.rs"]
mod tests;
