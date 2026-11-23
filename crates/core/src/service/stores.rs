//! Shim of the `stores` area: moved to `crate::money::stores` (spec
//! core-strangler, D1). The tree rules live there; the scenarios that create,
//! edit, move, archive and delete wallets live in `usecases::money::stores`.

pub use crate::usecases::money::stores::StoreMoveTargets;
