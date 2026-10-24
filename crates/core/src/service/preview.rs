//! «What will this operation do?» — the before/after of every touched
//! position, computed without writing anything.

use std::collections::BTreeMap;

use crate::Amount;
use crate::model::store::{AssetRef, StoreKind};
use crate::model::transaction::{FuturesAction, FuturesDirection, Transaction};
use crate::pricing::history::secs_or_now;
use crate::service::UnlockedVault;
use crate::service::worth::FlowClass;

/// One `(store, asset)` position a draft would change.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct PositionChange {
    pub store_id: String,
    pub store_name: String,
    pub asset: AssetRef,
    pub before: Amount,
    pub after: Amount,
    /// `after − before`, in base currency at the current price.
    pub delta_base: Amount,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum PreviewWarning {
    /// A non-debt wallet would go below zero.
    NegativeBalance { store_id: String, asset: AssetRef },
    /// A credit card would be charged past its limit.
    CreditLimit { store_id: String, limit: Amount },
    /// The spending eats into money earmarked by envelopes.
    IntoEnvelopes {
        store_id: String,
        asset: AssetRef,
        earmarked: Amount,
    },
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct TxPreview {
    pub changes: Vec<PositionChange>,
    pub base_currency: String,
    /// Where the draft lands in the daily budget.
    pub flow: FlowClass,
    pub warnings: Vec<PreviewWarning>,
    /// Validation failure as `"CODE: message"`, the same string saving would
    /// return; `None` when the draft would be accepted.
    pub error: Option<String>,
    /// An opened futures position: the price near which it is liquidated, in
    /// the settle asset; `None` for anything else or without leverage.
    #[serde(default)]
    pub liquidation_price: Option<Amount>,
    /// An asset sold for money (Wallets-4 · 21): what it stood at in the
    /// books and what the sale made over it; `None` for anything else.
    #[serde(default)]
    pub sale: Option<SalePreview>,
    /// An exchange against the market of its moment (Currency-1 · 6); `None`
    /// for anything else or while the received side has no price.
    #[serde(default)]
    pub convert: Option<ConvertPreview>,
}

/// What an exchange paid over the market, valued like its loss (`TxLoss`).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct ConvertPreview {
    /// Market price of one received unit in the given currency.
    pub market_price: Amount,
    /// Given minus the market value of what was received, in the given currency.
    pub overpay: Amount,
    /// The same in the base currency.
    pub overpay_base: Amount,
    /// `overpay` as a share of what was given, percent.
    pub overpay_pct: Amount,
}

/// A sale against the books, in the sale's currency.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct SalePreview {
    /// Lowercase code of the amounts below.
    pub currency: String,
    /// The sold quantity at the price the wallet is valued at today.
    pub book_value: Amount,
    /// Proceeds minus `book_value`; negative for a sale at a loss.
    pub gain: Amount,
}

/// Maintenance margin rate assumed for the estimate (Bybit's first tier, 0.5 %).
const MAINTENANCE_MARGIN: &str = "0.005";

/// Isolated-margin estimate: a long is liquidated when the price falls by
/// `1/leverage − maintenance`, a short when it rises by the same share.
pub fn liquidation_price(tx: &Transaction) -> Option<Amount> {
    let Transaction::TradeFutures(trade) = tx else {
        return None;
    };
    let leverage = trade.leverage.filter(|value| value.is_positive())?;
    if trade.action != FuturesAction::Open || !trade.fill_price.is_positive() {
        return None;
    }
    let maintenance = Amount::parse(MAINTENANCE_MARGIN).ok()?;
    let share = Amount::from(1) / leverage - maintenance;
    let factor = match trade.direction {
        FuturesDirection::Long => Amount::from(1) - share,
        FuturesDirection::Short => Amount::from(1) + share,
    };
    Some((trade.fill_price * factor).max(Amount::ZERO).round(8))
}

