//! Shim of the `notify_snapshot` area: moved to `mwt_reports::notify_snapshot` (spec core-strangler, D1).

pub use mwt_reports::notify_snapshot::{SNAPSHOT_DAYS, ScheduledNotice, SnapshotAlert};
