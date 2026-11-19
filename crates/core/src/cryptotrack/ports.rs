//! The ports of the crypto module (core-strangler T46, spec target-structure
//! §4): what the module needs from the outside, as traits, so the engine and
//! the scenarios run against fakes with no network.
//!
//! * [`ChainSource`]: a node or an explorer, read for one *subject* (an
//!   address); the real one is [`crate::cryptotrack::rpc::RpcClient`].
//! * [`PriceSource`]: historic prices of assets, with the consent policy of
//!   token contracts (`N1`); the real one is
//!   [`crate::pricing::PricingService`].
//! * [`ExchangeClient`]: the read-only API of an exchange account; every
//!   transport of [`crate::cryptotrack::sources::bybit::Fills`] is one.
//!
//! Like every fetch these are pure in the sense of `AGENTS.md`, rule 4: they
//! read and hand back, they never touch the document. The futures are `Send`
//! because a command fetches with the vault lock released.

use std::future::Future;

use chrono::NaiveDate;
use serde_json::Value;

use crate::cryptotrack::sources::bybit::{Fills, FillsPage, PageRequest};
use crate::cryptotrack::tokens::known;
use crate::error::Result;
use crate::exchange::{EarnYield, InterTransfer};
use crate::model::cryptotrack::{CryptoAsset, CryptoPricePoint};
use crate::pricing::PricingService;

/// A chain node or explorer.
pub trait ChainSource: Send + Sync {
    /// GET `{base}{path_and_query}` for `subject` and parse the body as JSON.
    fn get_json(
        &self,
        base: &str,
        path_and_query: &str,
        subject: &str,
    ) -> impl Future<Output = Result<Value>> + Send;

    /// One JSON-RPC call for `subject`; the `result` field.
    fn call(
        &self,
        url: &str,
        subject: &str,
        method: &str,
        params: Value,
    ) -> impl Future<Output = Result<Value>> + Send;
}

/// A provider of historic prices.
pub trait PriceSource: Send + Sync {
    /// Whether a token *contract* may be sent to the provider (`N1`). A
    /// contract names what the user holds, and the provider is a third party:
    /// with `false`, an unknown token stays unpriced and no request names it.
    /// Natively priced coins do not depend on it.
    fn may_lookup_contract(&self) -> bool;

    /// Historic USD prices for `(asset, date)` pairs. What the provider does
    /// not have, or what the policy withholds, is missing from the answer,
    /// never zero.
    fn historic_prices(
        &self,
        wanted: &[(CryptoAsset, NaiveDate)],
    ) -> impl Future<Output = Result<Vec<CryptoPricePoint>>> + Send;
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
        self.fetch_historic_prices(known::is_known, wanted).await
    }
}

/// The read-only API of one exchange account.
pub trait ExchangeClient: Send + Sync {
    /// One page of fills of a window.
    fn fills_page(
        &self,
        request: PageRequest<'_>,
    ) -> impl Future<Output = Result<FillsPage>> + Send;

    /// Moves between the user's own exchange accounts, newest first.
    fn transfers(&self, limit: usize) -> impl Future<Output = Result<Vec<InterTransfer>>> + Send;

    /// Earn payouts, newest first.
    fn payouts(&self, limit: usize) -> impl Future<Output = Result<Vec<EarnYield>>> + Send;
}

impl<T: Fills + Send + Sync> ExchangeClient for T {
    fn fills_page(
        &self,
        request: PageRequest<'_>,
    ) -> impl Future<Output = Result<FillsPage>> + Send {
        self.page(request)
    }

    fn transfers(&self, limit: usize) -> impl Future<Output = Result<Vec<InterTransfer>>> + Send {
        Fills::transfers(self, limit)
    }

    fn payouts(&self, limit: usize) -> impl Future<Output = Result<Vec<EarnYield>>> + Send {
        Fills::payouts(self, limit)
    }
}

#[cfg(test)]
#[path = "ports_tests.rs"]
mod tests;
