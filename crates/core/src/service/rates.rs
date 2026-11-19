//! Shim of the `rates` area: moved to `crate::money::rates` (spec core-strangler, D1).
//! The scenarios that write the rate caches or speak the wire types live in
//! `usecases::money::rates`.

pub use crate::money::rates::RateBook;
#[cfg(test)]
pub(crate) use crate::money::rates::is_fiat_of;

use std::collections::HashSet;

use crate::Amount;
use crate::ipc_contract::CachedPrice;
use crate::model::store::AssetRef;
use crate::model::transaction::Transaction;
use crate::model::vault::{FxPoint, PricePoint};
use crate::money::rates::ValueCurve;

use super::UnlockedVault;
use super::forward::forward;
use super::valuation::ValuationCurve;

impl UnlockedVault {
    // forward:begin
    forward! {
        /// Parse the caches once for a batch of lookups.
        pub fn rate_book(&self) -> RateBook
            => crate::money::rates::rate_book @books;
        /// Units of `currency` per USD at `ts` (RFC3339; empty = now).
        pub fn fx_at(&self, currency: &str, ts: &str) -> Option<Amount>
            => crate::money::rates::fx_at @books;
        /// USD price of one unit of an asset at `ts` (RFC3339; empty = now).
        pub fn price_at(&self, asset_key: &str, ts: &str) -> Option<Amount>
            => crate::money::rates::price_at @books;
        /// `amount` of one currency in another at `ts` (RFC3339; empty = now).
        pub fn convert_at(&self, amount: Amount, from: &str, to: &str, ts: &str) -> Option<Amount>
            => crate::money::rates::convert_at @books;
        /// Currencies whose rate history matters: the base and budget
        /// currencies, every fiat the vault holds or moved, and the currencies of
        /// hand-set prices. The rest of the table keeps only its latest rate.
        pub fn fx_currencies_in_use(&self) -> HashSet<String>
            => crate::money::rates::fx_currencies_in_use @books;
        /// A code that names a coin rather than a fiat currency: no FX rate is
        /// cached for it, and the vault holds a coin with that ticker or it is a
        /// well-known one. Its rate comes from the price cache, not FX sources.
        pub fn is_crypto_code(&self, code: &str) -> bool
            => crate::money::rates::is_crypto_code @doc;
        /// The asset a crypto favourite is priced as: the identity the vault
        /// already holds, else the well-known coin. `None` for fiat and unknowns.
        pub fn favorite_asset(&self, code: &str) -> Option<AssetRef>
            => crate::money::rates::favorite_asset @doc;
        /// Which of `currencies` have no cached rate taken on `date` (UTC day).
        /// USD never needs one, and coins are priced, not looked up as FX.
        pub fn fx_missing_on(&self, currencies: &[String], date: chrono::NaiveDate) -> Vec<String>
            => crate::money::rates::fx_missing_on @doc;
        /// The cached USD quote of an asset nearest `ts` (RFC3339; empty = now),
        /// with the moment that rate was taken. Lives with its contract type
        /// (`money` may not use `ipc_contract`).
        pub fn cached_quote(&self, asset: &AssetRef, ts: &str) -> CachedPrice
            => crate::usecases::money::rates::cached_quote;
        /// Turn a price typed in any currency into the USD price the ledger keeps,
        /// at the rate of the transaction's moment. Fails with `FX_MISSING` when
        /// the currency has no rate at all.
        pub fn resolve_input_price(&self, input: &mut crate::ipc_contract::TransactionInput) -> crate::Result<()>
            => crate::usecases::money::rates::resolve_input_price;
        /// The held spot coin with this ticker.
        pub(crate) fn held_coin(&self, code: &str) -> Option<&AssetRef>
            => crate::money::rates::held_coin @doc;
        /// Which currencies keep their rate history: the ones in use. Owns its
        /// data, so it can be used while the cache is borrowed mutably.
        pub(crate) fn fx_history_filter(&self) -> impl Fn(&str) -> bool + use<>
            => crate::money::rates::fx_history_filter @books;
        /// Add a fetched FX table to the cache. Does not persist: the refresh
        /// that fetched it writes once at the end.
        pub fn apply_fx(&mut self, fresh: &[FxPoint])
            => crate::usecases::money::rates::apply_fx;
        /// Thin both rate caches by the refresh rules, adding nothing.
        pub(crate) fn thin_rate_caches(&mut self)
            => crate::usecases::money::rates::thin_rate_caches;
        /// Add fetched quotes to the cache. Does not persist.
        pub fn apply_prices(&mut self, fresh: &[PricePoint])
            => crate::usecases::money::rates::apply_prices;
        /// Stamp what was saved without a price — imported, automated, or
        /// entered before the rate was cached. Run after a refresh; does not
        /// persist. Returns how many sides got a price.
        pub fn restamp_missing(&mut self) -> usize
            => crate::usecases::money::rates::restamp_missing;
    }
    // forward:end

    /// Give every unpriced side the USD price of its moment (plan §6.2).
    pub(crate) fn stamp_prices(book: &RateBook, tx: &mut Transaction) -> usize {
        crate::money::rates::stamp_prices(book, tx)
    }
}

/// The valuation curves feed the book through its own trait, so `money::rates`
/// never names the valuation module.
impl ValueCurve for ValuationCurve {
    fn currency(&self) -> &str {
        &self.currency
    }

    fn value_at(&self, secs: i64) -> Option<Amount> {
        ValuationCurve::value_at(self, secs)
    }
}
