//! What the crypto module stores inside the vault (stage 2, spec E0).
//!
//! The module keeps its data in sibling top-level fields of
//! [`crate::VaultDoc`] (`crypto_accounts`, `crypto_events`, …) rather than in
//! one nested object: sync stamps, tombstones and batches are per record, so
//! editing one operation must not send the whole module (decision P1 of
//! 2026-11-10).
//!
//! Nothing derived is stored: balances, average prices and totals are
//! computed from the events every time.
//!
//! An event ([`CryptoTx`]) is built with every field named — there is no
//! `Default` — so a new source cannot forget [`CryptoTx::tracking`]:
//!
//! ```
//! # use trade_core::model::chain::Chain;
//! # use trade_core::model::cryptotrack::{CryptoTx, CryptoTxKind, TrackingRef};
//! let event = CryptoTx {
//!     id: "ev-1".into(),
//!     chain: Chain::Ethereum,
//!     account_id: "acc-1".into(),
//!     time: "2026-11-10T10:00:00Z".into(),
//!     block: Some(21_000_000),
//!     from: "0xaaaa".into(),
//!     to: "0xbbbb".into(),
//!     legs: Vec::new(),
//!     gas: None,
//!     protocol: None,
//!     method: None,
//!     kind: CryptoTxKind::Transfer,
//!     raw: None,
//!     tracking: TrackingRef::ChainTx { chain: Chain::Ethereum, hash: "0xdead".into() },
//! };
//! assert!(event.validate().is_ok());
//! ```
//!
//! The same event without `tracking` does not compile:
//!
//! ```compile_fail
//! # use trade_core::model::chain::Chain;
//! # use trade_core::model::cryptotrack::{CryptoTx, CryptoTxKind};
//! let event = CryptoTx {
//!     id: "ev-1".into(),
//!     chain: Chain::Ethereum,
//!     account_id: "acc-1".into(),
//!     time: "2026-11-10T10:00:00Z".into(),
//!     block: None,
//!     from: "0xaaaa".into(),
//!     to: "0xbbbb".into(),
//!     legs: Vec::new(),
//!     gas: None,
//!     protocol: None,
//!     method: None,
//!     kind: CryptoTxKind::Transfer,
//!     raw: None,
//! };
//! ```
//!
//! Every record here is read from files already on disk, so a later spec adds
//! fields with `#[serde(default)]` and never renames or re-types one
//! (`AGENTS.md`, rule 5).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::Amount;
use crate::model::chain::Chain;

/// Networks whose addresses are hex and case-insensitive, so the same address
/// typed in two cases is one address. Everything else (Bitcoin, Solana, Tron,
/// Cardano…) is case-sensitive and kept exactly as it was typed.
const EVM_CHAINS: &[Chain] = &[
    Chain::Ethereum,
    Chain::Bsc,
    Chain::Polygon,
    Chain::Arbitrum,
    Chain::Optimism,
    Chain::Avalanche,
    Chain::Base,
];

/// Whether `chain` addresses hex, case-insensitively (EVM).
pub fn is_evm(chain: Chain) -> bool {
    EVM_CHAINS.contains(&chain)
}

/// The one place an address or a contract is normalized: trimmed, and
/// lowercased on an EVM network.
///
/// `AssetRef::key()` stores a contract as it was given, which splits the
/// price series of one token typed in two cases; the crypto module normalizes
/// on creation instead, where it is still possible.
pub fn normalize_address(chain: Chain, address: &str) -> String {
    let address = address.trim();
    if is_evm(chain) {
        address.to_lowercase()
    } else {
        address.to_string()
    }
}

/// The network part of a stored key, stable across releases: the name serde
/// writes (`chain_slug_matches_serde` checks it).
pub fn chain_slug(chain: Chain) -> String {
    format!("{chain:?}").to_lowercase()
}

/// Identity of a token: its network and its contract address there.
///
/// The symbol is deliberately not part of it — two tokens share a symbol, and
/// one token renames itself — so a price series and a holding stay with the
/// contract.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct TokenId {
    pub chain: Chain,
    /// Normalized by [`normalize_address`] when built with [`TokenId::new`].
    pub contract: String,
}

impl TokenId {
    /// A token id with its contract normalized for the network.
    pub fn new(chain: Chain, contract: &str) -> Self {
        Self {
            chain,
            contract: normalize_address(chain, contract),
        }
    }

    /// `<chain>|<contract>`: the record key of `crypto_tokens` and the price
    /// key of `crypto_prices`.
    pub fn key(&self) -> String {
        format!("{}|{}", chain_slug(self.chain), self.contract)
    }
}

/// What an operation or a position holds: the network's own coin, or a token
/// on it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CryptoAsset {
    /// The network's gas coin (ETH on Ethereum, SOL on Solana).
    Native { chain: Chain },
    /// A token on a network.
    Token(TokenId),
}

