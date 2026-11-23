//! Shim of the `search` area: moved to `crate::money::search` (spec core-strangler, D1).

pub use crate::money::search::{PINNED_CAP, PinnedView, SEARCH_TX_CAP, SearchEntry};
