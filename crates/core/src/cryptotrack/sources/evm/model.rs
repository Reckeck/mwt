//! What the EVM source hands over: raw, unmerged facts of the network.
//!
//! Nothing here is a [`crate::model::cryptotrack::CryptoTx`]: turning facts
//! into operations (a swap is two transfers of one transaction) is the
//! engine's job (spec E5). The source reports what the explorer said, checked
//! and with exact quantities.

use serde::{Deserialize, Serialize};

use super::uint::Uint256;
use crate::Amount;
use crate::error::Result;

/// What moved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RawAsset {
    /// The network's gas coin.
    Native { decimals: u8 },
    Token {
        /// Lowercase `0x` address.
        contract: String,
        symbol: String,
        name: String,
        decimals: u8,
        /// The explorer flags it as a scam or low reputation: show it
        /// hidden by default.
        suspect: bool,
    },
}

impl RawAsset {
    pub fn decimals(&self) -> u8 {
        match self {
            RawAsset::Native { decimals } | RawAsset::Token { decimals, .. } => *decimals,
        }
    }
}

/// Which explorer list a fact came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RawKind {
    /// A transaction itself: native value, method, fee.
    Transaction,
    /// An ERC-20 `Transfer` log.
    TokenTransfer,
    /// A native value moved by a contract call inside a transaction.
    InternalTransfer,
}

/// One fact of the history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawEvent {
    /// `evm:{chainId}:{hash}:{logIndex}` for a token transfer,
    /// `evm:{chainId}:{hash}:tx` for the transaction itself and
    /// `evm:{chainId}:{hash}:i{index}` for an internal transfer. Built only
    /// from validated parts, so the same fact always has the same id.
    pub id: String,
    pub chain_id: u64,
    /// Lowercase `0x` + 64 hex.
    pub hash: String,
    pub kind: RawKind,
    pub block: u64,
    /// RFC3339 UTC, whole seconds.
    pub time: String,
    pub from: String,
    pub to: String,
    pub asset: RawAsset,
    /// Raw integer units (wei, token base units), never rounded.
    pub value: Uint256,
    /// The 4-byte selector the explorer reports, `0x…`.
    pub method: Option<String>,
    /// `false` for a reverted transaction: its value did not move.
    pub succeeded: bool,
    /// What the transaction paid in wei, when the explorer knows it; only on
    /// [`RawKind::Transaction`].
    pub fee: Option<Uint256>,
    /// The fields above as the explorer reported them, compact JSON, for a
    /// later re-read.
    pub payload: String,
}

impl RawEvent {
    /// The quantity in whole units, exactly or an error (never rounded).
    pub fn amount(&self) -> Result<Amount> {
        self.value.scaled(self.asset.decimals())
    }
}

/// Where a balance was read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BalanceSource {
    /// The node, at the `latest` block: authoritative.
    Node,
    /// The explorer's indexed holder list: may lag behind the node.
    Explorer,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawBalance {
    pub asset: RawAsset,
    pub value: Uint256,
    pub source: BalanceSource,
}

impl RawBalance {
    pub fn amount(&self) -> Result<Amount> {
        self.value.scaled(self.asset.decimals())
    }
}

/// An item the source refused to trust, with the reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rejected {
    /// The id or hash it would have had, when that much was readable.
    pub subject: Option<String>,
    pub reason: String,
}

/// One step of a read. Pure: nothing here touches the document.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourceBatch {
    pub raw_events: Vec<RawEvent>,
    pub balances: Vec<RawBalance>,
    /// Where to continue; `None` when everything has been read. Keep it
    /// together with the events of the same batch.
    pub next_cursor: Option<String>,
    pub rejected: Vec<Rejected>,
}
