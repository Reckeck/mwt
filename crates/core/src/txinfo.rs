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

use std::time::Duration;

use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::error::Result;
pub use crate::model::chain::Chain;

const MEMPOOL_SPACE_TX: &str = "https://mempool.space/api/tx";
const SOLANA_RPC: &str = "https://api.mainnet-beta.solana.com";

/// What a hash *looks* like, independent of any network call.
///
/// EVM chains all share one hash format, so they cannot be told apart from the
/// hash alone — hence [`HashShape::EvmAmbiguous`] rather than a guessed chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
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
    let h = raw.trim();
    if h.is_empty() {
        return HashShape::Malformed;
    }

    // EVM / Tron.
    if let Some(hex) = h.strip_prefix("0x").or_else(|| h.strip_prefix("0X")) {
        return match (hex.len(), hex.chars().all(|c| c.is_ascii_hexdigit())) {
            (64, true) => HashShape::EvmAmbiguous,
            _ => HashShape::Malformed,
        };
    }

    let lower = h.to_ascii_lowercase();
    let all_hex = lower.chars().all(|c| c.is_ascii_hexdigit());

    // Bitcoin-style txid: 64 hex chars, no `0x` prefix.
    if all_hex && h.len() == 64 {
        return HashShape::BitcoinLike;
    }

    // Base58 (no 0/O/I/l): Solana signatures are 87-90 chars.
    if h.chars().all(is_base58) {
        return match h.len() {
            87..=90 => HashShape::Solana,
            _ => HashShape::UnknownShape,
        };
    }

    HashShape::Malformed
}

fn is_base58(c: char) -> bool {
    c.is_ascii_alphanumeric() && !"0OIl".contains(c)
}

/// Satoshis -> BTC, without going through `f64` so 8 decimals stay exact.
fn sat_to_btc(sats: i64) -> String {
    use rust_decimal::Decimal;
    let btc = Decimal::from(sats) / Decimal::from(100_000_000i64);
    crate::Amount::new(btc).to_string()
}

/// Lamports -> SOL (9 decimals).
fn lamports_to_sol(lamports: i64) -> String {
    use rust_decimal::Decimal;
    let sol = Decimal::from(lamports) / Decimal::from(1_000_000_000i64);
    crate::Amount::new(sol).to_string()
}

/// Everything we could learn about a transaction. All fields optional: a
/// partial answer is still useful and is always safe to display.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, ts_rs::TS)]
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
            explorer_url: network.and_then(|c| c.explorer_url(&tx_hash)),
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

#[derive(Debug, Clone)]
pub struct TxInfoService {
    client: Client,
}

impl Default for TxInfoService {
    fn default() -> Self {
        Self {
            // Deliberately short: a slow explorer must never stall the form.
            client: Client::builder()
                .timeout(Duration::from_secs(8))
                .build()
                .expect("reqwest client"),
        }
    }
}

impl TxInfoService {
    /// Offline classification first, then opportunistic enrichment.
    ///
    /// Always succeeds: network problems, unsupported chains and malformed
    /// hashes all return whatever the offline layer could determine.
    pub async fn inspect(&self, raw: &str, forced: Option<Chain>) -> Result<TxInfo> {
        let mut info = TxInfo::from_hash(raw);

        // An explicit network choice always wins over the heuristic.
        if let Some(chain) = forced {
            info.network = Some(chain);
            info.explorer_url = chain.explorer_url(&info.tx_hash);
            info.needs_manual_network = false;
        }

        let Some(chain) = info.network else {
            return Ok(info);
        };

        if let Ok(Some(extra)) = self.fetch(chain, &info.tx_hash).await {
            info.merge(extra);
        }
        Ok(info)
    }

    /// Provider lookup. `Ok(None)` means "this provider told us nothing".
    async fn fetch(&self, chain: Chain, tx_hash: &str) -> Result<Option<TxInfo>> {
        match chain {
            Chain::Bitcoin => self.fetch_bitcoin(tx_hash).await,
            Chain::Solana => self.fetch_solana(tx_hash).await,
            // No keyless provider wired up for these yet.
            _ => Ok(None),
        }
    }