impl CryptoAsset {
    pub fn chain(&self) -> Chain {
        match self {
            CryptoAsset::Native { chain } => *chain,
            CryptoAsset::Token(token) => token.chain,
        }
    }

    /// Price key: `<chain>|native` for a coin, [`TokenId::key`] for a token.
    pub fn key(&self) -> String {
        match self {
            CryptoAsset::Native { chain } => format!("{}|native", chain_slug(*chain)),
            CryptoAsset::Token(token) => token.key(),
        }
    }
}

/// An address the module follows. Every account is shown by a
/// [`crate::model::store::Store`] whose `crypto_account_id` points back here
/// (decision P5): a connected address by a wallet of kind `Crypto`, an
/// exchange read through its key by one of kind `Exchange`. The module owns
/// the reading, the wallet owns the presentation, and the value reaches the
/// capital through `doc.holdings` like any other wallet's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct CryptoAccount {
    pub id: String,
    pub name: String,
    pub chain: Chain,
    /// Normalized by [`normalize_address`] when built with
    /// [`CryptoAccount::new`].
    pub address: String,
    /// The source that reads it (`etherscan`, `bybit`…); `None` until one is
    /// chosen.
    #[serde(default)]
    pub source: Option<String>,
    /// Watched but not counted in the module's value.
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub note: String,
    pub created_at: String,
}

impl CryptoAccount {
    /// An account with its address normalized for the network.
    pub fn new(id: &str, name: &str, chain: Chain, address: &str, created_at: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            chain,
            address: normalize_address(chain, address),
            source: None,
            archived: false,
            note: String::new(),
            created_at: created_at.to_string(),
        }
    }
}

/// A token the module has met, with what it takes to show and price it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct CryptoToken {
    /// Record key, with [`CryptoToken::contract`]: see
    /// `sync::registry::COLLECTIONS`.
    pub chain: Chain,
    /// Normalized by [`normalize_address`] when built with
    /// [`CryptoToken::new`].
    pub contract: String,
    pub symbol: String,
    #[serde(default)]
    pub name: String,
    pub decimals: u8,
    /// CoinGecko coin id, when the token is known there.
    #[serde(default)]
    pub coingecko_id: Option<String>,
    /// The user hid it: a scam airdrop is not worth showing or pricing.
    #[serde(default)]
    pub hidden: bool,
    /// What the old purchases of this token cost, in USD per unit: bought
    /// before the module followed the address, so no event carries it.
    #[serde(default)]
    #[ts(optional)]
    pub basis_usd: Option<Amount>,
    /// The user's decision on the token. A refetch rewrites a token's text
    /// and never this, so a decision outlives it; with none, the built-in
    /// list or the quarantine decides (`cryptotrack::tokens::status`).
    #[serde(default)]
    #[ts(optional)]
    pub verdict: Option<TokenVerdict>,
    /// When the verdict was decided (RFC3339 UTC); `None` with no verdict, and
    /// for a verdict an older release stored. Set and cleared together with
    /// `verdict`, so the settings list can say "hidden on <date>".
    #[serde(default)]
    #[ts(optional)]
    pub verdict_at: Option<String>,
}

/// The user's decision on a token that is not on the built-in list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TokenVerdict {
    /// Taken into the totals.
    Confirmed,
    /// Hidden: listed nowhere and priced nowhere.
    Rejected,
}

impl CryptoToken {
    pub fn new(token: TokenId, symbol: &str, decimals: u8) -> Self {
        Self {
            chain: token.chain,
            contract: token.contract,
            symbol: symbol.to_string(),
            name: String::new(),
            decimals,
            coingecko_id: None,
            hidden: false,
            basis_usd: None,
            verdict: None,
            verdict_at: None,
        }
    }

    pub fn id(&self) -> TokenId {
        TokenId::new(self.chain, &self.contract)
    }
}

/// Which way an asset moved, from the followed account's point of view.
#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum CryptoDirection {
    /// Into the account.
    #[default]
    In,
    /// Out of the account.
    Out,
}

/// One asset movement of an operation. A swap has two legs, a transfer one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct CryptoLeg {
    pub direction: CryptoDirection,
    pub asset: CryptoAsset,
    /// Quantity in whole units (never raw integer units): money is `Amount`.
    pub amount: Amount,
    /// The other side of this leg, when the source names it.
    #[serde(default)]
    pub counterparty: Option<String>,
}

/// What the network charged for an operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct CryptoFee {
    pub asset: CryptoAsset,
    pub amount: Amount,
}

