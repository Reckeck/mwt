//! The capital scenarios: today's net worth assembled from the holdings, the
//! baskets and the contact debts, and the hand-set prices of property. The
//! figures and rules come from `money::worth`; writes end in one
//! `vault.transact`.

use crate::Amount;
use crate::error::Result;
use crate::model::vault::{ManualPrice, now_iso};
use crate::money::books::Books;
use crate::money::portfolio::PortfolioSummary;
use crate::money::worth::{NetWorth, plan_manual_price};
use crate::reports::net_worth_now::net_worth_with_crypto;
use crate::service::Session;

/// USD value of one unit of a hand-priced asset, read at the present moment.
#[cfg(test)]
pub(crate) fn manual_price_usd(books: &Books<'_>, asset_key: &str) -> Option<Amount> {
    crate::money::worth::manual_price_usd(books, asset_key, crate::time::secs_or_now(""))
}

/// Set the value of one unit of a property asset.
pub fn set_manual_price(
    vault: &mut Session,
    asset_key: &str,
    unit_price: Amount,
    currency: &str,
) -> Result<ManualPrice> {
    let price = plan_manual_price(&vault.doc, asset_key, unit_price, currency, now_iso())?;
    vault.transact(|tx| {
        let prices = &mut tx.doc_mut().manual_prices;
        prices.retain(|existing| existing.asset_key != asset_key);
        prices.push(price.clone());
        Ok(price)
    })
}

pub fn clear_manual_price(vault: &mut Session, asset_key: &str) -> Result<()> {
    vault.transact(|tx| {
        tx.doc_mut()
            .manual_prices
            .retain(|existing| existing.asset_key != asset_key);
        Ok(())
    })
}

/// The portfolio summary with today's net worth worked out for it.
pub fn portfolio_summary(books: &Books<'_>) -> PortfolioSummary {
    crate::money::portfolio::portfolio_summary(books, &net_worth(books))
}

pub fn net_worth(books: &Books<'_>) -> NetWorth {
    net_worth_with_crypto(books).0
}
