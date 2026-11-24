//! The TxInfo side of the transaction lookup (`mwt-money` keeps the offline half: `HashShape`, `TxInfo`;
//! `mwt-net` owns the HTTP client and the providers' wire types): optional enrichment from keyless
//! public explorers for Bitcoin and Solana.

use mwt_net::{BtcTx, SolTokenAmount, SolTokenBalance, SolTx, TxInfoClient};

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

    let occurred = tx
        .status
        .as_ref()
        .and_then(|status| status.block_time)
        .map(mwt_model::unix_to_iso);

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

/// Decimal-adjusted token amount: `uiAmountString`, else `amount` shifted by `decimals`
/// with string arithmetic (no floats).
fn token_amount(token: &SolTokenAmount) -> Option<String> {
    if let Some(ui) = &token.ui_amount_string {
        return Some(ui.clone());
    }
    let raw = token.amount.as_deref()?;
    if raw.is_empty() || !raw.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let decimals = usize::from(token.decimals?);
    if decimals == 0 {
        return Some(raw.to_string());
    }
    let padded = format!("{raw:0>width$}", width = decimals + 1);
    let (whole, fraction) = padded.split_at(padded.len() - decimals);
    let whole = whole.trim_start_matches('0');
    let whole = if whole.is_empty() { "0" } else { whole };
    let fraction = fraction.trim_end_matches('0');
    Some(if fraction.is_empty() {
        whole.to_string()
    } else {
        format!("{whole}.{fraction}")
    })
}

/// Amount of the first token balance; the SOL lamport balance only when there is no token balance.
fn balance_amount(token: Option<&SolTokenBalance>, lamports: Option<i64>) -> Option<String> {
    match token {
        Some(balance) => balance.ui_token_amount.as_ref().and_then(token_amount),
        None => lamports.map(lamports_to_sol),
    }
}

/// `None` when the transaction carries no `meta`.
fn solana_info(value: SolTx, tx_hash: &str) -> Option<TxInfo> {
    let meta = value.meta?;

    let token_before = meta
        .pre_token_balances
        .as_ref()
        .and_then(|balances| balances.first());
    let token_after = meta
        .post_token_balances
        .as_ref()
        .and_then(|balances| balances.first());
    let first =
        |balances: &Option<Vec<i64>>| balances.as_ref().and_then(|list| list.first().copied());

    Some(TxInfo {
        tx_hash: tx_hash.to_string(),
        network: Some(Chain::Solana),
        explorer_url: Chain::Solana.explorer_url(tx_hash),
        occurred_at: value.block_time.map(mwt_model::unix_to_iso),
        status: Some("confirmed".to_string()),
        asset_in: token_before.and_then(|balance| balance.mint.clone()),
        amount_in: balance_amount(token_before, first(&meta.pre_balances)),
        asset_out: token_after.and_then(|balance| balance.mint.clone()),
        amount_out: balance_amount(token_after, first(&meta.post_balances)),
        fee: meta.fee.map(lamports_to_sol),
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

    #[test]
    fn bitcoin_info_maps_date_amounts_fee_and_status() {
        use mwt_net::{BtcPrevout, BtcStatus, BtcVin, BtcVout};
        let tx = BtcTx {
            txid: Some("ab".into()),
            fee: Some(141),
            status: Some(BtcStatus {
                confirmed: Some(true),
                block_time: Some(1_700_000_000),
            }),
            vin: Some(vec![BtcVin {
                prevout: Some(BtcPrevout {
                    scriptpubkey_address: Some("bc1a".into()),
                    value: Some(1000),
                }),
            }]),
            vout: Some(vec![BtcVout {
                scriptpubkey_address: Some("bc1b".into()),
                value: Some(859),
            }]),
        };
        let info = bitcoin_info(tx, BTC_TXID);
        assert_eq!(
            info.occurred_at,
            Some(mwt_model::unix_to_iso(1_700_000_000))
        );
        assert_eq!(info.status.as_deref(), Some("confirmed"));
        assert_eq!(info.amount_in.as_deref(), Some("0.00001"));
        assert_eq!(info.amount_out.as_deref(), Some("0.00000859"));
        assert_eq!(info.fee.as_deref(), Some("0.00000141"));
    }

    #[test]
    fn solana_info_maps_date_amounts_fee_and_status() {
        use mwt_net::SolMeta;
        let balance = |amount: &str, ui: &str| token_balance(amount, 6, Some(ui));
        let tx = SolTx {
            block_time: Some(1_700_000_000),
            meta: Some(SolMeta {
                fee: Some(5000),
                pre_balances: None,
                post_balances: None,
                pre_token_balances: Some(vec![balance("7", "0.000007")]),
                post_token_balances: Some(vec![balance("5", "0.000005")]),
            }),
        };
        let info = solana_info(tx, "sig").expect("meta present");
        assert_eq!(
            info.occurred_at,
            Some(mwt_model::unix_to_iso(1_700_000_000))
        );
        assert_eq!(info.status.as_deref(), Some("confirmed"));
        assert_eq!(info.amount_in.as_deref(), Some("0.000007"));
        assert_eq!(info.amount_out.as_deref(), Some("0.000005"));
        assert_eq!(info.fee.as_deref(), Some("0.000005"));
    }

    fn token_balance(amount: &str, decimals: u8, ui: Option<&str>) -> SolTokenBalance {
        SolTokenBalance {
            mint: Some("mint".into()),
            ui_token_amount: Some(SolTokenAmount {
                amount: Some(amount.into()),
                decimals: Some(decimals),
                ui_amount_string: ui.map(String::from),
            }),
        }
    }

    #[test]
    fn token_amount_is_decimal_adjusted() {
        let balance = token_balance("7", 6, Some("0.000007"));
        let amount = balance.ui_token_amount.as_ref().and_then(token_amount);
        assert_eq!(amount.as_deref(), Some("0.000007"));
    }

    #[test]
    fn token_amount_shifts_decimals_without_ui_string() {
        for (raw, expected) in [("7", "0.000007"), ("1500000", "1.5"), ("2000000", "2")] {
            let balance = token_balance(raw, 6, None);
            let amount = balance.ui_token_amount.as_ref().and_then(token_amount);
            assert_eq!(amount.as_deref(), Some(expected));
        }
    }

    #[test]
    fn token_balance_never_falls_back_to_sol() {
        use mwt_net::SolMeta;
        let tx = SolTx {
            block_time: None,
            meta: Some(SolMeta {
                fee: None,
                pre_balances: Some(vec![1_000_000_000]),
                post_balances: Some(vec![1_000_000_000]),
                pre_token_balances: Some(vec![SolTokenBalance {
                    mint: Some("mint".into()),
                    ui_token_amount: None,
                }]),
                post_token_balances: None,
            }),
        };
        let info = solana_info(tx, "sig").expect("meta present");
        assert_eq!(info.asset_in.as_deref(), Some("mint"));
        assert_eq!(info.amount_in, None);
        assert_eq!(info.amount_out.as_deref(), Some("1"));
    }

    #[test]
    fn token_amount_keeps_integer_for_zero_decimals() {
        let balance = token_balance("5", 0, None);
        let amount = balance.ui_token_amount.as_ref().and_then(token_amount);
        assert_eq!(amount.as_deref(), Some("5"));
    }
}