    async fn fetch_bitcoin(&self, tx_hash: &str) -> Result<Option<TxInfo>> {
        #[derive(Deserialize)]
        struct Prevout {
            scriptpubkey_address: Option<String>,
            value: Option<i64>,
        }
        #[derive(Deserialize)]
        struct Vin {
            prevout: Option<Prevout>,
        }
        #[derive(Deserialize)]
        struct Vout {
            scriptpubkey_address: Option<String>,
            value: Option<i64>,
        }
        #[derive(Deserialize)]
        struct Status {
            confirmed: Option<bool>,
        }
        #[derive(Deserialize)]
        struct Tx {
            txid: Option<String>,
            fee: Option<i64>,
            time: Option<i64>,
            block_time: Option<i64>,
            status: Option<Status>,
            vin: Option<Vec<Vin>>,
            vout: Option<Vec<Vout>>,
        }

        let url = format!("{MEMPOOL_SPACE_TX}/{tx_hash}");
        let Ok(resp) = self.client.get(&url).send().await else {
            return Ok(None);
        };
        if !resp.status().is_success() {
            return Ok(None);
        }
        let Ok(tx) = resp.json::<Tx>().await else {
            return Ok(None);
        };

        // BTC amounts are satoshis.
        let sats = |v: Option<i64>| v.map(sat_to_btc);

        // The first input and first output are the conventional "from"/"to"
        // of a simple transfer; enough to prefill the form.
        let first_in = tx
            .vin
            .as_ref()
            .and_then(|v| v.first())
            .and_then(|v| v.prevout.as_ref());
        let first_out = tx.vout.as_ref().and_then(|v| v.first());

        let occurred = tx
            .block_time
            .or(tx.time)
            .map(crate::model::vault::unix_to_iso);

        Ok(Some(TxInfo {
            tx_hash: tx.txid.unwrap_or_else(|| tx_hash.to_string()),
            network: Some(Chain::Bitcoin),
            explorer_url: Chain::Bitcoin.explorer_url(tx_hash),
            occurred_at: occurred,
            status: tx
                .status
                .and_then(|s| s.confirmed)
                .map(|c| if c { "confirmed" } else { "pending" }.to_string()),
            asset_in: first_in
                .and_then(|p| p.scriptpubkey_address.clone())
                .map(|_| "BTC".to_string()),
            amount_in: sats(first_in.and_then(|p| p.value)),
            asset_out: first_out
                .and_then(|v| v.scriptpubkey_address.clone())
                .map(|_| "BTC".to_string()),
            amount_out: sats(first_out.and_then(|v| v.value)),
            fee: sats(tx.fee),
            fee_asset: Some("BTC".to_string()),
            ..Default::default()
        }))
    }

    async fn fetch_solana(&self, tx_hash: &str) -> Result<Option<TxInfo>> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct TokenAmount {
            amount: Option<String>,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct TokenBalance {
            mint: Option<String>,
            token_amount: Option<TokenAmount>,
        }
        #[derive(Deserialize)]
        struct Meta {
            fee: Option<i64>,
            pre_balances: Option<Vec<i64>>,
            post_balances: Option<Vec<i64>>,
            pre_token_balances: Option<Vec<TokenBalance>>,
            post_token_balances: Option<Vec<TokenBalance>>,
        }
        #[derive(Deserialize)]
        struct Tx {
            block_time: Option<i64>,
            meta: Option<Meta>,
        }
        #[derive(Deserialize)]
        struct RpcResult {
            value: Option<Tx>,
        }
        #[derive(Deserialize)]
        struct RpcResp {
            result: Option<RpcResult>,
        }

        // Solana is a JSON-RPC API, so it needs a POST body rather than a path.
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getTransaction",
            "params": [
                tx_hash,
                { "encoding": "jsonParsed", "maxSupportedTransactionVersion": 0 }
            ]
        });
        let Ok(resp) = self.client.post(SOLANA_RPC).json(&body).send().await else {
            return Ok(None);
        };
        if !resp.status().is_success() {
            return Ok(None);
        }
        let Ok(parsed) = resp.json::<RpcResp>().await else {
            return Ok(None);
        };
        let Some(value) = parsed.result.and_then(|r| r.value) else {
            return Ok(None);
        };
        let Some(meta) = value.meta else {
            return Ok(None);
        };

        // lamports -> SOL
        let sol = |v: Option<i64>| v.map(lamports_to_sol);

        let token_before = meta.pre_token_balances.as_ref().and_then(|v| v.first());
        let token_after = meta.post_token_balances.as_ref().and_then(|v| v.first());

        Ok(Some(TxInfo {
            tx_hash: tx_hash.to_string(),
            network: Some(Chain::Solana),
            explorer_url: Chain::Solana.explorer_url(tx_hash),
            occurred_at: value.block_time.map(crate::model::vault::unix_to_iso),
            status: Some("confirmed".to_string()),
            asset_in: token_before.and_then(|a| a.mint.clone()),
            amount_in: token_before
                .and_then(|a| a.token_amount.as_ref())
                .and_then(|t| t.amount.clone())
                .or_else(|| sol(meta.pre_balances.and_then(|b| b.first().copied()))),
            asset_out: token_after.and_then(|a| a.mint.clone()),
            amount_out: token_after
                .and_then(|a| a.token_amount.as_ref())
                .and_then(|t| t.amount.clone())
                .or_else(|| sol(meta.post_balances.and_then(|b| b.first().copied()))),
            fee: sol(meta.fee),
            fee_asset: Some("SOL".to_string()),
            ..Default::default()
        }))
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

    /// The whole point of the feature: an unreachable provider must still
    /// leave the caller with usable data instead of an error.
    #[tokio::test]
    async fn unreachable_provider_still_returns_offline_data() {
        let svc = TxInfoService {
            client: Client::builder()
                .timeout(Duration::from_millis(1))
                .build()
                .unwrap(),
        };
        let info = svc
            .inspect(BTC_TXID, None)
            .await
            .expect("inspect must never fail");
        assert_eq!(info.network, Some(Chain::Bitcoin));
        assert!(info.explorer_url.is_some());
    }

    #[tokio::test]
    async fn malformed_hash_never_touches_the_network() {
        let svc = TxInfoService::default();
        let info = svc.inspect("garbage", None).await.unwrap();
        assert_eq!(info.network, None);
        assert_eq!(info.tx_hash, "garbage");
    }

    impl TxInfo {
        /// Test helper mirroring what the UI does with an unresolved hash.
        fn candidates_are_resolved(&self) -> bool {
            !self.needs_manual_network && self.network.is_some()
        }
    }
}
