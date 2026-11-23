//! Shim of the `losses` area: moved to `crate::household::losses` (spec core-strangler, D1).

pub use crate::household::losses::{
    FlowAmount, FlowLink, FlowSummary, FxRow, FxTable, LossBucket, LossRow, TxLoss,
};
pub use mwt_reports::overview::LossReport;
