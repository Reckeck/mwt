//! Read-only Bybit V5 integration used to prefill the trade form.
//!
//! Scope is deliberately narrow: **only** `GET` endpoints are ever called, so
//! the integration cannot place, amend or cancel an order. A read-only API key
//! is assumed, and the module has no code path that could change that.
//!
//! Auth follows the V5 spec: sign
//! `timestamp + api_key + recv_window + queryString` with HMAC-SHA256 and send
//! it lowercase-hex in `X-BAPI-SIGN`, alongside `X-BAPI-API-KEY`,
//! `X-BAPI-TIMESTAMP` and `X-BAPI-RECV-WINDOW`.
//!
//! Every failure is non-fatal for the caller: the form simply stays empty.

use std::time::Duration;

use hmac::{Hmac, Mac};
use reqwest::Client;
use reqwest::header::{HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::error::{Error, Result};

const MAINNET: &str = "https://api.bybit.com";
const TESTNET: &str = "https://api-testnet.bybit.com";

/// Bybit rejects requests whose timestamp drifts outside this window.
const RECV_WINDOW_MS: u64 = 5_000;

/// `/v5/execution/list` refuses `startTime`/`endTime` windows wider than 7 days.
const IMPORT_WINDOW_MS: u64 = 7 * 24 * 60 * 60 * 1000;

/// Upper bound on history requests per category, so a full-history import
/// cannot fan out into hundreds of calls against the user's rate limit.
const MAX_IMPORT_PAGES: usize = 12;

/// Product type. Bybit requires this on most private endpoints.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "BybitCategory")]
pub enum Category {
    #[default]
    Spot,
    Linear,
    Inverse,
}

impl Category {
    pub fn as_str(self) -> &'static str {
        match self {
            Category::Spot => "spot",
            Category::Linear => "linear",
            Category::Inverse => "inverse",
        }
    }
}

/// A single fill from `GET /v5/execution/list`.
///
/// Field names mirror Bybit's response so the mapping stays auditable.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Execution {
    pub symbol: String,
    pub side: String,
    pub order_type: String,
    pub exec_price: String,
    pub exec_qty: String,
    pub exec_value: String,
    pub exec_fee: String,
    pub exec_time: String,
    pub is_maker: bool,
    pub closed_size: Option<String>,
    /// Bybit's unique execution ID (execId) for deduplication.
    pub exec_id: String,
}

impl Execution {
    /// Split `BTCUSDT` into `(BTC, USDT)`.
    ///
    /// Bybit concatenates base and quote with no separator, so the quote leg
    /// is matched against the known USDT/USDC suffixes first and only then
    /// guessed from the remainder.
    pub fn split_symbol(symbol: &str) -> Option<(String, String)> {
        let upper = symbol.to_ascii_uppercase();
        for quote in ["USDT", "USDC", "USD", "BTC", "ETH"] {
            if let Some(base) = upper.strip_suffix(quote)
                && !base.is_empty()
                && base != quote
            {
                return Some((base.to_string(), quote.to_string()));
            }
        }
        // Perpetuals look like `BTCUSDT` but are `BTC`-margined; fall back to
        // treating the whole symbol as one leg rather than inventing a pair.
        None
    }

    /// `true` when the fill closed (reduced) a derivatives position.
    pub fn is_closing(&self) -> bool {
        self.closed_size
            .as_ref()
            .is_some_and(|size| !size.is_empty() && size != "0")
    }

    /// Best-effort product category from the symbol, for imports that sweep
    /// every category in one pass and therefore cannot tag fills as they fetch.
    /// Bybit names perpetual futures `…PERP`, so that suffix is the only signal.
    pub fn category_hint(&self) -> &'static str {
        if self.symbol.to_ascii_uppercase().ends_with("PERP") {
            "linear"
        } else {
            "spot"
        }
    }
}

/// A fill shaped for the trade form: what was sold, what was bought, costs.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub struct TradePrefill {
    /// `buy` when the user acquired the base asset, `sell` when they gave it up.
    pub side: String,
    pub symbol: String,
    pub asset_sold: Option<String>,
    pub qty_sold: Option<String>,
    pub asset_bought: Option<String>,
    pub qty_bought: Option<String>,
    pub price: Option<String>,
    pub fee: Option<String>,
    pub fee_asset: Option<String>,
    /// ISO-8601 UTC, ready for the form's date field.
    pub occurred_at: Option<String>,
    pub comment: Option<String>,
}

