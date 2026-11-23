//! Shim of the `sync_ids` area: the deterministic record ids moved to `crate::sync::sync_ids` (spec
//! core-strangler, D1). The old path stays for the scenarios that mint ids until they import the new one.

#[cfg(test)]
pub(crate) use crate::sync::sync_ids::sync_safe_id;
#[cfg(test)]
pub(crate) use crate::sync::sync_ids::{debt_terms_id, payment_link_id, reading_id};
