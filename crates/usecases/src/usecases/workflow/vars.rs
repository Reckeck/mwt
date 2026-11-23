//! The workflow lookups that read balances through the portfolio area (spec
//! core-strangler, Z6): the balance of an account and the live accounts.

use crate::Amount;
use crate::model::store::AssetRef;
use crate::money::books::Books;
use crate::workflow::types::WfValue;

/// The balance of an account with its sub-accounts: of one asset, or the
/// whole value in the base currency.
pub fn balance(
    books: &Books<'_>,
    store_id: &str,
    asset: Option<&AssetRef>,
) -> std::result::Result<WfValue, String> {
    let doc = books.doc();
    if !doc.stores.iter().any(|store| store.id == store_id) {
        return Err(format!("account {store_id} not found"));
    }
    match asset {
        Some(asset) => {
            let key = asset.key();
            let mut ids = crate::money::tree::subtree_ids(doc, store_id);
            ids.push(store_id.to_string());
            let quantity = doc
                .holdings
                .iter()
                .filter(|holding| ids.contains(&holding.store_id) && holding.asset.key() == key)
                .fold(Amount::ZERO, |sum, holding| sum + holding.quantity);
            Ok(WfValue::money(quantity, &asset.symbol))
        }
        None => {
            let overview = crate::money::portfolio::store_overview(books, store_id, true)
                .map_err(|err| err.to_string())?;
            Ok(WfValue::money(
                overview.subtree_base.round(2),
                &overview.base_currency,
            ))
        }
    }
}

/// Live accounts under a wallet, or every live wallet: id, name and value in
/// the base currency.
pub fn accounts(books: &Books<'_>, parent: Option<&str>) -> Vec<(String, String, Amount)> {
    books
        .doc()
        .stores
        .iter()
        .filter(|store| !store.archived && store.parent_id.as_deref() == parent)
        .map(|store| {
            let value = crate::money::portfolio::store_overview(books, &store.id, true)
                .map_or(Amount::ZERO, |overview| overview.subtree_base.round(2));
            (store.id.clone(), store.name.clone(), value)
        })
        .collect()
}

/// Price of one unit of an asset in a currency.
pub fn price(books: &Books<'_>, asset: &AssetRef, currency: &str, secs: i64) -> Option<Amount> {
    let book = books.rate_book();
    let usd = book.price_at(&asset.key(), secs)?;
    book.convert(usd, "usd", currency, secs)
}

pub fn convert(
    books: &Books<'_>,
    amount: Amount,
    from: &str,
    to: &str,
    secs: i64,
) -> Option<Amount> {
    books.rate_book().convert(amount, from, to, secs)
}
