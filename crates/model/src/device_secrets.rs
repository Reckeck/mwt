//! The data of the device secrets file (what it holds, not how it is sealed or
//! stored): the exchange keys, the per-network endpoints and the Blockscout key.
//! The file itself (`vault_io::device_secrets`) reads and writes them; they live
//! here so the read context `Books` can hand them out without naming the vault.

use serde::{Deserialize, Serialize};

use mwt_base::{Error, Result};

use crate::chain::Chain;
use crate::vault::ExchangeKeys;

/// One exchange key kept on this device.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceKey {
    pub keys: ExchangeKeys,
    /// The read-only check passed; `false` = it could not run (no network)
    /// and the next import runs it again.
    #[serde(default)]
    pub verified: bool,
}

/// The optional Blockscout key of one network (decision R8, spec E17). A
/// device secret like the Bybit keys: never in the document, never synced,
/// never shown again after it is saved. `Debug` hides the value, it is wiped
/// from memory on drop, and it is sent only in an `x-api-key` header.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BlockscoutKey(String);

impl BlockscoutKey {
    /// A key a header can carry: printable ASCII without spaces, at most 256
    /// characters. The error never echoes the value.
    pub fn parse(raw: &str) -> Result<Self> {
        let text = raw.trim();
        if text.is_empty() || text.len() > 256 || !text.chars().all(|ch| ch.is_ascii_graphic()) {
            return Err(Error::InvalidInput(
                "the Blockscout key must be 1-256 visible ASCII characters without spaces".into(),
            ));
        }
        Ok(Self(text.to_owned()))
    }

    /// The value, for one request. Do not log it.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for BlockscoutKey {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str("BlockscoutKey(<hidden>)")
    }
}

impl Drop for BlockscoutKey {
    fn drop(&mut self) {
        zeroize::Zeroize::zeroize(&mut self.0);
    }
}

/// What the user set for one network on this device: their own node, their
/// Blockscout instance and explorer, and the optional Blockscout key. An
/// endpoint left `None` is the preset of `cryptotrack::chains`. They live in
/// the secrets file because a node URL often carries a provider key in its
/// path, and a localhost node is meaningful on this device only.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkEndpoints {
    pub chain: Chain,
    #[serde(default)]
    pub rpc: Option<String>,
    #[serde(default)]
    pub blockscout: Option<String>,
    #[serde(default)]
    pub explorer: Option<String>,
    #[serde(default)]
    pub blockscout_key: Option<BlockscoutKey>,
}

impl NetworkEndpoints {
    /// Nothing set: the entry carries no information and is dropped.
    pub fn is_blank(&self) -> bool {
        self.rpc.is_none()
            && self.blockscout.is_none()
            && self.explorer.is_none()
            && self.blockscout_key.is_none()
    }
}

/// Everything in the secrets file.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceSecrets {
    #[serde(default)]
    pub bybit: Vec<DeviceKey>,
    /// Per-network endpoints and the Blockscout key (spec E17).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub crypto_networks: Vec<NetworkEndpoints>,
}

impl DeviceSecrets {
    pub fn is_empty(&self) -> bool {
        self.bybit.is_empty() && self.crypto_networks.is_empty()
    }

    pub fn network_for(&self, chain: Chain) -> Option<&NetworkEndpoints> {
        self.crypto_networks
            .iter()
            .find(|entry| entry.chain == chain)
    }

    pub fn bybit_for(&self, store_id: &str) -> Option<&DeviceKey> {
        self.bybit
            .iter()
            .find(|entry| entry.keys.store_id == store_id)
    }
}
