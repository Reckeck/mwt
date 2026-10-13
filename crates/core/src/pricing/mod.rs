//! Price & FX fetching with provider fallback and an in-vault cache.
//!
//! Provider chain (all free, no keys):
//!   crypto spot:  CoinGecko (keyless) -> CoinCap -> Binance ticker
//!   tokens (ERC-20 etc.): CoinGecko `/simple/token_price/{platform}`
//!   fiat FX:      open.er-api.com (latest); by date: Frankfurter/ECB -> ЦБ РФ,
//!                 walking back up to a week (see [`fx_sources`])
//!
//! Rate limits are mitigated server-side by batching (one call per asset
//! group) and client-side by an aggressive refresh cadence driven from the
//! app layer (debounce + periodic timer).

use std::collections::{HashMap, HashSet};

pub mod fx_sources;
pub mod history;
pub mod key_rate;

use reqwest::Client;
use reqwest::header::{ACCEPT, USER_AGENT};
use rust_decimal::Decimal;

use crate::Amount;
use crate::error::{Error, Result};
use crate::model::chain::Chain;
use crate::model::store::AssetRef;
use crate::model::vault::{FxPoint, PricePoint, VaultDoc};

const COINGECKO_V3: &str = "https://api.coingecko.com/api/v3";
const COINCAP_V2: &str = "https://api.coincap.io/v2";
const BINANCE_API: &str = "https://api.binance.com/api/v3";
const ER_API: &str = "https://open.er-api.com/v6";

#[derive(Debug, Clone)]
pub struct PricingService {
    client: Client,
}

impl Default for PricingService {
    fn default() -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .expect("reqwest client"),
        }
    }
}

impl PricingService {
    /// Network-only price fetch (no cache mutation) — usable across `.await`
    /// without borrowing the vault.
    pub async fn fetch_prices(&self, assets: &[AssetRef]) -> Result<Vec<PricePoint>> {
        if assets.is_empty() {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        let mut seen = HashSet::new();

        for asset in assets {
            if asset.is_fiat() || seen.contains(&asset.key()) {
                continue;
            }
            seen.insert(asset.key());
            let value_usd = self.price_usd(asset).await?;
            out.push(PricePoint {
                asset_key: asset.key(),
                value_usd,
                source: "cryptoprice".into(),
                ts: crate::model::vault::now_iso(),
            });
        }
        Ok(out)
    }

    /// Insert fetched price points into the vault cache, keeping history
    /// (plan §6.1).
    pub fn apply_prices(doc: &mut VaultDoc, fresh: &[PricePoint]) {
        history::merge_prices(&mut doc.price_cache, fresh);
    }

    /// Network-only FX fetch (no cache mutation): the whole table, every
    /// currency the provider knows, as units per 1 USD.
    pub async fn fetch_fx(&self) -> Result<Vec<FxPoint>> {
        let rates = self.fx_table().await?;
        let ts = crate::model::vault::now_iso();
        let mut out: Vec<FxPoint> = rates
            .into_iter()
            .filter(|(_, per_usd)| per_usd.is_positive())
            .map(|(currency, per_usd)| FxPoint {
                currency,
                per_usd,
                ts: ts.clone(),
                source: "open.er-api".into(),
            })
            .collect();
        out.sort_by(|left, right| left.currency.cmp(&right.currency));
        Ok(out)
    }

    /// Search crypto assets for the UI autocomplete (CoinGecko `/search`).
    pub async fn search_assets(&self, query: &str, limit: usize) -> Result<Vec<AssetSearchHit>> {
        let url = format!("{COINGECKO_V3}/search");
        let resp = self
            .client
            .get(url)
            .query(&[("query", query)])
            .header(USER_AGENT, "multi-wallet-tracker")
            .header(ACCEPT, "application/json")
            .send()
            .await
            .map_err(|err| Error::Network(err.to_string()))?;

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|err| Error::Network(err.to_string()))?;

        let mut hits = Vec::new();
        if let Some(coins) = body.get("coins").and_then(|list| list.as_array()) {
            for coin in coins.iter().take(limit) {
                let id = coin["id"].as_str().unwrap_or_default();
                let symbol = coin["symbol"].as_str().unwrap_or_default().to_uppercase();
                let name = coin["name"].as_str().unwrap_or_default();
                let platform = coin
                    .pointer("/market_cap_rank")
                    .and_then(|rank| rank.as_u64());
                if !id.is_empty() {
                    hits.push(AssetSearchHit {
                        symbol,
                        name: name.to_string(),
                        coingecko_id: Some(id.to_string()),
                        chain: None,
                        contract: None,
                        rank: platform.unwrap_or(0),
                    });
                }
            }
        }
        hits.sort_by_key(|hit| hit.rank);
        Ok(hits)
    }

