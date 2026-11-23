//! Shim of the `valuation` area: moved to `crate::money::valuation` (spec core-strangler, D1).
//! The scenarios that write valuations and the revalue rule live in
//! `usecases::money::valuation`.

pub(crate) use crate::money::valuation::ValuationCurve;
pub use crate::money::valuation::{
    PropertyPurchase, PropertyValue, ValuationBaseKind, ValuationInput,
};
