//! One property at a glance: what it is worth, what is owed on it and what
//! it costs a month, all in the base currency.

use crate::Amount;
use crate::error::{Error, Result};
use crate::kernel::books::Books;
use crate::model::planning::UtilityItem;
use crate::model::store::{PropertyCategory, Store, StoreKind};
use crate::money::tree::find_store;

use crate::household::property_links::property_year;

use super::trend::meter_trend;
use super::types::PropertySummary;

/// The property summary: value, debt, equity and monthly costs, priced at
/// `now` (unix seconds).
pub fn property_summary(
    books: &Books<'_>,
    store_id: &str,
    zone: &str,
    now: i64,
) -> Result<PropertySummary> {
    let doc = books.doc();
    let store = find_store(doc, store_id)
        .filter(|store| store.kind == StoreKind::Property)
        .ok_or_else(|| Error::Missing(format!("property {store_id}")))?;
    let (base, fx) = books.base_and_fx();
    let book = books.rate_book();
    let value_usd = doc
        .holdings
        .iter()
        .filter(|holding| holding.store_id == store.id)
        .fold(Amount::ZERO, |sum, holding| {
            sum + holding.quantity * books.price_usd(&holding.asset.key())
        });
    let loans: Vec<&Store> = doc
        .stores
        .iter()
        .filter(|loan| !loan.archived && loan.secured_by.as_deref() == Some(store_id))
        .collect();
    let debt_usd = loans.iter().fold(Amount::ZERO, |sum, loan| {
        let balance = doc
            .holdings
            .iter()
            .filter(|holding| holding.store_id == loan.id)
            .fold(Amount::ZERO, |acc, holding| {
                acc + holding.quantity * books.price_usd(&holding.asset.key())
            });
        sum - balance.min(Amount::ZERO)
    });
    let to_base = |amount: Amount, currency: &str| {
        book.convert(amount, currency, base, now)
            .unwrap_or(Amount::ZERO)
    };
    let loan_month_base = loans.iter().fold(Amount::ZERO, |sum, loan| {
        let payment = loan
            .liability
            .as_ref()
            .and_then(|terms| terms.monthly_payment)
            .unwrap_or(Amount::ZERO);
        // A loan's currency is what it holds; an empty one is in the base currency.
        let currency = doc
            .holdings
            .iter()
            .find(|holding| holding.store_id == loan.id)
            .map(|holding| holding.asset.symbol.to_lowercase())
            .unwrap_or_else(|| base.to_string());
        sum + to_base(payment, &currency)
    });
    let items: Vec<&UtilityItem> = doc
        .utility_items
        .iter()
        .filter(|item| item.active && item.property_id == store_id)
        .collect();
    let utilities_month_base = items.iter().fold(Amount::ZERO, |sum, item| {
        let monthly = meter_trend(books, &item.id, zone)
            .map(|trend| trend.monthly)
            .unwrap_or(Amount::ZERO);
        sum + to_base(monthly, &item.currency)
    });
    let value_base = value_usd * fx;
    let debt_base = debt_usd * fx;
    let year = property_year(books, store_id, now);
    let twelve = Amount::new(rust_decimal::Decimal::from(12));
    let is_car = store
        .property
        .as_ref()
        .is_some_and(|info| info.category == PropertyCategory::Car);
    Ok(PropertySummary {
        rent_12m_base: year.rent.round(2),
        expenses_12m_base: year.expenses.round(2),
        rent_yield_pct: (value_base.is_positive() && year.rent.is_positive()).then(|| {
            (year.rent / value_base * Amount::new(rust_decimal::Decimal::from(100))).round(2)
        }),
        ownership_month_base: is_car.then(|| ((year.expenses + year.wear) / twelve).round(2)),
        wear_12m_base: year.wear.round(2),
        store_id: store.id.clone(),
        base_currency: base.to_string(),
        value_base,
        debt_base,
        equity_base: value_base - debt_base,
        utilities_month_base,
        loan_month_base,
        monthly_cost_base: utilities_month_base + loan_month_base,
        utility_count: items.len() as u32,
    })
}