    // -- provider chain ----------------------------------------------------

    /// Every FX rate the public provider knows, keyed by lowercase currency
    /// code (units per 1 USD).
    ///
    /// One call returns the whole table, so this is also how the settings
    /// pickers learn which fiat currencies exist at all.
    pub async fn fx_rates(&self) -> Result<HashMap<String, Amount>> {
        self.fx_table().await
    }

    /// Every coin symbol CoinGecko lists, lowercased.
    ///
    /// Backs the crypto half of the currency catalog. A failure here is not
    /// fatal for callers: the fiat half and the built-in fallback still stand.
    pub async fn coin_symbols(&self) -> Result<Vec<String>> {
        let url = format!("{COINGECKO_V3}/coins/list");
        let resp = self
            .client
            .get(url)
            .header(USER_AGENT, "multi-wallet-tracker")
            .header(ACCEPT, "application/json")
            .send()
            .await
            .map_err(|err| Error::Network(err.to_string()))?;
        if !resp.status().is_success() {
            return Err(Error::Pricing(format!(
                "coingecko http {:?}",
                resp.status()
            )));
        }
        let coins: Vec<CoinListEntry> = resp
            .json()
            .await
            .map_err(|err| Error::Network(err.to_string()))?;
        let mut out: Vec<String> = coins
            .into_iter()
            .filter_map(|coin| {
                let symbol = coin.symbol.trim().to_lowercase();
                (!symbol.is_empty()).then_some(symbol)
            })
            .collect();
        out.sort();
        out.dedup();
        Ok(out)
    }

    async fn price_usd(&self, asset: &AssetRef) -> Result<Amount> {
        // 1) CoinGecko by id
        if let Some(id) = &asset.coingecko_id
            && let Ok(prices) = self
                .coingecko_id_prices(&[(id.clone(), &asset.symbol)])
                .await
            && prices.get(id).is_some_and(|price| !price.is_zero())
        {
            return Ok(prices[id]);
        }
        // 2) CoinGecko token by contract
        if let (Some(chain), Some(contract)) = (asset.chain, &asset.contract)
            && let Ok(price) = self
                .coingecko_token_price(chain, contract, &asset.symbol)
                .await
        {
            return Ok(price);
        }
        // 3) CoinCap search
        if let Ok(price) = self.coincap_symbol(asset).await {
            return Ok(price);
        }
        // 4) Binance ticker
        self.binance_ticker(&asset.symbol).await
    }

    async fn coingecko_id_prices(
        &self,
        ids_symbols: &[(String, &str)],
    ) -> Result<HashMap<String, Amount>> {
        let ids: Vec<&str> = ids_symbols.iter().map(|(id, _)| id.as_str()).collect();
        let url = format!("{COINGECKO_V3}/simple/price");
        let resp = self
            .client
            .get(&url)
            .query(&[
                ("ids", ids.join(",")),
                ("vs_currencies", "usd".into()),
                ("include_24hr_change", "true".into()),
            ])
            .header(USER_AGENT, "multi-wallet-tracker")
            .header(ACCEPT, "application/json")
            .send()
            .await
            .map_err(|err| Error::Network(err.to_string()))?;
        if !resp.status().is_success() {
            return Err(Error::Pricing(format!(
                "coingecko http {:?}",
                resp.status()
            )));
        }
        let body: HashMap<String, serde_json::Value> = resp
            .json()
            .await
            .map_err(|err| Error::Network(err.to_string()))?;
        let mut out = HashMap::new();
        for (id, quote) in body {
            if let Some(price) = quote.get("usd").and_then(|value| value.as_f64()) {
                out.insert(id, Amount::from_f64(price));
            }
        }
        Ok(out)
    }