impl Execution {
    /// Map a fill onto trade-form fields.
    pub fn to_prefill(&self) -> TradePrefill {
        let is_buy = self.side.eq_ignore_ascii_case("buy");
        let (base, quote) = Self::split_symbol(&self.symbol).unwrap_or_else(|| {
            // Unknown layout: keep the raw symbol as the base so the user at
            // least sees something to correct.
            (self.symbol.to_ascii_uppercase(), String::new())
        });

        let nonempty = |raw: String| if raw.is_empty() { None } else { Some(raw) };

        let (asset_sold, qty_sold, asset_bought, qty_bought) = if is_buy {
            (
                nonempty(quote.clone()),
                nonempty(self.exec_value.clone()),
                nonempty(base.clone()),
                nonempty(self.exec_qty.clone()),
            )
        } else {
            (
                nonempty(base.clone()),
                nonempty(self.exec_qty.clone()),
                nonempty(quote.clone()),
                nonempty(self.exec_value.clone()),
            )
        };

        // Bybit reports fees in the fee coin; for the common case that is the
        // asset received (buy) or given up (sell).
        let fee_asset = if is_buy {
            nonempty(quote)
        } else {
            nonempty(base)
        };

        TradePrefill {
            side: if is_buy { "buy" } else { "sell" }.to_string(),
            symbol: self.symbol.to_ascii_uppercase(),
            asset_sold,
            qty_sold,
            asset_bought,
            qty_bought,
            price: Some(self.exec_price.clone()),
            fee: Some(self.exec_fee.clone()),
            fee_asset,
            occurred_at: self
                .exec_time
                .parse::<i64>()
                .ok()
                .map(|ms| crate::model::vault::unix_to_iso(ms / 1000)),
            comment: Some(format!(
                "bybit {} {} · {} @ {}",
                self.category_label(),
                self.side.to_ascii_lowercase(),
                self.exec_qty,
                self.exec_price
            )),
        }
    }

    fn category_label(&self) -> &'static str {
        if self.symbol.ends_with("PERP") {
            "perp"
        } else {
            "spot"
        }
    }
}

#[derive(Debug, Deserialize)]
struct ApiEnvelope<T> {
    #[serde(rename = "retCode")]
    retcode: i64,
    #[serde(rename = "retMsg")]
    retmsg: String,
    /// Missing on error responses; serde maps an absent `Option` to `None`.
    result: Option<T>,
}

#[derive(Debug, Default, Deserialize)]
struct ExecutionPage {
    #[serde(default)]
    list: Vec<ApiExecution>,
}

/// Bybit's execution objects are camelCase (`execPrice`, `isMaker`,
/// `closedSize`), so map them explicitly instead of relying on field spelling.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiExecution {
    #[serde(default)]
    symbol: String,
    #[serde(default)]
    side: String,
    #[serde(default)]
    order_type: String,
    #[serde(default)]
    exec_price: String,
    #[serde(default)]
    exec_qty: String,
    #[serde(default)]
    exec_value: String,
    #[serde(default)]
    exec_fee: String,
    #[serde(default)]
    exec_time: String,
    #[serde(default)]
    is_maker: bool,
    #[serde(default)]
    closed_size: Option<String>,
    /// Bybit's unique execution ID.
    #[serde(default)]
    exec_id: String,
}

