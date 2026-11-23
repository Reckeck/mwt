//! The TxInfo side of the transaction lookup (`mwt-money` keeps the offline half: `HashShape`, `TxInfo`;
//! `mwt-net` owns the HTTP client and the providers' wire types): optional enrichment from keyless
//! public explorers for Bitcoin and Solana.

use mwt_net::{BtcTx, SolTx, TxInfoClient};

use crate::error::Result;
pub use mwt_money::txinfo::*;

/// Satoshis -> BTC, without going through `f64` so 8 decimals stay exact.
fn sat_to_btc(sats: i64) -> String {
    use rust_decimal::Decimal;
    let btc = Decimal::from(sats) / Decimal::from(100_000_000i64);
    mwt_base::Amount::new(btc).to_string()
}

/// Lamports -> SOL (9 decimals).
fn lamports_to_sol(lamports: i64) -> String {
    use rust_decimal::Decimal;
    let sol = Decimal::from(lamports) / Decimal::from(1_000_000_000i64);
    mwt_base::Amount::new(sol).to_string()
}

#[derive(Debug, Clone, Default)]
pub struct TxInfoService {
    client: TxInfoClient,
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
        Ok(match chain {
            Chain::Bitcoin => self
                .client
                .bitcoin_tx(tx_hash)
                .await
                .map(|tx| bitcoin_info(tx, tx_hash)),
            Chain::Solana => self
                .client
                .solana_tx(tx_hash)
                .await
                .and_then(|tx| solana_info(tx, tx_hash)),
            // No keyless provider wired up for these yet.
            _ => None,
        })
    }
}

fn bitcoin_info(tx: BtcTx, tx_hash: &str) -> TxInfo {
    // BTC amounts are satoshis.
    let sats = |value: Option<i64>| value.map(sat_to_btc);

    // The first input and first output are the conventional "from"/"to"
    // of a simple transfer; enough to prefill the form.
    let first_in = tx
        .vin
        .as_ref()
        .and_then(|inputs| inputs.first())
        .and_then(|input| input.prevout.as_ref());
    let first_out = tx.vout.as_ref().and_then(|outputs| outputs.first());

    let occurred = tx.block_time.or(tx.time).map(mwt_model::unix_to_iso);

    TxInfo {
        tx_hash: tx.txid.unwrap_or_else(|| tx_hash.to_string()),
        network: Some(Chain::Bitcoin),
        explorer_url: Chain::Bitcoin.explorer_url(tx_hash),
        occurred_at: occurred,
        status: tx
            .status
            .and_then(|status| status.confirmed)
            .map(|confirmed| if confirmed { "confirmed" } else { "pending" }.to_string()),
        asset_in: first_in
            .and_then(|prevout| prevout.scriptpubkey_address.clone())
            .map(|_| "BTC".to_string()),
        amount_in: sats(first_in.and_then(|prevout| prevout.value)),
        asset_out: first_out
            .and_then(|output| output.scriptpubkey_address.clone())
            .map(|_| "BTC".to_string()),
        amount_out: sats(first_out.and_then(|output| output.value)),
        fee: sats(tx.fee),
        fee_asset: Some("BTC".to_string()),
        ..Default::default()
    }
}

/// `None` when the transaction carries no `meta`.
fn solana_info(value: SolTx, tx_hash: &str) -> Option<TxInfo> {
    let meta = value.meta?;

    // lamports -> SOL
    let sol = |value: Option<i64>| value.map(lamports_to_sol);

    let token_before = meta
        .pre_token_balances
        .as_ref()
        .and_then(|balances| balances.first());
    let token_after = meta
        .post_token_balances
        .as_ref()
        .and_then(|balances| balances.first());

    Some(TxInfo {
        tx_hash: tx_hash.to_string(),
        network: Some(Chain::Solana),
        explorer_url: Chain::Solana.explorer_url(tx_hash),
        occurred_at: value.block_time.map(mwt_model::unix_to_iso),
        status: Some("confirmed".to_string()),
        asset_in: token_before.and_then(|balance| balance.mint.clone()),
        amount_in: token_before
            .and_then(|balance| balance.token_amount.as_ref())
            .and_then(|token| token.amount.clone())
            .or_else(|| {
                sol(meta
                    .pre_balances
                    .and_then(|balances| balances.first().copied()))
            }),
        asset_out: token_after.and_then(|balance| balance.mint.clone()),
        amount_out: token_after
            .and_then(|balance| balance.token_amount.as_ref())
            .and_then(|token| token.amount.clone())
            .or_else(|| {
                sol(meta
                    .post_balances
                    .and_then(|balances| balances.first().copied()))
            }),
        fee: sol(meta.fee),
        fee_asset: Some("SOL".to_string()),
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const BTC_TXID: &str = "4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b";

    /// The whole point of the feature: an unreachable provider must still
    /// leave the caller with usable data instead of an error.
    #[tokio::test]
    async fn unreachable_provider_still_returns_offline_data() {
        let svc = TxInfoService {
            client: TxInfoClient::with_timeout(std::time::Duration::from_millis(1)),
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
}
