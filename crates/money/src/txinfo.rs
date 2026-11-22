//! Best-effort transaction lookup from a `tx_hash`.
//!
//! Two independent layers, so the feature degrades instead of failing:
//!
//! 1. [`HashShape`] — fully offline. Classifies the hash purely by its
//!    character set/length, which is enough to pre-select the network and
//!    build an explorer link without touching the network at all.
//! 2. [`TxInfoService`] — optional enrichment from keyless public explorers
//!    for the networks whose format is unambiguous (Bitcoin, Solana).
//!
//! Design rules, driven by the offline-first nature of the app:
//!
//! * **Never block the user.** Every failure path returns `Ok(None)` or an
//!   empty info struct; the caller leaves the form fields exactly as typed.
//! * **Never guess an EVM network.** `0x`-prefixed hashes are byte-identical
//!   across Ethereum/BSC/Polygon/Arbitrum/… , so they are reported as
//!   [`HashShape::EvmAmbiguous`] and the user picks the network.
//! * **Short timeouts.** Explorers rate-limit aggressively and may ban
//!   repeat offenders, so we give up quickly rather than hang the form.

use serde::{Deserialize, Serialize};

pub use mwt_model::Chain;

/// What a hash *looks* like, independent of any network call.
///
/// EVM chains all share one hash format, so they cannot be told apart from the
/// hash alone — hence [`HashShape::EvmAmbiguous`] rather than a guessed chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum HashShape {
    /// 32-byte hex with no `0x` prefix. This is the Bitcoin txid format, and
    /// also the Litecoin/Dogecoin one, so Bitcoin is the default and the user
    /// can still override it.
    BitcoinLike,
    /// Base58, 87-90 chars: a Solana signature.
    Solana,
    /// `0x` + 64 hex: any EVM chain, or Tron. Needs the user to say which.
    EvmAmbiguous,
    /// Recognised as hex/base58 but of an unexpected length.
    UnknownShape,
    /// Not a plausible transaction identifier at all.
    Malformed,
}

impl HashShape {
    /// Chains this shape could plausibly be, best guess first.
    pub fn candidates(self) -> Vec<Chain> {
        match self {
            HashShape::BitcoinLike => vec![Chain::Bitcoin, Chain::Litecoin, Chain::Dogecoin],
            HashShape::Solana => vec![Chain::Solana],
            HashShape::EvmAmbiguous => vec![
                Chain::Ethereum,
                Chain::Bsc,
                Chain::Polygon,
                Chain::Arbitrum,
                Chain::Optimism,
                Chain::Avalanche,
                Chain::Base,
                Chain::Tron,
            ],
            HashShape::UnknownShape | HashShape::Malformed => Vec::new(),
        }
    }

    /// `true` when the hash alone cannot identify the network, so the user has
    /// to choose. Only EVM-style hashes are genuinely ambiguous.
    pub fn needs_manual_network(self) -> bool {
        matches!(self, HashShape::EvmAmbiguous)
    }
}

/// Classify a transaction hash using only its shape. Pure and offline.
pub fn hash_shape(raw: &str) -> HashShape {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return HashShape::Malformed;
    }

    // EVM / Tron.
    if let Some(hex) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        return match (hex.len(), hex.chars().all(|char| char.is_ascii_hexdigit())) {
            (64, true) => HashShape::EvmAmbiguous,
            _ => HashShape::Malformed,
        };
    }

    let lower = trimmed.to_ascii_lowercase();
    let all_hex = lower.chars().all(|char| char.is_ascii_hexdigit());

    // Bitcoin-style txid: 64 hex chars, no `0x` prefix.
    if all_hex && trimmed.len() == 64 {
        return HashShape::BitcoinLike;
    }

    // Base58 (no 0/O/I/l): Solana signatures are 87-90 chars.
    if trimmed.chars().all(is_base58) {
        return match trimmed.len() {
            87..=90 => HashShape::Solana,
            _ => HashShape::UnknownShape,
        };
    }

    HashShape::Malformed
}

fn is_base58(char: char) -> bool {
    char.is_ascii_alphanumeric() && !"0OIl".contains(char)
}

/// Everything we could learn about a transaction. All fields optional: a
/// partial answer is still useful and is always safe to display.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub struct TxInfo {
    pub tx_hash: String,
    /// `None` when the shape alone cannot identify the network.
    pub network: Option<Chain>,
    /// Set when the user must resolve an ambiguous `0x…` hash themselves.
    pub needs_manual_network: bool,
    pub explorer_url: Option<String>,
    /// ISO-8601 UTC when the transaction was mined/confirmed.
    pub occurred_at: Option<String>,
    pub status: Option<String>,
    pub asset_in: Option<String>,
    pub amount_in: Option<String>,
    pub asset_out: Option<String>,
    pub amount_out: Option<String>,
    pub fee: Option<String>,
    pub fee_asset: Option<String>,
}

impl TxInfo {
    /// Offline-only result: classify the hash and build an explorer link.
    ///
    /// This is the guaranteed-available half of the feature.
    pub fn from_hash(raw: &str) -> TxInfo {
        let tx_hash = raw.trim().to_string();
        let shape = hash_shape(&tx_hash);

        let network = match shape {
            // Commit to a chain only for formats where one network dominates.
            HashShape::BitcoinLike => Some(Chain::Bitcoin),
            HashShape::Solana => Some(Chain::Solana),
            _ => None,
        };

        TxInfo {
            explorer_url: network.and_then(|chain| chain.explorer_url(&tx_hash)),
            tx_hash,
            network,
            needs_manual_network: shape.needs_manual_network(),
            ..Default::default()
        }
    }