/// What an operation did, as far as the module could tell.
#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum CryptoTxKind {
    Transfer,
    Swap,
    Approve,
    Stake,
    Unstake,
    Claim,
    Bridge,
    /// A contract call the module does not recognize.
    Contract,
    #[default]
    Unknown,
}

/// What an operation was found through, so it can be fetched, deduplicated
/// and shown again. Never optional: an event whose origin is unknown cannot
/// be refreshed or deduplicated.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TrackingRef {
    /// One transaction of a network.
    ChainTx { chain: Chain, hash: String },
    /// The history of an address.
    Address { chain: Chain, address: String },
    /// A position inside a protocol.
    Position {
        protocol: String,
        chain: Chain,
        id: String,
    },
    /// A fill of an exchange (`exec_id` deduplicates it, as the Bybit import
    /// already does).
    ExchangeHistory { exchange: String, exec_id: String },
}

/// One operation of the crypto module. Built with every field named
/// (there is no `Default`), so a new source cannot forget
/// [`CryptoTx::tracking`] — see the examples at the top of this module.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct CryptoTx {
    pub id: String,
    pub chain: Chain,
    /// The followed account this event belongs to.
    pub account_id: String,
    /// RFC3339 UTC, as everywhere in the document.
    pub time: String,
    #[serde(default)]
    pub block: Option<u64>,
    /// Sender address, as the source reported it.
    #[serde(default)]
    pub from: String,
    /// Recipient address, as the source reported it.
    #[serde(default)]
    pub to: String,
    #[serde(default)]
    pub legs: Vec<CryptoLeg>,
    #[serde(default)]
    pub gas: Option<CryptoFee>,
    /// The protocol the call belongs to (`uniswap`, `aave`…), when known.
    #[serde(default)]
    pub protocol: Option<String>,
    /// The contract method, when the source named it.
    #[serde(default)]
    pub method: Option<String>,
    pub kind: CryptoTxKind,
    /// The source's own answer for this event, kept for a later re-read. A
    /// text, not a parsed value: every format stores it the same way.
    #[serde(default)]
    pub raw: Option<String>,
    pub tracking: TrackingRef,
}

/// The user's correction over what a source reported. The event itself is
/// left as it came, so a refetch does not lose the correction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct CryptoOverride {
    pub id: String,
    /// The event it corrects.
    pub event_id: String,
    #[serde(default)]
    pub kind: Option<CryptoTxKind>,
    #[serde(default)]
    pub comment: Option<String>,
    /// Keys of the ledger categories the operation counts under.
    #[serde(default)]
    pub categories: Vec<String>,
    /// The chain of operations this event belongs to: the same `flow_id` a
    /// fiat [`crate::model::transaction::TxMeta`] carries. This is how an
    /// exchange of fiat for coins is told: neither leg is an exchange on its
    /// own, the chain is (decision P6). It lives here and not on the event
    /// because a refetch rewrites the event and keeps the override.
    #[serde(default)]
    #[ts(optional)]
    pub flow_id: Option<String>,
    /// Who the operation went with or through — the same contact directory a
    /// fiat [`crate::model::transaction::TxMeta`] points at. The network only
    /// reports addresses, so «through whom» can never come from a source: an
    /// address matched against a contact's [`crate::model::contact::DetailKind::CryptoAddress`]
    /// account fills it, and the user names it where no match exists
    /// (decision P8).
    #[serde(default)]
    #[ts(optional)]
    pub contact_id: Option<String>,
    /// The user said this is not an exchange: the app stops suggesting a fiat
    /// operation for it. A decision about their own data, so it lives here and
    /// syncs, and is not a remembered browser setting (spec E10).
    #[serde(default)]
    #[ts(as = "Option<bool>", optional)]
    pub not_exchange: bool,
    /// Out of the module's lists and totals (dust, a scam airdrop).
    #[serde(default)]
    pub hidden: bool,
    /// The fiat record (`TxMeta::id`) the user pointed this operation at, so
    /// the two are shown to each other. A visible reference only: it joins no
    /// chain, changes no cost basis and no sum (that is `flow_id`). It lives
    /// here and not on the fiat `TxMeta` so it is gated and wiped with the
    /// crypto collections: a fiat record never carries a chain's data to a
    /// device below level 3 (spec E13). The record may be gone later; readers
    /// treat that as «not found», not as an error.
    #[serde(default)]
    #[ts(optional)]
    pub fiat_tx_id: Option<String>,
    pub updated_at: String,
}

impl CryptoOverride {
    /// Whether the correction changes what the operation itself shows, which
    /// is what the «corrected» mark on a row means. A pointer at a fiat
    /// record (`fiat_tx_id`, spec E13) says where the other half of the
    /// movement is written down; it corrects nothing about the operation, so
    /// an operation that carries only a pointer is not marked. A field added
    /// here later has to decide which of the two it is.
    pub fn corrects_the_operation(&self) -> bool {
        self.kind.is_some()
            || self.comment.is_some()
            || !self.categories.is_empty()
            || self.flow_id.is_some()
            || self.contact_id.is_some()
            || self.not_exchange
            || self.hidden
    }
}

