//! Shim of the `losses` area: moved to `crate::household::losses` (spec core-strangler, D1).

pub use crate::household::losses::{FlowAmount, FlowLink, FxRow, LossBucket, LossRow};
pub use mwt_reports::overview::LossReport;