    /// Overlay whatever the explorer returned, leaving unknown fields alone.
    pub fn merge(&mut self, other: TxInfo) {
        if other.network.is_some() {
            self.network = other.network;
        }
        if other.explorer_url.is_some() {
            self.explorer_url = other.explorer_url;
        }
        if other.occurred_at.is_some() {
            self.occurred_at = other.occurred_at;
        }
        if other.status.is_some() {
            self.status = other.status;
        }
        if other.asset_in.is_some() {
            self.asset_in = other.asset_in;
        }
        if other.amount_in.is_some() {
            self.amount_in = other.amount_in;
        }
        if other.asset_out.is_some() {
            self.asset_out = other.asset_out;
        }
        if other.amount_out.is_some() {
            self.amount_out = other.amount_out;
        }
        if other.fee.is_some() {
            self.fee = other.fee;
        }
        if other.fee_asset.is_some() {
            self.fee_asset = other.fee_asset;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BTC_TXID: &str = "4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b";
    const SOL_SIG: &str =
        "5h6xBEauJ3PK6SWCZ1PGhBvj8vDdWG3KpwecmoR9AC7XWkAA1sGdbEXbBnKUcv2fjXjNAgBy3s1W1sUKBZSgWE8d";
    const EVM_TX: &str = "0x71c7656ec7ab88b098defb751b7401b5f6d8976f5c1b42c39f5f4b0a3f2a1c9d";

    #[test]
    fn classifies_by_shape_offline() {
        assert_eq!(hash_shape(BTC_TXID), HashShape::BitcoinLike);
        assert_eq!(hash_shape(SOL_SIG), HashShape::Solana);
        assert_eq!(hash_shape(EVM_TX), HashShape::EvmAmbiguous);
        assert_eq!(
            hash_shape("0x0000000000000000000000000000000000000000000000000000000000000001"),
            HashShape::EvmAmbiguous
        );
    }

    #[test]
    fn rejects_implausible_hashes() {
        assert_eq!(hash_shape(""), HashShape::Malformed);
        assert_eq!(hash_shape("   "), HashShape::Malformed);
        assert_eq!(hash_shape("0x123"), HashShape::Malformed);
        // '0' is not base58 and this is too short to be anything else.
        assert_eq!(hash_shape("0O0O0O"), HashShape::Malformed);
        assert_eq!(hash_shape("not-a-hash-at-all"), HashShape::Malformed);
    }

    #[test]
    fn trims_surrounding_whitespace() {
        assert_eq!(
            hash_shape(&format!("  {BTC_TXID}\n")),
            HashShape::BitcoinLike
        );
    }

    #[test]
    fn evm_hashes_never_guess_the_network() {
        let info = TxInfo::from_hash(EVM_TX);
        assert_eq!(info.network, None);
        assert!(info.needs_manual_network);
        assert_eq!(info.explorer_url, None);
        assert!(!info.candidates_are_resolved());
    }

    #[test]
    fn unprefixed_64hex_defaults_to_bitcoin_but_stays_overridable() {
        // Litecoin/Dogecoin share this shape, so Bitcoin is only a default:
        // it must not be flagged as resolved-by-the-user nor be locked in.
        let info = TxInfo::from_hash(BTC_TXID);
        assert_eq!(info.network, Some(Chain::Bitcoin));
        assert!(!info.needs_manual_network);
        assert!(info.explorer_url.is_some());

        let mut forced = info.clone();
        forced.network = Some(Chain::Litecoin);
        forced.explorer_url = Chain::Litecoin.explorer_url(&forced.tx_hash);
        assert!(forced.explorer_url.unwrap().contains("litecoinspace.org"));
    }

    #[test]
    fn offline_info_builds_an_explorer_link() {
        let btc = TxInfo::from_hash(BTC_TXID);
        assert_eq!(
            btc.explorer_url.as_deref(),
            Some(
                "https://mempool.space/tx/4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b"
            )
        );
        let sol = TxInfo::from_hash(SOL_SIG);
        assert!(
            sol.explorer_url
                .unwrap()
                .starts_with("https://solscan.io/tx/")
        );
    }

    #[test]
    fn explicit_network_resolves_ambiguous_hashes() {
        let mut info = TxInfo::from_hash(EVM_TX);
        assert!(info.needs_manual_network);
        info.network = Some(Chain::Ethereum);
        info.needs_manual_network = false;
        info.explorer_url = Chain::Ethereum.explorer_url(&info.tx_hash);
        assert!(info.explorer_url.unwrap().contains("etherscan.io"));
    }

    /// Partial enrichment must not wipe out what the offline layer found.
    #[test]
    fn merge_keeps_existing_values_when_new_ones_are_absent() {
        let mut info = TxInfo::from_hash(BTC_TXID);
        let network_before = info.network;
        info.merge(TxInfo {
            fee: Some("0.0001".into()),
            ..Default::default()
        });
        assert_eq!(info.fee.as_deref(), Some("0.0001"));
        assert_eq!(info.network, network_before);
        assert!(info.explorer_url.is_some());
    }

    impl TxInfo {
        /// Test helper mirroring what the UI does with an unresolved hash.
        fn candidates_are_resolved(&self) -> bool {
            !self.needs_manual_network && self.network.is_some()
        }
    }
}
