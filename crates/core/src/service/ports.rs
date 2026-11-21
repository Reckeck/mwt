//! Glue between the ports of the crypto module and the network adapters: an impl of a trait of
//! one layer for a type of another sits here, in the facade that sees both.

use chrono::NaiveDate;

use crate::cryptotrack::ports::PriceSource;
use crate::error::Result;
use crate::model::cryptotrack::{CryptoAsset, CryptoPricePoint};
use crate::pricing::PricingService;

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
