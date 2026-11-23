//! Shim of the `worth` area: moved to `crate::money::worth` (spec core-strangler, D1).
//! The assembly of `NetWorth` and the scenarios that write hand-set prices live
//! in `usecases::money::capital`.

pub use crate::money::worth::{FlowClass, LiabilityShare, NetWorth};
