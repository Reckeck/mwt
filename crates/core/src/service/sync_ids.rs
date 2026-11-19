//! Shim of the `sync_ids` area: the deterministic record ids moved to `crate::sync::sync_ids` (spec
//! core-strangler, D1). The old path stays for the scenarios that mint ids until they import the new one.

// forward:begin
#[cfg(test)]
pub(crate) use crate::sync::sync_ids::sync_safe_id;
pub(crate) use crate::sync::sync_ids::{
    asset_class_id, debt_terms_id, goal_envelope_id, guest_contact_id, payment_link_id, reading_id,
    valuation_id,
};
// forward:end
