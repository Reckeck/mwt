//! Shim of the `cryptotrack/wipe` area: the rules moved to `crate::cryptotrack::service::wipe`,
//! the scenarios to `crate::usecases::cryptotrack::wipe` (spec core-strangler, D1).
//!
//! Three crate-private methods are vault-side and stay here: they read what is
//! private to `service` (the session password, this device's sync clock and the
//! device key bookkeeping), which the scenarios outside `service` cannot reach.

use std::path::Path;

use crate::error::Result;
use crate::ipc_contract::cryptotrack::wipe::{CryptoWipePreview, CryptoWipeReport};
use crate::model::vault::VaultDoc;
use crate::storage::VaultFile;

// What the wipe tests reach through `super::*`.
#[cfg(test)]
use crate::cryptotrack::service::wipe::WIPE_CONFIRM;
#[cfg(test)]
use crate::error::Error;
#[cfg(test)]
use crate::ipc_contract::security::SecurityLevel;
#[cfg(test)]
use crate::vault_io::backups;

use super::super::UnlockedVault;
use super::super::exchange_keys::DeviceKeys;
use super::super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// What a wipe would remove: the records per collection, the wallets that
        /// would be unlinked and archived, what the capital would lose and which
        /// backup the user has already.
        pub fn crypto_wipe_preview(&self) -> CryptoWipePreview
            => crate::usecases::cryptotrack::wipe::crypto_wipe_preview;
        /// Wipe the module. See the module docs for the order; `confirm` must be
        /// [`WIPE_CONFIRM`].
        ///
        /// Nothing is removed while the backup cannot be taken and read back, and
        /// a vault that holds nothing of the module takes no backup at all: there
        /// is nothing to risk and nothing to mark.
        pub fn crypto_wipe_module(&mut self, confirm: &str) -> Result<CryptoWipeReport>
            => crate::usecases::cryptotrack::wipe::crypto_wipe_module;
    }
    // forward:end

    // Called by the wipe tests, which hand the backup step in; not in the baseline
    // (crate-private), so outside the regenerated block.
    forward! {
        /// [`Self::crypto_wipe_module`] with the backup step handed in, so a test
        /// can watch what a failed backup leaves behind: the answer must be
        /// "everything, untouched".
        #[cfg(test)]
        pub(crate) fn wipe_inner(&mut self, confirm: &str, backup: impl FnOnce(&mut Self) -> Result<String>) -> Result<CryptoWipeReport>
            => crate::usecases::cryptotrack::wipe::wipe_inner;
    }

    // Vault-side: they read what is private to `service`, so the scenarios,
    // which live outside it, ask for them here. Each is one line, with no rule
    // in it.

    /// The backup copy at `path`, opened again with this session's password.
    pub(crate) fn read_back_backup(&self, path: &Path) -> Result<VaultDoc> {
        VaultFile::open(path, &self.password).map(|(doc, _)| doc)
    }

    /// This device's id in the sync clock.
    pub(crate) fn sync_device_id(&self) -> &str {
        self.clock.device()
    }

    /// The session forgets what the device secrets file held.
    pub(crate) fn forget_device_keys(&mut self) {
        self.device_keys = DeviceKeys::default();
    }
}

#[cfg(test)]
#[path = "wipe_tests.rs"]
mod tests;
