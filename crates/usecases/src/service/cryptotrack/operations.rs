//! Shim of the `cryptotrack/operations` area: the rules and the reads moved to
//! `crate::cryptotrack::service::operations`, the scenario to
//! `crate::usecases::cryptotrack::operations` (spec core-strangler, D1).

#[cfg(test)]
mod tests;
