//! Shim of the `preview` area: moved to `crate::money::preview` (spec
//! core-strangler, D1). A preview is a query over the books: it writes nothing.

pub use crate::money::preview::{PositionChange, PreviewWarning, TxPreview};
