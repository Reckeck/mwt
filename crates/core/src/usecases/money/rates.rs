//! Scenarios of the rates area: the ones that read a request's rates for the
//! wire (`ipc_contract` types) or write the vault's own rate caches. Nothing
//! here persists: the refresh that fetched the rates writes once at the end.

use crate::Amount;
use crate::error::{Error, Result};
use crate::ipc_contract::{CachedPrice, TransactionInput};
use crate::model::store::AssetRef;
use crate::model::vault::{FxPoint, PricePoint};
use crate::money::rates::{fx_history_filter, iso_of, stamp_prices};
use crate::pricing::history;
use crate::service::UnlockedVault;
use crate::time::secs_or_now;

/// The cached USD quote of an asset nearest `ts` (RFC3339; empty = now),
/// with the moment that rate was taken. Lives here with its contract type
/// (`money` may not use `ipc_contract`).
pub fn cached_quote(vault: &UnlockedVault, asset: &AssetRef, ts: &str) -> CachedPrice {
    let secs = secs_or_now(ts);
    match vault.books().rate_book().quote_at(&asset.key(), secs) {
        Some((price_usd, taken)) => CachedPrice {
            price_usd,
            ts: iso_of(taken.unwrap_or(secs)),
        },
        None => CachedPrice::default(),
    }
}

/// Turn a price typed in any currency into the USD price the ledger keeps,
/// at the rate of the transaction's moment (the last one at or before it,
/// else the first after). A price in the base currency is also kept as is.
///
/// Fails with `FX_MISSING` when the currency has no rate at all: guessing 1
/// would silently store rubles as dollars.
pub fn resolve_input_price(vault: &UnlockedVault, input: &mut TransactionInput) -> Result<()> {
    let common = input.common_mut();
    let Some(price) = common.price.take() else {
        return Ok(());
    };
    let books = vault.books();
    let base = books.base_and_fx().0.to_string();
    let currency = common
        .price_currency
        .as_deref()
        .map(|code| code.trim().to_lowercase())
        .filter(|code| !code.is_empty())
        .unwrap_or_else(|| base.clone());
    let secs = secs_or_now(common.timestamp.as_deref().unwrap_or_default());
    let rate = books
        .rate_book()
        .fx_at(&currency, secs)
        .filter(|rate: &Amount| rate.is_positive())
        .ok_or_else(|| Error::FxMissing(currency.clone()))?;
    common.price_usd = Some(price / rate);
    if currency == base {
        common.price_base = Some(price);
        common.price_base_currency = Some(base);
    }
    Ok(())
}

/// Add a fetched FX table to the cache. Does not persist.
pub fn apply_fx(vault: &mut UnlockedVault, fresh: &[FxPoint]) {
    let keep_history = fx_history_filter(&vault.books());
    history::merge_fx(&mut vault.doc.fx_cache, fresh, keep_history);
}

/// Thin both rate caches by the refresh rules, adding nothing.
pub fn thin_rate_caches(vault: &mut UnlockedVault) {
    let keep_history = fx_history_filter(&vault.books());
    history::thin_fx(&mut vault.doc.fx_cache, keep_history);
    history::thin_prices(&mut vault.doc.price_cache);
}

/// Add fetched quotes to the cache. Does not persist.
pub fn apply_prices(vault: &mut UnlockedVault, fresh: &[PricePoint]) {
    history::merge_prices(&mut vault.doc.price_cache, fresh);
}

/// Stamp what was saved without a price — imported, automated, or
/// entered before the rate was cached. Run after a refresh; does not
/// persist. Returns how many sides got a price.
pub fn restamp_missing(vault: &mut UnlockedVault) -> usize {
    let book = vault.rate_book();
    let mut stamped = 0;
    for tx in &mut vault.doc.transactions {
        stamped += stamp_prices(&book, tx);
    }
    if stamped > 0 {
        vault.recompute_holdings();
    }
    stamped
}
