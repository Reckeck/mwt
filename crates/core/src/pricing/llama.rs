//! Historic token prices from DeFiLlama (`coins.llama.fi`), keyed by
//! `(chain, contract, date)` (spec E4).
//!
//! The request names a coin and a moment, nothing else: the wallet address
//! the token was found on is not an input of any function here, so it cannot
//! reach a URL. Like every `fetch_*` this does not touch the document; the
//! answer is applied by [`crate::cryptotrack::prices::apply_historic`].

use std::collections::BTreeMap;

use chrono::NaiveDate;
use reqwest::header::{ACCEPT, USER_AGENT};
use rust_decimal::Decimal;

use super::PricingService;
use crate::Amount;
use crate::cryptotrack::prices::{date_secs, date_ts};
use crate::error::{Error, Result};
use crate::model::chain::Chain;
use crate::model::cryptotrack::{CryptoAsset, CryptoPricePoint};

pub const DEFILLAMA_COINS: &str = "https://coins.llama.fi";

/// The `source` of a point this module produced.
pub const SOURCE: &str = "defillama";

/// DeFiLlama's name for a network; `None` for one it does not price by contract.
fn llama_chain(chain: Chain) -> Option<&'static str> {
    Some(match chain {
        Chain::Ethereum => "ethereum",
        Chain::Bsc => "bsc",
        Chain::Polygon => "polygon",
        Chain::Arbitrum => "arbitrum",
        Chain::Optimism => "optimism",
        Chain::Avalanche => "avax",
        Chain::Base => "base",
        Chain::Solana => "solana",
        Chain::Tron => "tron",
        _ => return None,
    })
}

/// The coin identifier of a request: `<chain>:<contract>` for a token,
/// `coingecko:<id>` for a network's own coin. `None` when the asset has no
/// identifier DeFiLlama understands.
pub fn llama_coin(asset: &CryptoAsset) -> Option<String> {
    match asset {
        CryptoAsset::Native { chain } => chain
            .info()
            .cg_coin_id
            .map(|coin_id| format!("coingecko:{coin_id}")),
        CryptoAsset::Token(token) => {
            let chain = llama_chain(token.chain)?;
            (!token.contract.is_empty()).then(|| format!("{chain}:{}", token.contract))
        }
    }
}

/// `…/prices/historical/<unix>/<coin>[,<coin>…]`.
pub fn historical_url(base: &str, secs: i64, coins: &[String]) -> String {
    format!("{base}/prices/historical/{secs}/{}", coins.join(","))
}

/// A DeFiLlama price as an exact decimal; zero and junk are "no price".
fn parse_price(value: &serde_json::Value) -> Option<Amount> {
    let text = value.as_f64().map(|_| value.to_string())?;
    let parsed = Decimal::from_str_exact(&text)
        .or_else(|_| Decimal::from_scientific(&text))
        .ok()?;
    parsed
        .is_sign_positive()
        .then_some(Amount(parsed))
        .filter(|price| !price.is_zero())
}