/// A holding inside a protocol (lending, staking, a liquidity pool). Filled
/// by the DeFi stage (D0 and later); the field exists from E0 so the sync
/// registry and the vault schema do not change again.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct CryptoPosition {
    pub id: String,
    pub account_id: String,
    pub chain: Chain,
    /// `uniswap-v3`, `aave-v3`, …
    pub protocol: String,
    /// What the protocol calls this position, for the list.
    #[serde(default)]
    pub name: String,
    /// What is held in it; a debt leg is [`CryptoDirection::Out`].
    #[serde(default)]
    pub legs: Vec<CryptoLeg>,
    /// The tokens the protocol handed over as the receipt for *this* position
    /// (an aToken and its debt token, stETH, the position NFT), as the
    /// adapter that read the chain saw them.
    ///
    /// They sit in the wallet and a node reports them as holdings, so they
    /// are kept here to be left out of the capital — the money behind them is
    /// already counted as the position. The static table in
    /// `cryptotrack::protocols::receipt_tokens` can only hold what a protocol
    /// publishes for every market in advance; this is what was actually seen.
    #[serde(default)]
    pub receipts: Vec<CryptoAsset>,
    pub updated_at: String,
}

/// The module's value at a moment, kept so its history survives a refetch
/// (balances themselves are never stored).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct CryptoSnapshot {
    pub id: String,
    /// RFC3339 UTC.
    pub at: String,
    /// One account, or the whole module when `None`.
    #[serde(default)]
    pub account_id: Option<String>,
    pub value_usd: Amount,
}

/// How far a source has read one account. Belongs to the device: it names a
/// page of a provider this device talked to, so it is listed in
/// `sync::registry::LOCAL`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct CryptoCursor {
    /// `<source>|<account_id>`.
    pub id: String,
    pub source: String,
    pub account_id: String,
    /// The provider's own cursor (a block, a page token).
    #[serde(default)]
    pub cursor: Option<String>,
    pub updated_at: String,
}

/// A cached token price, in USD. A cache: it unions between devices and never
/// reaches the sync question (`cache: true` in the registry).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct CryptoPricePoint {
    /// [`CryptoAsset::key`].
    pub token_key: String,
    pub value_usd: Amount,
    /// RFC3339 UTC; the first sync sends only recent points.
    pub ts: String,
    /// The provider that quoted it.
    pub source: String,
}

/// A wipe of the module (spec E12), kept as a record so the other devices
/// learn about it through an ordinary sync instead of seeing the data come
/// back from them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct CryptoWipe {
    pub id: String,
    /// RFC3339 UTC of the wipe.
    pub at: String,
    /// The device that wiped.
    #[serde(default)]
    pub device_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chain_slug_matches_serde() {
        for chain in crate::model::chain::ALL_CHAINS {
            let serialized = serde_json::to_value(chain).expect("a chain serializes");
            assert_eq!(
                serde_json::Value::from(chain_slug(*chain)),
                serialized,
                "the stored slug of {chain:?} must stay the name serde writes"
            );
        }
    }

    #[test]
    fn an_evm_contract_is_one_token_in_any_case() {
        let upper = TokenId::new(
            Chain::Ethereum,
            " 0xA0B86991C6218B36C1D19D4A2E9EB0CE3606EB48 ",
        );
        let lower = TokenId::new(
            Chain::Ethereum,
            "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
        );
        assert_eq!(upper, lower);
        assert_eq!(
            upper.key(),
            "ethereum|0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
        );
    }

    #[test]
    fn a_case_sensitive_address_is_kept_as_typed() {
        let solana = "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU";
        assert_eq!(
            CryptoAccount::new("acc", "Main", Chain::Solana, solana, "2026-11-10T00:00:00Z")
                .address,
            solana
        );
        assert_eq!(
            CryptoAccount::new(
                "acc",
                "Main",
                Chain::Ethereum,
                "0xAAAA",
                "2026-11-10T00:00:00Z"
            )
            .address,
            "0xaaaa"
        );
    }

    #[test]
    fn a_native_coin_and_a_token_have_different_price_keys() {
        let native = CryptoAsset::Native {
            chain: Chain::Ethereum,
        };
        let token = CryptoAsset::Token(TokenId::new(Chain::Ethereum, "0xDead"));
        assert_eq!(native.key(), "ethereum|native");
        assert_eq!(token.key(), "ethereum|0xdead");
        assert_eq!(native.chain(), token.chain());
    }
}
