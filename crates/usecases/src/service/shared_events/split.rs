//! Shim of the split of an event: moved to `crate::household::shared_events::split`
//! (spec core-strangler, D1).

// Nothing under `service/` names the split any more; the shim stays for the area map.
#[allow(unused_imports)]
pub use crate::household::shared_events::split::*;
