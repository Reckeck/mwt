//! Shim of the `cryptotrack/wipe` area: the rules moved to `crate::cryptotrack::service::wipe`,
//! the scenarios to `crate::usecases::cryptotrack::wipe` (spec core-strangler, D1).
//!
//! Three crate-private methods are vault-side and stay here: they read what is
//! private to `service` (the session password, this device's sync clock and the
//! device key bookkeeping), which the scenarios outside `service` cannot reach.

use std::path::Path;

use crate::error::Result;
use crate::model::vault::VaultDoc;
use crate::storage::VaultFile;

use super::super::Session;
use super::super::exchange_keys::DeviceKeys;

impl Session {
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
