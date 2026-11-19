//! "Investments" (Money 2 · 13): what was put into investments and what it made,
//! per asset. Coins on crypto and exchange wallets join by asset; a futures
//! sub-account is one row with its margin. Fiat and dollar stablecoins are
//! cash waiting, not an investment, and stay out.

use crate::Amount;
use crate::exchange::is_usd_stablecoin;
use crate::model::store::{AssetRef, Store, StoreKind, SubType};

use crate::kernel::books::Books;
use crate::money::portfolio::{StorePosition, store_positions};
use crate::money::tree::ancestors_of;

#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct InvestmentRow {
    /// The ticker of a coin, the name of a futures sub-account.
    pub label: String,
    /// The coin; `None` for a futures sub-account.
    pub asset: Option<AssetRef>,
    /// The futures sub-account.
    pub store_id: Option<String>,
    pub quantity: Amount,
    /// What `quantity` counts: the ticker, or the margin's currency.
    pub unit: String,
    /// At cost where the cost is known, else at today's value.
    pub invested: Amount,
    pub value: Amount,
    /// Not sold yet: `value − invested` where the cost is known, else zero.
    pub profit: Amount,
    /// Every part has a known cost, so `profit` is whole.
    pub cost_known: bool,
}

/// All in the base currency, biggest investment first.
#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct Investments {
    pub base_currency: String,
    pub invested: Amount,
    pub value: Amount,
    pub profit: Amount,
    pub rows: Vec<InvestmentRow>,
}

/// `store_ids` limits it to these wallets and what is under them; empty is all.
pub fn investments(books: &Books<'_>, store_ids: &[String]) -> Investments {
    let doc = books.doc();
    let (base, fx) = books.base_and_fx();
    let in_scope = |store: &Store| {
        store_ids.is_empty()
            || store_ids.contains(&store.id)
            || ancestors_of(doc, &store.id)
                .iter()
                .any(|parent| store_ids.contains(&parent.id))
    };
    let mut rows: Vec<InvestmentRow> = Vec::new();
    let stores = doc.stores.iter().filter(|store| {
        !store.archived
            && matches!(store.kind, StoreKind::Crypto | StoreKind::Exchange)
            && in_scope(store)
    });
    for store in stores {
        let positions = store_positions(books, &store.id, fx);
        if store.sub_type == Some(SubType::Futures) {
            rows.extend(futures_row(store, &positions));
        } else {
            for position in &positions {
                add_position(&mut rows, position);
            }
        }
    }
    for row in &mut rows {
        row.invested = row.invested.round(2);
        row.value = row.value.round(2);
        row.profit = row.profit.round(2);
    }
    rows.sort_by_key(|row| std::cmp::Reverse(row.invested));
    let sum = |pick: fn(&InvestmentRow) -> Amount| {
        rows.iter()
            .fold(Amount::ZERO, |total, row| total + pick(row))
    };
    Investments {
        base_currency: base.to_string(),
        invested: sum(|row| row.invested),
        value: sum(|row| row.value),
        profit: sum(|row| row.profit),
        rows,
    }
}

/// A futures sub-account is one row: its margin, valued as it stands.
fn futures_row(store: &Store, positions: &[StorePosition]) -> Option<InvestmentRow> {
    let margin = positions.first()?;
    let value = positions
        .iter()
        .fold(Amount::ZERO, |sum, row| sum + row.value_base);
    Some(InvestmentRow {
        label: store.name.clone(),
        asset: None,
        store_id: Some(store.id.clone()),
        quantity: margin.quantity,
        unit: margin.asset.symbol.clone(),
        invested: value,
        value,
        profit: Amount::ZERO,
        cost_known: true,
    })
}

/// Joins a coin position to the row of its asset, or starts one. Cash (fiat,
/// dollar stablecoins) and empty positions stay out.
fn add_position(rows: &mut Vec<InvestmentRow>, position: &StorePosition) {
    let asset = &position.asset;
    if asset.is_fiat() || is_usd_stablecoin(&asset.key()) || position.quantity <= Amount::ZERO {
        return;
    }
    let invested = position
        .avg_cost
        .map_or(position.value_base, |cost| cost * position.quantity);
    let known = position.avg_cost.is_some();
    let profit = if known {
        position.value_base - invested
    } else {
        Amount::ZERO
    };
    match rows
        .iter_mut()
        .find(|row| row.asset.as_ref().is_some_and(|held| held.same_as(asset)))
    {
        Some(row) => {
            row.quantity += position.quantity;
            row.invested += invested;
            row.value += position.value_base;
            row.profit += profit;
            row.cost_known &= known;
        }
        None => rows.push(InvestmentRow {
            label: asset.symbol.clone(),
            asset: Some(asset.clone()),
            store_id: None,
            quantity: position.quantity,
            unit: asset.symbol.clone(),
            invested,
            value: position.value_base,
            profit,
            cost_known: known,
        }),
    }
}
