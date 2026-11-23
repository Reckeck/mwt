//! Shim of the `peer_sync/repair_report` area: moved to `crate::sync::repair::repair_report` (spec core-strangler, D1).

#[cfg(test)]
pub(crate) use crate::sync::repair::repair_report::RepairReport;
