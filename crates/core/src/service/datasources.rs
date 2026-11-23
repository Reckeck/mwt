//! Shim of the `datasources` area: the rules moved to `crate::planning::datasources`, the
//! scenarios to `crate::usecases::planning::datasources` (spec core-strangler, D1).

pub use crate::planning::datasources::{DataSourceDraft, DataSourceView, SourceUse, is_due};