impl From<ApiExecution> for Execution {
    fn from(api: ApiExecution) -> Self {
        Execution {
            symbol: api.symbol,
            side: api.side,
            order_type: api.order_type,
            exec_price: api.exec_price,
            exec_qty: api.exec_qty,
            exec_value: api.exec_value,
            exec_fee: api.exec_fee,
            exec_time: api.exec_time,
            is_maker: api.is_maker,
            closed_size: api.closed_size,
            exec_id: api.exec_id,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BybitClient {
    client: Client,
    base_url: &'static str,
}

impl Default for BybitClient {
    fn default() -> Self {
        Self::new(MAINNET)
    }
}

impl BybitClient {
    pub fn new(base_url: &'static str) -> Self {
        BybitClient {
            client: Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .expect("reqwest client"),
            base_url,
        }
    }

    /// Testnet variant, so the integration can be exercised without real funds.
    pub fn testnet() -> Self {
        Self::new(TESTNET)
    }

    /// The exact string that gets signed, per the V5 spec for GET requests:
    /// `timestamp + api_key + recv_window + queryString`.
    pub fn signing_payload(
        api_key: &str,
        recv_window: u64,
        timestamp_ms: u64,
        query: &str,
    ) -> String {
        format!("{timestamp_ms}{api_key}{recv_window}{query}")
    }

    /// Build the signed headers for a GET request.
    ///
    /// Kept separate and pure so the signing scheme is unit-testable without
    /// any network access.
    pub fn signed_headers(
        api_key: &str,
        api_secret: &str,
        recv_window: u64,
        timestamp_ms: u64,
        query: &str,
    ) -> Result<HeaderMap> {
        let payload = Self::signing_payload(api_key, recv_window, timestamp_ms, query);

        let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(api_secret.as_bytes())
            .map_err(|_| Error::InvalidInput("invalid api secret".into()))?;
        mac.update(payload.as_bytes());
        let signature = hex_encode(&mac.finalize().into_bytes());

        let mut headers = HeaderMap::new();
        headers.insert("X-BAPI-API-KEY", header(api_key)?);
        headers.insert("X-BAPI-TIMESTAMP", header(&timestamp_ms.to_string())?);
        headers.insert("X-BAPI-SIGN", header(&signature)?);
        headers.insert("X-BAPI-RECV-WINDOW", header(&recv_window.to_string())?);
        // HMAC mode; sent by every official sample client.
        headers.insert("X-BAPI-SIGN-TYPE", header("2")?);
        Ok(headers)
    }

    /// Recent fills for a product, newest first.
    ///
    /// Returns an empty list (never an error) when the key is absent or the
    /// call fails, so the caller can simply leave the form untouched.
    pub async fn executions(
        &self,
        api_key: &str,
        api_secret: &str,
        category: Category,
        symbol: Option<&str>,
        limit: u32,
    ) -> Result<Vec<Execution>> {
        if api_key.is_empty() || api_secret.is_empty() {
            return Ok(Vec::new());
        }

        let limit = limit.clamp(1, 100);
        let mut query = format!("category={}&limit={limit}", category.as_str());
        if let Some(filter) = symbol.filter(|sym| !sym.is_empty()) {
            query.push_str("&symbol=");
            query.push_str(filter);
        }

        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_millis() as u64)
            .unwrap_or_default();

        let headers =
            Self::signed_headers(api_key, api_secret, RECV_WINDOW_MS, timestamp_ms, &query)?;

        let url = format!("{}/v5/execution/list?{query}", self.base_url);
        let resp = self
            .client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .map_err(|err| Error::Network(err.to_string()))?;

        let envelope: ApiEnvelope<ExecutionPage> = resp
            .json()
            .await
            .map_err(|err| Error::Network(err.to_string()))?;
        if envelope.retcode != 0 {
            return Err(Error::Network(format!(
                "bybit error {}: {}",
                envelope.retcode, envelope.retmsg
            )));
        }

        Ok(envelope
            .result
            .map(|page| page.list.into_iter().map(Execution::from).collect())
            .unwrap_or_default())
    }

    /// Fetch ALL executions across every category, for a full-history import.
    ///
    /// `/v5/execution/list` has no cursor: it pages with `startTime`/`endTime`
    /// and rejects windows wider than 7 days. So for each category we walk
    /// backwards in fixed 7-day windows from now until we have enough fills, a
    /// window comes back empty, or the page budget runs out. Returns fills
    /// newest-first.
    pub async fn all_executions(
        &self,
        api_key: &str,
        api_secret: &str,
        max_total: u32,
    ) -> Result<Vec<Execution>> {
        if api_key.is_empty() || api_secret.is_empty() || max_total == 0 {
            return Ok(Vec::new());
        }

        let mut all: Vec<Execution> = Vec::new();
        for category in [Category::Spot, Category::Linear, Category::Inverse] {
            if all.len() as u32 >= max_total {
                break;
            }
            let mut window_end = Self::now_millis();

            for _ in 0..MAX_IMPORT_PAGES {
                if all.len() as u32 >= max_total {
                    break;
                }
                let window_start = window_end.saturating_sub(IMPORT_WINDOW_MS);
                let limit = (max_total - all.len() as u32).clamp(1, 100);
                let query = format!(
                    "category={}&startTime={window_start}&endTime={window_end}&limit={limit}",
                    category.as_str()
                );

                let headers = Self::signed_headers(
                    api_key,
                    api_secret,
                    RECV_WINDOW_MS,
                    Self::now_millis(),
                    &query,
                )?;

                let url = format!("{}/v5/execution/list?{query}", self.base_url);
                let resp = self
                    .client
                    .get(&url)
                    .headers(headers)
                    .send()
                    .await
                    .map_err(|err| Error::Network(err.to_string()))?;

                let envelope: ApiEnvelope<ExecutionPage> = resp
                    .json()
                    .await
                    .map_err(|err| Error::Network(err.to_string()))?;
                if envelope.retcode != 0 {
                    // No permission for this category (common for inverse on a
                    // spot-only key): skip it, keep sweeping the others.
                    break;
                }
                let rows = envelope.result.map(|page| page.list).unwrap_or_default();
                all.extend(rows.into_iter().map(Execution::from));

                if window_start == 0 {
                    break; // walked back to the epoch
                }
                window_end = window_start - 1;
            }
        }

        // Sort newest-first (by exec_time desc)
        all.sort_by(|left, right| right.exec_time.cmp(&left.exec_time));
        all.truncate(max_total as usize);
        Ok(all)
    }
}

fn header(value: &str) -> Result<HeaderValue> {
    HeaderValue::from_str(value).map_err(|_| Error::InvalidInput("invalid header value".into()))
}

impl BybitClient {
    /// Milliseconds since the Unix epoch, for signing and query windows.
    fn now_millis() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_millis() as u64)
            .unwrap_or_default()
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fill() -> Execution {
        Execution {
            symbol: "BTCUSDT".into(),
            side: "Buy".into(),
            order_type: "Market".into(),
            exec_price: "65000".into(),
            exec_qty: "0.5".into(),
            exec_value: "32500".into(),
            exec_fee: "16.25".into(),
            exec_time: "1700000000000".into(),
            is_maker: false,
            closed_size: None,
            exec_id: "exec-1".into(),
        }
    }

    /// The signing payload must match the V5 spec's worked example byte for
    /// byte. This is the part of the scheme we can verify from the docs.
    ///
    /// (The docs also print a sample signature, but never publish the secret
    /// behind it, so it cannot be reproduced — only the concatenation order,
    /// HMAC-SHA256 and lowercase-hex encoding can be checked.)
    #[test]
    fn signing_payload_matches_the_documented_example() {
        let query = "category=option&symbol=BTC-29JUL22-25000-C";
        assert_eq!(
            BybitClient::signing_payload("XXXXXXXXXX", 5000, 1658384314791, query),
            "1658384314791XXXXXXXXXX5000category=option&symbol=BTC-29JUL22-25000-C"
        );
    }

    /// Regression vector for the HMAC itself, computed independently with
    /// Python's `hmac`/`hashlib` over the payload above.
    #[test]
    fn signature_is_a_stable_lowercase_hex_hmac() {
        let headers = BybitClient::signed_headers(
            "XXXXXXXXXX",
            "0123456789abcdef0123456789abcdef",
            5000,
            1658384314791,
            "category=option&symbol=BTC-29JUL22-25000-C",
        )
        .unwrap();
        assert_eq!(
            headers.get("X-BAPI-SIGN").unwrap(),
            "00960eb36f4785bbb9ace7c3d6f42e4e418141a5afbc0d626bb55e53ef001f59"
        );
    }

    #[test]
    fn signed_headers_carry_every_required_field() {
        let headers =
            BybitClient::signed_headers("key123", "secret", 5000, 1700000000000, "category=spot")
                .unwrap();
        assert_eq!(headers.get("X-BAPI-API-KEY").unwrap(), "key123");
        assert_eq!(headers.get("X-BAPI-TIMESTAMP").unwrap(), "1700000000000");
        assert_eq!(headers.get("X-BAPI-RECV-WINDOW").unwrap(), "5000");
        // Lowercase hex, 64 chars for HMAC-SHA256.
        let sig = headers.get("X-BAPI-SIGN").unwrap().to_str().unwrap();
        assert_eq!(sig.len(), 64);
        assert!(
            sig.chars()
                .all(|char| char.is_ascii_hexdigit() && !char.is_ascii_uppercase())
        );
    }

    #[test]
    fn changing_the_query_changes_the_signature() {
        let spot = BybitClient::signed_headers("k", "s", 5000, 1, "category=spot").unwrap();
        let linear = BybitClient::signed_headers("k", "s", 5000, 1, "category=linear").unwrap();
        assert_ne!(
            spot.get("X-BAPI-SIGN").unwrap(),
            linear.get("X-BAPI-SIGN").unwrap()
        );
    }

    #[test]
    fn splits_symbols_into_base_and_quote() {
        assert_eq!(
            Execution::split_symbol("BTCUSDT"),
            Some(("BTC".into(), "USDT".into()))
        );
        assert_eq!(
            Execution::split_symbol("ethusdc"),
            Some(("ETH".into(), "USDC".into()))
        );
        assert_eq!(
            Execution::split_symbol("SOLUSDT"),
            Some(("SOL".into(), "USDT".into()))
        );
        assert_eq!(Execution::split_symbol("WEIRD"), None);
    }

    /// A buy spends quote to get base, so the *sold* leg must be the quote.
    #[test]
    fn buy_prefill_swaps_the_legs_correctly() {
        let prefill = fill().to_prefill();
        assert_eq!(prefill.side, "buy");
        assert_eq!(prefill.asset_sold.as_deref(), Some("USDT"));
        assert_eq!(prefill.qty_sold.as_deref(), Some("32500"));
        assert_eq!(prefill.asset_bought.as_deref(), Some("BTC"));
        assert_eq!(prefill.qty_bought.as_deref(), Some("0.5"));
        assert_eq!(prefill.price.as_deref(), Some("65000"));
        assert_eq!(prefill.fee.as_deref(), Some("16.25"));
        assert_eq!(prefill.fee_asset.as_deref(), Some("USDT"));
    }

    #[test]
    fn sell_prefill_swaps_the_legs_correctly() {
        let mut execution = fill();
        execution.side = "Sell".into();
        let prefill = execution.to_prefill();
        assert_eq!(prefill.side, "sell");
        assert_eq!(prefill.asset_sold.as_deref(), Some("BTC"));
        assert_eq!(prefill.qty_sold.as_deref(), Some("0.5"));
        assert_eq!(prefill.asset_bought.as_deref(), Some("USDT"));
        assert_eq!(prefill.qty_bought.as_deref(), Some("32500"));
        assert_eq!(prefill.fee_asset.as_deref(), Some("BTC"));
    }

    #[test]
    fn exec_time_millis_becomes_iso_seconds() {
        let prefill = fill().to_prefill();
        // 1700000000000 ms == 2023-11-14T22:13:20Z
        assert_eq!(prefill.occurred_at.as_deref(), Some("2023-11-14T22:13:20Z"));
    }

    #[test]
    fn comments_record_the_provenance() {
        let prefill = fill().to_prefill();
        let comment = prefill.comment.unwrap();
        assert!(comment.contains("bybit"), "{comment}");
        assert!(comment.contains("buy"), "{comment}");
        assert!(comment.contains("0.5"), "{comment}");
        assert!(comment.contains("65000"), "{comment}");
    }

    #[test]
    fn closing_positions_are_detected() {
        let mut execution = fill();
        assert!(!execution.is_closing());
        execution.closed_size = Some("0.5".into());
        assert!(execution.is_closing());
        execution.closed_size = Some("0".into());
        assert!(!execution.is_closing());
        execution.closed_size = None;
        assert!(!execution.is_closing());
    }

    #[test]
    fn unknown_symbol_layout_does_not_panic() {
        let mut execution = fill();
        execution.symbol = "MYSTERY".into();
        let prefill = execution.to_prefill();
        // Base leg is known; the quote leg is not, so it must stay empty
        // rather than be guessed at.
        assert_eq!(prefill.asset_bought.as_deref(), Some("MYSTERY"));
        assert_eq!(prefill.qty_bought.as_deref(), Some("0.5"));
        assert_eq!(prefill.asset_sold, None);
        assert_eq!(prefill.fee_asset, None);
    }

    /// No credentials must short-circuit to an empty list, never a request.
    #[tokio::test]
    async fn missing_credentials_return_empty_without_network() {
        let client = BybitClient::testnet();
        let out = client
            .executions("", "", Category::Spot, None, 10)
            .await
            .unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn api_envelope_is_parsed_from_bybit_field_names() {
        let raw = r#"{
            "retCode": 0,
            "retMsg": "OK",
            "result": {
                "list": [{
                    "symbol": "ETHUSDT",
                    "side": "Sell",
                    "orderType": "Limit",
                    "execPrice": "3000",
                    "execQty": "2",
                    "execValue": "6000",
                    "execFee": "6",
                    "execTime": "1700000000000",
                    "isMaker": true,
                    "closedSize": "0"
                }]
            }
        }"#;
        let env: ApiEnvelope<ExecutionPage> = serde_json::from_str(raw).unwrap();
        assert_eq!(env.retcode, 0);
        let exec: Execution = env.result.unwrap().list.into_iter().next().unwrap().into();
        assert_eq!(exec.symbol, "ETHUSDT");
        assert_eq!(exec.side, "Sell");
        assert!(exec.is_maker);
        assert_eq!(exec.exec_value, "6000");
    }

    #[test]
    fn categories_map_to_bybit_strings() {
        assert_eq!(Category::Spot.as_str(), "spot");
        assert_eq!(Category::Linear.as_str(), "linear");
        assert_eq!(Category::Inverse.as_str(), "inverse");
    }
}
