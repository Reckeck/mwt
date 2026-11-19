//! One asset across every wallet: where the same ticker is held, how much and
//! at what value — the «this asset elsewhere» block of the asset sheet.

use crate::Amount;
use crate::model::store::AssetRef;

use crate::kernel::books::Books;
use crate::money::portfolio::store_positions;
use crate::money::price_info::{PriceInfo, PriceOrigin};
use crate::money::tree::{ancestors_of, root_of};

/// The asset on one wallet.
#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct AssetHolding {
    pub store_id: String,
    pub store_name: String,
    /// Names from the top-level wallet down to the parent of `store_id`.
    pub path: Vec<String>,
    pub root_id: String,
    pub archived: bool,
    /// The asset as this wallet holds it: the network can differ between rows.
    pub asset: AssetRef,
    pub quantity: Amount,
    pub avg_cost: Option<Amount>,
    pub price_base: Amount,
    pub value_base: Amount,
    pub unrealized_base: Amount,
    pub price_info: PriceInfo,
    /// Valued at one dollar only because nobody quoted the stablecoin yet.
    pub peg_valued: bool,
}

/// Every wallet holding the asset, biggest first, with the totals.
#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct AssetHoldings {
    pub asset: AssetRef,
    pub base_currency: String,
    pub total_quantity: Amount,
    pub total_value_base: Amount,
    pub rows: Vec<AssetHolding>,
}

/// The same asset (`AssetRef::same_as`: one key, or one ticker on one
/// network) on every wallet. Derivatives are never joined to their coin.
pub fn asset_holdings(books: &Books<'_>, asset: &AssetRef) -> AssetHoldings {
    let doc = books.doc();
    let (base, fx) = books.base_and_fx();
    let mut rows: Vec<AssetHolding> = doc
        .stores
        .iter()
        .flat_map(|store| {
            store_positions(books, &store.id, fx)
                .into_iter()
                .filter(|position| position.asset.same_as(asset))
                .map(|position| AssetHolding {
                    store_id: store.id.clone(),
                    store_name: store.name.clone(),
                    path: ancestors_of(doc, &store.id)
                        .into_iter()
                        .map(|parent| parent.name.clone())
                        .collect(),
                    root_id: root_of(doc, &store.id),
                    archived: store.archived,
                    peg_valued: position.price_info.origin == PriceOrigin::Peg,
                    asset: position.asset,
                    quantity: position.quantity,
                    avg_cost: position.avg_cost,
                    price_base: position.price_base,
                    value_base: position.value_base,
                    unrealized_base: position.unrealized_base,
                    price_info: position.price_info,
                })
                .collect::<Vec<_>>()
        })
        .collect();
    rows.sort_by_key(|row| std::cmp::Reverse(row.value_base));
    AssetHoldings {
        asset: asset.clone(),
        base_currency: base.to_string(),
        total_quantity: rows
            .iter()
            .fold(Amount::ZERO, |sum, row| sum + row.quantity),
        total_value_base: rows
            .iter()
            .fold(Amount::ZERO, |sum, row| sum + row.value_base),
        rows,
    }
}
