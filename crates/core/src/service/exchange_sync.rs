//! Shim of the `exchange` area (sync): the rules moved to
//! `crate::cryptotrack::exchange_sync`, the scenarios to
//! `crate::usecases::cryptotrack::exchange_sync` (spec core-strangler, D1).

pub use crate::cryptotrack::exchange_sync::{IMPORT_ACCOUNTS, ImportPreview};

use super::UnlockedVault;

impl UnlockedVault {
    /// Discarded ids are remembered, so the next sync does not stage them again.
    #[cfg(test)]
    pub(crate) fn remember_discarded(&mut self, ids: impl IntoIterator<Item = String>) {
        crate::cryptotrack::exchange_sync::remember_discarded(&mut self.doc, ids);
    }
}

#[cfg(test)]
#[path = "exchange_sync_tests.rs"]
mod tests;
