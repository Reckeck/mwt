//! Shim of the `portfolio` area: moved to `crate::money::portfolio` (spec
//! core-strangler, D1). The views and the holdings fold live there; the scenario
//! that stores the holdings lives in `usecases::money::portfolio`.

pub use crate::money::portfolio::{
    AssetBalance, AssetStat, HistoryPoint, NamedValue, PnlLeg, PortfolioSummary, RealizedPnl,
    StoreChildRow, StoreCrumb, StoreOverview, StorePosition, StoreTreeNode, StoreValue,
};
