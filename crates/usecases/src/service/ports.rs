//! Glue between the ports of the crypto module and the network adapters: an impl of a trait of
//! one layer for a type of another sits here, in the facade that sees both (as newtypes: the ports
//! live in `mwt-cryptotrack`, the clients in `mwt-net`).

use chrono::NaiveDate;
use mwt_net::{BybitClient, RpcCall, RpcClient};
use serde_json::Value;
use zeroize::Zeroizing;

use crate::cryptotrack::ports::{ChainCall, ChainSource, PriceSource};
use crate::cryptotrack::sources::bybit::{Fills, FillsPage, PageRequest};
use crate::error::Result;
use crate::model::bybit::{EarnYield, InterTransfer};
use crate::model::cryptotrack::{CryptoAsset, CryptoPricePoint};
use crate::pricing::PricingService;

/// The shared rate-limited client as the production [`ChainSource`]. A newtype, because the port
/// lives in `mwt-cryptotrack` and the client in `mwt-net`: neither crate may name the other, and
/// the orphan rule forbids the impl for the bare type.
#[derive(Debug, Clone)]
pub struct RpcChain(RpcClient);

impl RpcChain {
    pub fn new(client: RpcClient) -> Self {
        Self(client)
    }

    /// A copy that sends `key` (the Blockscout key) in a header; see [`RpcClient::with_api_key`].
    #[must_use]
    pub fn with_api_key(&self, key: Option<&str>) -> Self {
        Self(self.0.with_api_key(key))
    }
}

impl ChainSource for RpcChain {
    async fn get_json(&self, base: &str, path_and_query: &str, subject: &str) -> Result<Value> {
        self.0.get_json(base, path_and_query, subject).await
    }

    async fn call(&self, url: &str, subject: &str, method: &str, params: Value) -> Result<Value> {
        self.0.call(url, subject, method, params).await
    }

    async fn batch(
        &self,
        url: &str,
        subject: &str,
        calls: &[ChainCall],
    ) -> Result<Vec<Result<Value>>> {
        let calls: Vec<RpcCall> = calls
            .iter()
            .map(|call| RpcCall::new(&call.method, call.params.clone()))
            .collect();
        self.0.batch(url, subject, &calls).await
    }
}

/// The DeFiLlama adapter: a borrowed newtype over [`PricingService`] (the port lives in
/// `mwt-cryptotrack`, the service in `mwt-net`), so `pricing` does not depend on the port.
#[derive(Debug, Clone, Copy)]
pub struct LlamaPrices<'pricing>(pub &'pricing PricingService);

impl PriceSource for LlamaPrices<'_> {
    fn may_lookup_contract(&self) -> bool {
        self.0.looks_up_contracts()
    }

    async fn historic_prices(
        &self,
        wanted: &[(CryptoAsset, NaiveDate)],
    ) -> Result<Vec<CryptoPricePoint>> {
        self.0.fetch_historic_prices(wanted).await
    }
}

/// The real transport: signed read-only GETs against Bybit V5. The key and
/// the secret are held zeroizing and are in no `Debug` and in no log.
pub struct BybitApi {
    client: BybitClient,
    api_key: Zeroizing<String>,
    api_secret: Zeroizing<String>,
}

impl BybitApi {
    pub fn new(client: BybitClient, api_key: &str, api_secret: &str) -> Self {
        Self {
            client,
            api_key: Zeroizing::new(api_key.to_owned()),
            api_secret: Zeroizing::new(api_secret.to_owned()),
        }
    }
}

impl std::fmt::Debug for BybitApi {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.debug_struct("BybitApi")
            .field("api_key", &"<redacted>")
            .field("api_secret", &"<redacted>")
            .finish()
    }
}

impl Fills for BybitApi {
    async fn page(&self, request: PageRequest<'_>) -> Result<FillsPage> {
        let page = self
            .client
            .executions_page(
                &self.api_key,
                &self.api_secret,
                request.product,
                request.start_ms,
                request.end_ms,
                request.page,
                request.limit,
            )
            .await?;
        Ok(FillsPage {
            fills: page.fills,
            next_page: page.next_page,
        })
    }

    async fn transfers(&self, limit: usize) -> Result<Vec<InterTransfer>> {
        self.client
            .inter_transfers(&self.api_key, &self.api_secret, limit)
            .await
    }

    async fn payouts(&self, limit: usize) -> Result<Vec<EarnYield>> {
        self.client
            .earn_yields(&self.api_key, &self.api_secret, limit)
            .await
    }
}

#[cfg(test)]
#[path = "ports_tests.rs"]
mod tests;