impl UnlockedVault {
    /// Preview a draft. `replaces` is the id of the transaction being edited:
    /// its effect is taken out first, so the preview shows the net change.
    pub fn preview_transaction(&self, mut tx: Transaction, replaces: Option<&str>) -> TxPreview {
        // An edit is the same transaction: a correction must not count itself.
        if let Some(id) = replaces {
            tx.meta_mut().id = id.to_string();
        }
        let error = self
            .prepare_transaction(&mut tx)
            .err()
            .map(|err| format!("{}: {err}", err.code()));

        let mut deltas: BTreeMap<(String, String), (AssetRef, Amount)> = BTreeMap::new();
        let mut add = |store_id: &str, asset: &AssetRef, amount: Amount| {
            let entry = deltas
                .entry((store_id.to_string(), asset.key()))
                .or_insert_with(|| (asset.clone(), Amount::ZERO));
            entry.1 += amount;
        };
        for leg in tx.legs() {
            add(&leg.store_id, &leg.asset, leg.amount);
        }
        if let Some(old) = replaces.and_then(|id| {
            self.doc
                .transactions
                .iter()
                .find(|known| known.meta().id == id)
        }) {
            for leg in old.legs() {
                add(&leg.store_id, &leg.asset, -leg.amount);
            }
        }

        let (base, fx) = self.base_and_fx();
        let mut changes = Vec::new();
        let mut warnings = Vec::new();
        for ((store_id, asset_key), (asset, delta)) in deltas {
            if delta.is_zero() {
                continue;
            }
            let before = self.position(&store_id, &asset_key);
            let after = before + delta;
            let store = self.find_store(&store_id);
            let price = if asset.is_derivative() {
                Amount::ZERO
            } else {
                self.price_usd_for(&asset_key)
            };
            if let Some(store) = store {
                let is_debt = store.kind == StoreKind::Liability || store.liability.is_some();
                let limit = store.liability.as_ref().and_then(|terms| terms.limit);
                match limit {
                    Some(limit) if after < -limit => warnings.push(PreviewWarning::CreditLimit {
                        store_id: store_id.clone(),
                        limit,
                    }),
                    _ if !is_debt && after.is_negative() && !asset.is_derivative() => warnings
                        .push(PreviewWarning::NegativeBalance {
                            store_id: store_id.clone(),
                            asset: asset.clone(),
                        }),
                    _ => {
                        let earmarked = self.earmarked(&store_id, &asset_key);
                        if delta.is_negative() && !earmarked.is_zero() && after < earmarked {
                            warnings.push(PreviewWarning::IntoEnvelopes {
                                store_id: store_id.clone(),
                                asset: asset.clone(),
                                earmarked,
                            });
                        }
                    }
                }
            }
            changes.push(PositionChange {
                store_name: store.map(|store| store.name.clone()).unwrap_or_default(),
                store_id,
                asset,
                before,
                after,
                delta_base: delta * price * fx,
            });
        }

        TxPreview {
            changes,
            base_currency: base,
            flow: self.classify_flow(&tx),
            warnings,
            error,
            liquidation_price: liquidation_price(&tx),
            sale: self.sale_preview(&tx),
            convert: self.convert_preview(&tx),
        }
    }

    /// `None` when the asset has no price yet: there is nothing to compare with.
    fn sale_preview(&self, tx: &Transaction) -> Option<SalePreview> {
        let Transaction::AssetSell(sell) = tx else {
            return None;
        };
        let usd = self.price_usd_for(&sell.asset.key());
        if !usd.is_positive() || !sell.quantity.is_positive() {
            return None;
        }
        let currency = sell.currency.symbol.to_lowercase();
        let book_value = (sell.quantity * usd * self.per_usd(&currency)?).round(2);
        let gain = (sell.quantity * sell.unit_price).round(2) - book_value;
        Some(SalePreview {
            currency,
            book_value,
            gain,
        })
    }

    pub(crate) fn convert_preview(&self, tx: &Transaction) -> Option<ConvertPreview> {
        let Transaction::Convert(inner) = tx else {
            return None;
        };
        let (give, got) = (inner.from.amount.abs(), inner.to.amount.abs());
        if !give.is_positive() || !got.is_positive() {
            return None;
        }
        let book = self.rate_book();
        let secs = secs_or_now(&tx.meta().timestamp);
        let currency = inner.from.asset.symbol.to_lowercase();
        let got_value = Self::value_at(&book, &inner.to.asset, got, &currency, secs);
        if !got_value.is_positive() {
            return None;
        }
        let base = self.base_and_fx().0;
        let overpay = give - got_value;
        let overpay_base = Self::value_at(&book, &inner.from.asset, give, &base, secs)
            - Self::value_at(&book, &inner.to.asset, got, &base, secs);
        Some(ConvertPreview {
            market_price: (got_value / got).round(6),
            overpay: overpay.round(2),
            overpay_base: overpay_base.round(2),
            overpay_pct: (overpay / give * Amount::from(100)).round(1),
        })
    }