    async fn coingecko_token_price(
        &self,
        chain: Chain,
        contract: &str,
        symbol: &str,
    ) -> Result<Amount> {
        let platform = chain
            .info()
            .cg_platform
            .ok_or_else(|| Error::Pricing("chain has no coingecko platform".into()))?;
        let url = format!("{COINGECKO_V3}/simple/token_price/{platform}");
        let resp = self
            .client
            .get(&url)
            .query(&[("contract_addresses", contract), ("vs_currencies", "usd")])
            .header(USER_AGENT, "multi-wallet-tracker")
            .header(ACCEPT, "application/json")
            .send()
            .await
            .map_err(|err| Error::Network(err.to_string()))?;
        if !resp.status().is_success() {
            return Err(Error::Pricing(format!(
                "token price http {:?}",
                resp.status()
            )));
        }
        let body: HashMap<String, HashMap<String, f64>> = resp
            .json()
            .await
            .map_err(|err| Error::Network(err.to_string()))?;
        body.get(contract)
            .and_then(|quote| quote.get("usd"))
            .copied()
            .ok_or_else(|| Error::Pricing(format!("no price for {symbol} token")))
            .map(Amount::from_f64)
    }

    async fn coincap_symbol(&self, asset: &AssetRef) -> Result<Amount> {
        let url = format!("{COINCAP_V2}/assets");
        let resp = self
            .client
            .get(&url)
            .query(&[("search", asset.symbol.to_ascii_lowercase())])
            .header(USER_AGENT, "multi-wallet-tracker")
            .header(ACCEPT, "application/json")
            .send()
            .await
            .map_err(|err| Error::Network(err.to_string()))?;
        if !resp.status().is_success() {
            return Err(Error::Pricing(format!("coincap http {:?}", resp.status())));
        }
        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|err| Error::Network(err.to_string()))?;
        let arr = body
            .get("data")
            .and_then(|data| data.as_array())
            .cloned()
            .unwrap_or_default();
        for item in arr {
            let sym = item["symbol"].as_str().unwrap_or_default().to_uppercase();
            if sym == asset.symbol
                && let Some(raw) = item["priceUsd"]
                    .as_str()
                    .and_then(|text| Decimal::from_str_exact(text).ok())
            {
                return Ok(Amount(raw));
            }
        }
        Err(Error::Pricing(format!(
            "no coincap price for {}",
            asset.symbol
        )))
    }

    async fn binance_ticker(&self, symbol: &str) -> Result<Amount> {
        let pair = format!("{symbol}USDT");
        let url = format!("{BINANCE_API}/ticker/price");
        let resp = self
            .client
            .get(&url)
            .query(&[("symbol", pair)])
            .header(USER_AGENT, "multi-wallet-tracker")
            .header(ACCEPT, "application/json")
            .send()
            .await
            .map_err(|err| Error::Network(err.to_string()))?;
        if !resp.status().is_success() {
            return Err(Error::Pricing(format!("binance http {:?}", resp.status())));
        }
        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|err| Error::Network(err.to_string()))?;
        body.get("price")
            .and_then(|price| price.as_str())
            .and_then(|text| Decimal::from_str_exact(text).ok())
            .map(Amount)
            .ok_or_else(|| Error::Pricing(format!("no binance price for {symbol}")))
    }

    async fn fx_table(&self) -> Result<HashMap<String, Amount>> {
        let url = format!("{ER_API}/latest/USD");
        let resp = self
            .client
            .get(&url)
            .header(USER_AGENT, "multi-wallet-tracker")
            .header(ACCEPT, "application/json")
            .send()
            .await
            .map_err(|err| Error::Network(err.to_string()))?;
        if !resp.status().is_success() {
            return Err(Error::Pricing(format!("er-api http {:?}", resp.status())));
        }
        let body: HashMap<String, serde_json::Value> = resp
            .json()
            .await
            .map_err(|err| Error::Network(err.to_string()))?;
        let rates = body
            .get("rates")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let pairs = match rates {
            serde_json::Value::Object(map) => map,
            _ => return Err(Error::Pricing("er-api: no rates".into())),
        };
        let mut out = HashMap::new();
        for (ccy, rate) in pairs {
            if let Some(rate) = rate.as_f64() {
                out.insert(ccy.to_lowercase(), Amount::from_f64(rate));
            }
        }
        Ok(out)
    }
}

#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct AssetSearchHit {
    pub symbol: String,
    pub name: String,
    pub coingecko_id: Option<String>,
    pub chain: Option<Chain>,
    pub contract: Option<String>,
    /// CoinGecko market-cap rank (0 = unknown).
    pub rank: u64,
}

/// One row of CoinGecko `/coins/list`; only the symbol is of interest here.
#[derive(serde::Deserialize)]
struct CoinListEntry {
    symbol: String,
}
