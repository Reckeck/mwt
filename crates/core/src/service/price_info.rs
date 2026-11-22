//! Shim of the `price_info` area: moved to `crate::money::price_info` (spec core-strangler, D1).

pub use crate::money::price_info::{PriceInfo, PriceOrigin};