    /// Current quantity of one asset in one store (zero when not held).
    fn position(&self, store_id: &str, asset_key: &str) -> Amount {
        self.doc
            .holdings
            .iter()
            .filter(|holding| holding.store_id == store_id && holding.asset.key() == asset_key)
            .fold(Amount::ZERO, |sum, holding| sum + holding.quantity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::test_support::{amount, income, payment, store};

    #[test]
    fn shows_before_and_after_without_writing() {
        let (_dir, mut vault) = crate::service::test_support::empty_vault();
        let cash = store(&mut vault, "Wallet", StoreKind::Cash, None);
        vault
            .add_transaction(income(&cash, "100", "salary"))
            .unwrap();
        let count = vault.doc.transactions.len();

        let preview = vault.preview_transaction(payment(&cash, "30", "food"), None);
        assert_eq!(preview.error, None);
        assert_eq!(preview.changes.len(), 1);
        assert_eq!(preview.changes[0].before, amount("100"));
        assert_eq!(preview.changes[0].after, amount("70"));
        assert!(preview.warnings.is_empty());
        assert_eq!(preview.flow, FlowClass::Daily);
        assert_eq!(vault.doc.transactions.len(), count);
    }

    #[test]
    fn a_sale_shows_the_book_value_and_the_gain() {
        use crate::model::transaction::{AssetSellTx, KindDetails, TopupTx, TxMeta};
        use crate::service::test_support::{meta, usd};
        let (_dir, mut vault) = crate::service::test_support::empty_vault();
        vault.set_base_currency("usd");
        let things = store(&mut vault, "Things", StoreKind::Property, None);
        let car = AssetRef {
            symbol: "CAR".into(),
            name: "Car".into(),
            coingecko_id: None,
            chain: None,
            contract: None,
            quote: "CAR".into(),
        };
        vault
            .add_transaction(Transaction::Topup(TopupTx {
                meta: meta("bought", &["income_other"]),
                store_id: things.clone(),
                asset: car.clone(),
                amount: amount("1"),
                fee: None,
                from: None,
                details: KindDetails {
                    unit_price: Some(amount("10000")),
                    ..KindDetails::default()
                },
                price: Default::default(),
            }))
            .unwrap();
        let sell = Transaction::AssetSell(AssetSellTx {
            meta: TxMeta::new("car"),
            store_id: things,
            asset: car,
            unit_price: amount("12000"),
            quantity: amount("1"),
            currency: usd(),
            proceeds_to: None,
            fee: None,
            price: Default::default(),
        });
        let sale = vault.sale_preview(&sell).unwrap();
        assert_eq!(sale.currency, "usd");
        assert_eq!(sale.book_value, amount("10000"));
        assert_eq!(sale.gain, amount("2000"));
        assert_eq!(vault.sale_preview(&payment("cash", "1", "food")), None);
    }

    #[test]
    fn estimates_liquidation_of_an_opened_position() {
        let mut tx = crate::model::fixtures::every_transaction()
            .into_iter()
            .find(|tx| matches!(tx, Transaction::TradeFutures(_)))
            .unwrap();
        let Transaction::TradeFutures(trade) = &mut tx else {
            unreachable!()
        };
        trade.action = FuturesAction::Open;
        let Transaction::TradeFutures(trade) = &mut tx else {
            unreachable!()
        };
        trade.fill_price = amount("89552");
        trade.leverage = Some(amount("5"));
        trade.direction = FuturesDirection::Long;
        assert_eq!(liquidation_price(&tx), Some(amount("72089.36")));
        let Transaction::TradeFutures(trade) = &mut tx else {
            unreachable!()
        };
        trade.direction = FuturesDirection::Short;
        assert_eq!(liquidation_price(&tx), Some(amount("107014.64")));
        let Transaction::TradeFutures(trade) = &mut tx else {
            unreachable!()
        };
        trade.leverage = None;
        assert_eq!(liquidation_price(&tx), None);
    }

    #[test]
    fn warns_about_going_negative() {
        let (_dir, mut vault) = crate::service::test_support::empty_vault();
        let cash = store(&mut vault, "Wallet", StoreKind::Cash, None);
        let preview = vault.preview_transaction(payment(&cash, "5", "food"), None);
        assert!(matches!(
            preview.warnings.as_slice(),
            [PreviewWarning::NegativeBalance { .. }]
        ));
    }

    #[test]
    fn edit_shows_the_net_change() {
        let (_dir, mut vault) = crate::service::test_support::empty_vault();
        let cash = store(&mut vault, "Wallet", StoreKind::Cash, None);
        vault
            .add_transaction(income(&cash, "100", "salary"))
            .unwrap();
        let spent = vault.add_transaction(payment(&cash, "30", "food")).unwrap();

        let mut edited = payment(&cash, "50", "food");
        edited.meta_mut().id = spent.meta().id.clone();
        let preview = vault.preview_transaction(edited, Some(&spent.meta().id));
        assert_eq!(preview.changes[0].before, amount("70"));
        assert_eq!(preview.changes[0].after, amount("50"));
    }

    #[test]
    fn invalid_draft_reports_the_error() {
        let (_dir, vault) = crate::service::test_support::empty_vault();
        let preview = vault.preview_transaction(payment("missing", "5", "food"), None);
        assert!(preview.error.is_some());
    }
}
