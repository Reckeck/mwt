//! IPC types of the per-network endpoints: RPC, Blockscout, the explorer,
//! their presets and the connection test.
//!
//! Owner: spec E17, as a subsection of Settings (decision P5 of 2026-11-11).
//! Every request goes out of Rust and not out of the webview, so the CSP
//! stays as it is; only `https` is accepted, apart from `localhost` for the
//! user's own node.
//!
//! Commands: `crates/app/src/commands/cryptotrack/network.rs`.

use serde::{Deserialize, Serialize};

use crate::model::chain::Chain;

/// Which endpoint of a network a command is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum EndpointKind {
    /// The JSON-RPC node (balances).
    Rpc,
    /// The Blockscout instance (history and held tokens).
    Blockscout,
}

/// One endpoint as the screen shows it: what is used now and what the presets
/// are.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct EndpointView {
    /// The address in use: the user's own, else the first preset. Empty when
    /// the network has no preset and the user set none (a Blockscout instance
    /// for BNB Smart Chain).
    pub value: String,
    /// The user's own address is in use, not a preset.
    pub custom: bool,
    /// The built-in addresses, the first one is the default.
    pub presets: Vec<String>,
    /// Only the host (and port) of `value`: what the provider is, without a
    /// path that may carry a provider key. Safe to show in a list.
    pub host: String,
}

/// One network's endpoints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct NetworkView {
    pub chain: Chain,
    pub label: String,
    /// EIP-155 chain id; a node answering with another one is the wrong network.
    #[ts(type = "number")]
    pub chain_id: u64,
    pub rpc: EndpointView,
    pub blockscout: EndpointView,
    pub explorer: EndpointView,
    /// A Blockscout key is stored on this device. The key itself never comes
    /// back to the screen.
    pub blockscout_key_set: bool,
}

/// The endpoints of every network.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct NetworkSettings {
    /// They can be changed here: protection level 3 on a real vault. Below it
    /// the presets are shown and used, read-only.
    pub editable: bool,
    pub networks: Vec<NetworkView>,
    /// When the tracking consent was given on this device (RFC 3339); filled
    /// by the command from the app settings, not by the vault.
    #[ts(optional)]
    pub consent_at: Option<String>,
    /// The name of this device if one is set; the screen shows it next to the date.
    #[ts(optional)]
    pub consent_device: Option<String>,
}

/// Save one network. An empty address goes back to the preset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct SetNetworkSettingsInput {
    pub chain: Chain,
    pub rpc: String,
    pub blockscout: String,
    pub explorer: String,
    /// The optional Blockscout key: `None` leaves the stored key as it is, an
    /// empty text removes it, any other text replaces it. Empty means keyless
    /// reads, the module works the same, only slower.
    #[ts(optional)]
    pub blockscout_key: Option<String>,
}

/// Ask one endpoint whether it answers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct TestNetworkInput {
    pub chain: Chain,
    pub kind: EndpointKind,
    /// The address typed in the field, not yet saved; `None` tests the one in use.
    #[ts(optional)]
    pub url: Option<String>,
    /// A key typed in the field, not yet saved. `None` or empty uses the stored
    /// key only when the address under test is the instance that key belongs
    /// to; any other address is tested without a key.
    #[ts(optional)]
    pub blockscout_key: Option<String>,
}

/// How a connection test ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum NetworkTestOutcome {
    Ok,
    /// The node answers but for another chain id.
    WrongNetwork,
    /// No answer, a refusal or a timeout.
    Unreachable,
    /// It answered with something that is not what this endpoint speaks.
    BadAnswer,
}

/// The result of a connection test. Carries no text from the provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct NetworkTestResult {
    pub chain: Chain,
    pub kind: EndpointKind,
    pub outcome: NetworkTestOutcome,
    #[ts(type = "number")]
    pub latency_ms: u64,
    /// The latest block, from a node that answered.
    #[ts(optional, type = "number")]
    pub block: Option<u64>,
}