/// Reads `{"coins": {"<coin>": {"price": …}}}` into points of `requested`
/// (coin identifier → token key); what the answer lacks stays missing.
pub fn parse_historical(
    body: &serde_json::Value,
    requested: &BTreeMap<String, String>,
    date: NaiveDate,
) -> Vec<CryptoPricePoint> {
    let Some(coins) = body.get("coins").and_then(|coins| coins.as_object()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (coin, quote) in coins {
        // The provider may answer an address in another case.
        let Some(token_key) = requested
            .iter()
            .find(|(asked, _)| asked.eq_ignore_ascii_case(coin))
            .map(|(_, key)| key)
        else {
            continue;
        };
        if let Some(value_usd) = quote.get("price").and_then(parse_price) {
            out.push(CryptoPricePoint {
                token_key: token_key.clone(),
                value_usd,
                ts: date_ts(date),
                source: SOURCE.into(),
            });
        }
    }
    out
}

impl PricingService {
    /// Historic USD prices for `(asset, date)` pairs, one request per date.
    /// Pure: nothing is written to the document. A price DeFiLlama does not
    /// have is missing from the result, not zero.
    pub async fn fetch_historic_prices(
        &self,
        wanted: &[(CryptoAsset, NaiveDate)],
    ) -> Result<Vec<CryptoPricePoint>> {
        let mut by_date: BTreeMap<NaiveDate, BTreeMap<String, String>> = BTreeMap::new();
        for (asset, date) in wanted {
            if let Some(coin) = llama_coin(asset) {
                by_date.entry(*date).or_default().insert(coin, asset.key());
            }
        }
        let mut out = Vec::new();
        let mut last_err = None;
        for (date, requested) in by_date {
            let coins: Vec<String> = requested.keys().cloned().collect();
            let url = historical_url(DEFILLAMA_COINS, date_secs(date), &coins);
            match self.get_json(&url).await {
                Ok(body) => out.extend(parse_historical(&body, &requested, date)),
                Err(err) => last_err = Some(err),
            }
        }
        match last_err {
            Some(err) if out.is_empty() => Err(err),
            _ => Ok(out),
        }
    }

    async fn get_json(&self, url: &str) -> Result<serde_json::Value> {
        let resp = self
            .client
            .get(url)
            .header(USER_AGENT, "multi-wallet-tracker")
            .header(ACCEPT, "application/json")
            .send()
            .await
            // `without_url`: a DefiLlama request names every contract the user
            // holds in its path, and an error must not carry that list on into
            // a message, a panic or a report.
            .map_err(|err| Error::Network(err.without_url().to_string()))?;
        if !resp.status().is_success() {
            return Err(Error::Pricing(format!(
                "defillama http {:?}",
                resp.status()
            )));
        }
        resp.json()
            .await
            .map_err(|err| Error::Network(err.without_url().to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::cryptotrack::TokenId;

    const WALLET: &str = "0x00000000000000000000000000000000deadbeef";
    const USDC: &str = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48";

    fn day() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 3, 1).unwrap()
    }

    #[test]
    fn the_request_url_carries_no_wallet_address() {
        // The wallet holding the token is not an input: the URL is built from
        // the token and the date alone.
        let asset = CryptoAsset::Token(TokenId::new(Chain::Ethereum, USDC));
        let coin = llama_coin(&asset).unwrap();
        let url = historical_url(DEFILLAMA_COINS, date_secs(day()), &[coin]);
        assert_eq!(
            url,
            format!(
                "https://coins.llama.fi/prices/historical/{}/ethereum:{}",
                date_secs(day()),
                USDC.to_lowercase()
            )
        );
        assert!(!url.to_lowercase().contains(&WALLET[2..].to_lowercase()));
    }

    #[test]
    fn native_coins_use_the_gas_coin_id() {
        let asset = CryptoAsset::Native {
            chain: Chain::Arbitrum,
        };
        assert_eq!(llama_coin(&asset).as_deref(), Some("coingecko:ethereum"));
        let unsupported = CryptoAsset::Token(TokenId::new(Chain::Algorand, "31566704"));
        assert_eq!(llama_coin(&unsupported), None);
    }

    #[test]
    fn parses_prices_exactly_and_drops_missing_ones() {
        let asset = CryptoAsset::Token(TokenId::new(Chain::Ethereum, USDC));
        let coin = llama_coin(&asset).unwrap();
        let requested = BTreeMap::from([
            (coin.clone(), asset.key()),
            (
                "ethereum:0xmissing".to_string(),
                "ethereum|0xmissing".to_string(),
            ),
        ]);
        let body = serde_json::json!({"coins": {
            format!("ethereum:{USDC}"): {"price": 0.9998, "timestamp": 1},
        }});
        let points = parse_historical(&body, &requested, day());
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].value_usd, Amount::parse("0.9998").unwrap());
        assert_eq!(points[0].ts, "2026-03-01T00:00:00Z");
        let zero = serde_json::json!({"coins": {coin: {"price": 0}}});
        assert!(parse_historical(&zero, &requested, day()).is_empty());
    }
}
