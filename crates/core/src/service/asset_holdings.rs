//! One asset across every wallet: where the same ticker is held, how much and
//! at what value — the «this asset elsewhere» block of the asset sheet.

use crate::Amount;
use crate::model::store::AssetRef;

use super::{PriceInfo, PriceOrigin, UnlockedVault};

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

impl UnlockedVault {
    /// The same asset (`AssetRef::same_as`: one key, or one ticker on one
    /// network) on every wallet. Derivatives are never joined to their coin.
    pub fn asset_holdings(&self, asset: &AssetRef) -> AssetHoldings {
        let (base, fx) = self.base_and_fx();
        let mut rows: Vec<AssetHolding> = self
            .doc
            .stores
            .iter()
            .flat_map(|store| {
                self.store_positions(&store.id, fx)
                    .into_iter()
                    .filter(|position| position.asset.same_as(asset))
                    .map(|position| AssetHolding {
                        store_id: store.id.clone(),
                        store_name: store.name.clone(),
                        path: self
                            .ancestors_of(&store.id)
                            .into_iter()
                            .map(|parent| parent.name.clone())
                            .collect(),
                        root_id: self.root_of(&store.id),
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
            base_currency: base,
            total_quantity: rows
                .iter()
                .fold(Amount::ZERO, |sum, row| sum + row.quantity),
            total_value_base: rows
                .iter()
                .fold(Amount::ZERO, |sum, row| sum + row.value_base),
            rows,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::model::chain::Chain;
    use crate::model::store::{AssetRef, StoreKind};
    use crate::model::transaction::{PriceStamp, Transaction};
    use crate::service::test_support::*;

    fn usdt_on(chain: Chain, contract: &str) -> AssetRef {
        AssetRef {
            contract: Some(contract.into()),
            ..AssetRef::crypto("USDT", "Tether", None, chain)
        }
    }

    #[test]
    fn asset_holdings_join_the_same_ticker_across_wallets() {
        let (_dir, mut vault) = empty_vault();
        let cold = store(&mut vault, "Cold", StoreKind::Crypto, None);
        let hot = store(&mut vault, "Hot", StoreKind::Crypto, None);
        let tether = AssetRef::crypto("USDT", "Tether", Some("tether"), Chain::Ethereum);
        let bare = AssetRef::crypto("USDT", "Tether", None, Chain::Ethereum);
        vault
            .add_transaction(income_of(&cold, tether.clone(), "100", "1", "in"))
            .unwrap();
        vault
            .add_transaction(income_of(&hot, bare, "50", "1", "in"))
            .unwrap();
        let held = vault.asset_holdings(&tether);
        assert_eq!(held.rows.len(), 2);
        assert_eq!(held.total_quantity, amount("150"));
        assert_eq!(held.rows[0].store_name, "Cold");
    }

    #[test]
    fn asset_holdings_keep_other_chains_apart() {
        let (_dir, mut vault) = empty_vault();
        let cold = store(&mut vault, "Cold", StoreKind::Crypto, None);
        let erc = usdt_on(
            Chain::Ethereum,
            "0xdac17f958d2ee523a2206206994597c13d831ec7",
        );
        let trc = usdt_on(Chain::Tron, "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t");
        vault
            .add_transaction(income_of(&cold, erc.clone(), "10", "1", "in"))
            .unwrap();
        vault
            .add_transaction(income_of(&cold, trc, "20", "1", "in"))
            .unwrap();
        let held = vault.asset_holdings(&erc);
        assert_eq!(held.rows.len(), 1);
        assert_eq!(held.total_quantity, amount("10"));
    }

    #[test]
    fn asset_holdings_leave_derivatives_out() {
        assert!(!btc().same_as(&AssetRef::perp("bybit", "BTC")));
        assert!(!AssetRef::perp("bybit", "BTC").same_as(&btc()));
        assert!(btc().same_as(&btc()));
    }

    #[test]
    fn asset_holdings_flag_peg_valued_stablecoins() {
        let (_dir, mut vault) = empty_vault();
        let cold = store(&mut vault, "Cold", StoreKind::Crypto, None);
        let tether = AssetRef::crypto("USDT", "Tether", Some("tether"), Chain::Ethereum);
        let mut topup = income_of(&cold, tether.clone(), "5", "1", "in");
        if let Transaction::Topup(inner) = &mut topup {
            inner.price = PriceStamp::default();
        }
        vault.add_transaction(topup).unwrap();
        let held = vault.asset_holdings(&tether);
        assert!(held.rows[0].peg_valued);
        assert_eq!(held.rows[0].value_base, amount("5"));
    }
}
