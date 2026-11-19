//! Shim of the `preview` area: moved to `crate::money::preview` (spec
//! core-strangler, D1). A preview is a query over the books: it writes nothing.

#[cfg(test)]
use crate::money::preview::{ConvertPreview, SalePreview};
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
        #[cfg(test)]
        pub(crate) fn sale_preview(&self, tx: &Transaction) -> Option<SalePreview>
            => crate::money::preview::sale_preview @books;
        #[cfg(test)]
        pub(crate) fn convert_preview(&self, tx: &Transaction) -> Option<ConvertPreview>
            => crate::money::preview::convert_preview @books;
    }
    // forward:end
}
