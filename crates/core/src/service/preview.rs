//! Shim of the `preview` area: moved to `crate::money::preview` (spec
//! core-strangler, D1). A preview is a query over the books: it writes nothing.

pub use crate::money::preview::{PositionChange, PreviewWarning, TxPreview};

use crate::model::transaction::Transaction;

use super::UnlockedVault;
use super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// Preview a draft. `replaces` is the id of the transaction being edited:
        /// its effect is taken out first, so the preview shows the net change.
        /// Reads the books only: nothing is written.
        pub fn preview_transaction(&self, mut tx: Transaction, replaces: Option<&str>) -> TxPreview
            => crate::money::preview::preview_transaction @books;
    }
    // forward:end
}
