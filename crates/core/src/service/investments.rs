//! "Investments" (Money 2 · 13): what was put into investments and what it made,
//! per asset. Coins on crypto and exchange wallets join by asset; a futures
//! sub-account is one row with its margin. Fiat and dollar stablecoins are
//! cash waiting, not an investment, and stay out.

use crate::Amount;
use crate::exchange::is_usd_stablecoin;
use crate::model::store::{AssetRef, Store, StoreKind, SubType};

use super::UnlockedVault;

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

impl UnlockedVault {
    /// `store_ids` limits it to these wallets and what is under them; empty is all.
    pub fn investments(&self, store_ids: &[String]) -> Investments {
        let (base, fx) = self.base_and_fx();
        let in_scope = |store: &Store| {
            store_ids.is_empty()
                || store_ids.contains(&store.id)
                || self
                    .ancestors_of(&store.id)
                    .iter()
                    .any(|parent| store_ids.contains(&parent.id))
        };
        let mut rows: Vec<InvestmentRow> = Vec::new();
        let stores = self.doc.stores.iter().filter(|store| {
            !store.archived
                && matches!(store.kind, StoreKind::Crypto | StoreKind::Exchange)
                && in_scope(store)
        });
        for store in stores {
            let positions = self.store_positions(&store.id, fx);
            if store.sub_type == Some(SubType::Futures) {
                let Some(margin) = positions.first() else {
                    continue;
                };
                let value = positions
                    .iter()
                    .fold(Amount::ZERO, |sum, row| sum + row.value_base);
                rows.push(InvestmentRow {
                    label: store.name.clone(),
                    asset: None,
                    store_id: Some(store.id.clone()),
                    quantity: margin.quantity,
                    unit: margin.asset.symbol.clone(),
                    invested: value,
                    value,
                    profit: Amount::ZERO,
                    cost_known: true,
                });
                continue;
            }
            for position in positions {
                let asset = &position.asset;
                if asset.is_fiat()
                    || is_usd_stablecoin(&asset.key())
                    || position.quantity <= Amount::ZERO
                {
                    continue;
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
            base_currency: base,
            invested: sum(|row| row.invested),
            value: sum(|row| row.value),
            profit: sum(|row| row.profit),
            rows,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::model::chain::Chain;
    use crate::model::store::{AssetRef, StoreKind};
    use crate::service::test_support::*;

    #[test]
    fn investments_join_coins_and_leave_cash_out() {
        let (_dir, mut vault) = empty_vault();
        let cold = store(&mut vault, "Cold", StoreKind::Crypto, None);
        let hot = store(&mut vault, "Hot", StoreKind::Crypto, None);
        let btc = AssetRef::crypto("BTC", "Bitcoin", Some("bitcoin"), Chain::Bitcoin);
        let usdt = AssetRef::crypto("USDT", "Tether", Some("tether"), Chain::Ethereum);
        vault
            .add_transaction(income_of(&cold, btc.clone(), "0.05", "60000", "buy"))
            .unwrap();
        vault
            .add_transaction(income_of(&hot, btc, "0.05", "60000", "buy"))
            .unwrap();
        vault
            .add_transaction(income_of(&hot, usdt, "500", "1", "cash"))
            .unwrap();
        let report = vault.investments(&[]);
        assert_eq!(report.rows.len(), 1, "USDT is cash, not an investment");
        assert_eq!(report.rows[0].label, "BTC");
        assert_eq!(report.rows[0].quantity, amount("0.1"));
        assert_eq!(report.rows[0].invested, amount("6000"));
        // Only the hot wallet: half of it.
        assert_eq!(vault.investments(&[hot]).rows[0].quantity, amount("0.05"));
    }
}
