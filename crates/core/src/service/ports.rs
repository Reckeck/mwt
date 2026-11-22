//! Glue between the ports of the crypto module and the network adapters: an impl of a trait of
//! one layer for a type of another sits here, in the facade that sees both.

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

/// The shared rate-limited client is the production [`ChainSource`].
impl ChainSource for RpcClient {
    async fn get_json(&self, base: &str, path_and_query: &str, subject: &str) -> Result<Value> {
        Self::get_json(self, base, path_and_query, subject).await
    }

    async fn call(&self, url: &str, subject: &str, method: &str, params: Value) -> Result<Value> {
        Self::call(self, url, subject, method, params).await
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
        Self::batch(self, url, subject, &calls).await
    }
}

/// The DeFiLlama adapter. It lives here, not in `pricing`, so the port stays in
/// `cryptotrack` and `pricing` (a lower layer) does not depend on it.
impl PriceSource for PricingService {
    fn may_lookup_contract(&self) -> bool {
        self.looks_up_contracts()
    }

    async fn historic_prices(
        &self,
        wanted: &[(CryptoAsset, NaiveDate)],
    ) -> Result<Vec<CryptoPricePoint>> {
        self.fetch_historic_prices(wanted).await
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
