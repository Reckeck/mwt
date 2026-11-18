//! The `usecases` module group (spec core-strangler, D1).

pub mod cryptotrack;
pub mod household;
pub mod money;
pub mod planning;
pub mod react;
pub mod reports;
pub mod sync;
pub mod workflow;

#[cfg(test)]
mod react_tests;
