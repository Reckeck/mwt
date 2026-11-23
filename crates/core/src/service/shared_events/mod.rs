//! Shim of the shared events area (event, people, items, split, settling, purchases,
//! ledger, view, text, analytics): moved to `crate::household::shared_events` and
//! `crate::usecases::household::shared_events` (spec core-strangler, D1).

mod analytics;
mod ledger;
mod purchases;
mod text;
mod view;

mod settle;
pub(crate) mod split;
pub use view::{
    EventTotals, ItemShareView, ItemView, MyPosition, PersonView, PlannedPayment, PurchaseView,
    SettlementView,
};
