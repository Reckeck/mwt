//! Shim of the `cryptotrack/network` area: the rules and the reads moved to
//! `crate::cryptotrack::service::network`, the scenario to
//! `crate::usecases::cryptotrack::network` (spec core-strangler, D1).

pub use crate::cryptotrack::service::network::NetworkRequest;
pub use crate::usecases::cryptotrack::network::{NetworkProbe, Probe, test_endpoint};

use crate::cryptotrack::tracking::ExplorerSettings;
use crate::error::Result;
use crate::ipc_contract::cryptotrack::network::{
    NetworkSettings, SetNetworkSettingsInput, TestNetworkInput,
};
use crate::model::chain::Chain;
use crate::vault_io::device_secrets::BlockscoutKey;

use super::super::UnlockedVault;
use super::super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// The endpoints of every network and whether they can be changed here.
        pub fn crypto_network_settings(&self) -> NetworkSettings
            => crate::usecases::cryptotrack::network::crypto_network_settings @books;
        /// Save one network. Everything is checked before anything is written.
        pub fn crypto_set_network_settings(&mut self, input: &SetNetworkSettingsInput) -> Result<()>
            => crate::usecases::cryptotrack::network::crypto_set_network_settings;
        /// What the reader of `chain` uses for one request. Never fails: a vault
        /// below level 3, a damaged file or a bad value mean the presets and no key.
        pub fn crypto_network_for_request(&self, chain: Chain) -> NetworkRequest
            => crate::cryptotrack::service::network::crypto_network_for_request @books;
        /// The user's own explorers, for the links of an operation. Bad values are
        /// dropped here and checked again when a link is built.
        pub fn crypto_explorer_settings(&self) -> ExplorerSettings
            => crate::cryptotrack::service::network::crypto_explorer_settings @books;
        /// What a connection test sends: the address typed or in use, and the key
        /// typed or stored.
        pub fn crypto_test_target(&self, input: &TestNetworkInput) -> Result<(String, Option<BlockscoutKey>)>
            => crate::usecases::cryptotrack::network::crypto_test_target @books;
    }
    // forward:end
}

#[cfg(test)]
mod tests;
