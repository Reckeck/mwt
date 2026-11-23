//! The HTTP half of the transaction lookup: keyless public explorers for Bitcoin and Solana.
//! It returns the providers' wire shapes untouched; the mapping to `TxInfo` lives in `mwt-usecases`.
//! Short timeouts: a slow explorer must never stall the form.

use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;

const MEMPOOL_SPACE_TX: &str = "https://mempool.space/api/tx";
const SOLANA_RPC: &str = "https://api.mainnet-beta.solana.com";

/// Deliberately short: a slow explorer must never stall the form.
const TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Debug, Deserialize)]
pub struct BtcPrevout {
    pub scriptpubkey_address: Option<String>,
    pub value: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct BtcVin {
    pub prevout: Option<BtcPrevout>,
}

#[derive(Debug, Deserialize)]
pub struct BtcVout {
    pub scriptpubkey_address: Option<String>,
    pub value: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct BtcStatus {
    pub confirmed: Option<bool>,
    pub block_time: Option<i64>,
}

/// A mempool.space transaction; amounts are satoshis.
#[derive(Debug, Deserialize)]
pub struct BtcTx {
    pub txid: Option<String>,
    pub fee: Option<i64>,
    pub status: Option<BtcStatus>,
    pub vin: Option<Vec<BtcVin>>,
    pub vout: Option<Vec<BtcVout>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SolTokenAmount {
    pub amount: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SolTokenBalance {
    pub mint: Option<String>,
    pub ui_token_amount: Option<SolTokenAmount>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SolMeta {
    pub fee: Option<i64>,
    pub pre_balances: Option<Vec<i64>>,
    pub post_balances: Option<Vec<i64>>,
    pub pre_token_balances: Option<Vec<SolTokenBalance>>,
    pub post_token_balances: Option<Vec<SolTokenBalance>>,
}

/// A Solana `getTransaction` result; amounts are lamports.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SolTx {
    pub block_time: Option<i64>,
    pub meta: Option<SolMeta>,
}

#[derive(Deserialize)]
struct RpcResp {
    result: Option<SolTx>,
}

#[derive(Debug, Clone)]
pub struct TxInfoClient {
    client: Client,
}

impl Default for TxInfoClient {
    fn default() -> Self {
        Self::with_timeout(TIMEOUT)
    }
}

impl TxInfoClient {
    pub fn with_timeout(timeout: Duration) -> Self {
        Self {
            client: Client::builder()
                .timeout(timeout)
                .build()
                .expect("reqwest client"),
        }
    }

    /// `None` means "this provider told us nothing": unreachable, non-2xx or unparsable.
    pub async fn bitcoin_tx(&self, tx_hash: &str) -> Option<BtcTx> {
        let url = format!("{MEMPOOL_SPACE_TX}/{tx_hash}");
        let resp = self.client.get(&url).send().await.ok()?;
        if !resp.status().is_success() {
            return None;
        }
        resp.json::<BtcTx>().await.ok()
    }

    /// Solana is a JSON-RPC API, so it needs a POST body rather than a path.
    /// `None` also when the node knows no such transaction.
    pub async fn solana_tx(&self, tx_hash: &str) -> Option<SolTx> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getTransaction",
            "params": [
                tx_hash,
                { "encoding": "jsonParsed", "maxSupportedTransactionVersion": 0 }
            ]
        });
        let resp = self.client.post(SOLANA_RPC).json(&body).send().await.ok()?;
        if !resp.status().is_success() {
            return None;
        }
        resp.json::<RpcResp>().await.ok()?.result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitcoin_wire_shape_parses_with_missing_fields() {
        let tx: BtcTx = serde_json::from_str(
            r#"{"txid":"ab","fee":141,
                "status":{"confirmed":true,"block_height":1,"block_time":1700000000},
                "vin":[{"prevout":{"scriptpubkey_address":"bc1a","value":1000}}],
                "vout":[{"scriptpubkey_address":"bc1b","value":859}]}"#,
        )
        .unwrap();
        assert_eq!(tx.fee, Some(141));
        assert_eq!(tx.status.unwrap().block_time, Some(1_700_000_000));
        assert_eq!(tx.vout.unwrap()[0].value, Some(859));
    }

    #[test]
    fn solana_wire_shape_is_the_real_get_transaction_result() {
        let parsed: RpcResp = serde_json::from_str(
            r#"{"jsonrpc":"2.0","id":1,"result":{"slot":430,"blockTime":1700000000,
                "meta":{"fee":5000,"preBalances":[100,2],"postBalances":[95,2],
                "preTokenBalances":[{"accountIndex":1,"mint":"m",
                "uiTokenAmount":{"amount":"7","decimals":6,"uiAmountString":"0.000007"}}]}}}"#,
        )
        .unwrap();
        let tx = parsed.result.unwrap();
        assert_eq!(tx.block_time, Some(1_700_000_000));
        let meta = tx.meta.unwrap();
        assert_eq!(meta.fee, Some(5000));
        assert_eq!(meta.pre_balances, Some(vec![100, 2]));
        let balance = &meta.pre_token_balances.unwrap()[0];
        assert_eq!(
            balance.ui_token_amount.as_ref().unwrap().amount.as_deref(),
            Some("7")
        );
    }

    #[test]
    fn solana_null_result_means_no_transaction() {
        let parsed: RpcResp =
            serde_json::from_str(r#"{"jsonrpc":"2.0","id":1,"result":null}"#).unwrap();
        assert!(parsed.result.is_none());
    }
}
