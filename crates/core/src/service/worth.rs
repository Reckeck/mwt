//! Net worth (assets, debts, what people owe), hand-set prices of property,
//! and the classification of money flows the budget is built on (plan §4.8,
//! §4.9, §7.3).

use std::collections::{HashMap, HashSet};

use crate::Amount;
use crate::error::{Error, Result};
use crate::model::category::CategoryGroup;
use crate::model::store::StoreKind;
use crate::model::transaction::{AccountRef, Transaction};
use crate::model::vault::{ManualPrice, now_iso};

use super::UnlockedVault;

/// Everything the user owns and owes, in the base currency. All parts are
/// non-negative; `net = financial + non_financial + receivables − liabilities − payables`.
#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct NetWorth {
    pub base_currency: String,
    /// Money and investments: every store except property and loans.
    pub financial: Amount,
    /// Property valued by hand (`asset` stores).
    pub non_financial: Amount,
    /// What people owe me (debt transfers).
    pub receivables: Amount,
    /// Loans, mortgages, credit cards.
    pub liabilities: Amount,
    /// What I owe people.
    pub payables: Amount,
    pub net: Amount,
}

/// Where a transaction lands in the budget (plan §7.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum FlowClass {
    Income,
    Fixed,
    Daily,
    Savings,
    /// Money entering the budget from the user's own non-budget store (sold crypto, cashed out).
    BudgetIn,
    /// Money leaving the budget for the user's own non-budget store.
    BudgetOut,
    /// Not a budget event: internal moves, debts, trades, bookkeeping.
    Ignored,
}

impl UnlockedVault {
    /// Units of `currency` per USD from the FX cache; USD itself is 1.
    pub(crate) fn per_usd(&self, currency: &str) -> Option<Amount> {
        let currency = currency.to_lowercase();
        if currency == "usd" {
            return Some(Amount::new(rust_decimal::Decimal::ONE));
        }
        self.doc
            .fx_cache
            .iter()
            .filter(|point| point.currency == currency && !point.per_usd.is_zero())
            .max_by(|left, right| left.ts.cmp(&right.ts))
            .map(|point| point.per_usd)
    }

    /// USD value of one unit of a hand-priced asset, if it has a price and the
    /// rate of its currency is known.
    pub(crate) fn manual_price_usd(&self, asset_key: &str) -> Option<Amount> {
        let manual = self
            .doc
            .manual_prices
            .iter()
            .find(|price| price.asset_key == asset_key)?;
        let rate = self.per_usd(&manual.currency)?;
        Some(manual.unit_price / rate)
    }

    /// Set the value of one unit of a property asset. Applies to its whole
    /// history (plan D24), so revaluing does not make the chart jump.
    pub fn set_manual_price(
        &mut self,
        asset_key: &str,
        unit_price: Amount,
        currency: &str,
    ) -> Result<ManualPrice> {
        if !unit_price.is_positive() {
            return Err(Error::InvalidInput("the price must be positive".into()));
        }
        let currency = currency.trim().to_lowercase();
        if currency.is_empty() {
            return Err(Error::InvalidInput("choose a currency".into()));
        }
        let held_in_property = self.doc.holdings.iter().any(|holding| {
            holding.asset.key() == asset_key
                && self.doc.stores.iter().any(|store| {
                    store.id == holding.store_id
                        && matches!(store.kind, StoreKind::Property | StoreKind::Loyalty)
                })
        });
        if !held_in_property {
            return Err(Error::InvalidInput(
                "a hand-set price is only for property held in a property wallet".into(),
            ));
        }
        let price = ManualPrice {
            asset_key: asset_key.to_string(),
            unit_price,
            currency,
            updated_at: now_iso(),
        };
        self.doc
            .manual_prices
            .retain(|existing| existing.asset_key != asset_key);
        self.doc.manual_prices.push(price.clone());
        self.persist()?;
        Ok(price)
    }

    pub fn clear_manual_price(&mut self, asset_key: &str) -> Result<()> {
        self.doc
            .manual_prices
            .retain(|existing| existing.asset_key != asset_key);
        self.persist()
    }

    pub fn net_worth(&self) -> NetWorth {
        let (base, fx) = self.base_and_fx();
        let kinds: HashMap<&str, StoreKind> = self
            .doc
            .stores
            .iter()
            .map(|store| (store.id.as_str(), store.kind))
            .collect();
        let mut out = NetWorth {
            base_currency: base,
            ..NetWorth::default()
        };
        let mut loans: HashMap<&str, Amount> = HashMap::new();
        for holding in self
            .doc
            .holdings
            .iter()
            .filter(|holding| !holding.asset.is_derivative())
        {
            let value = holding.quantity * self.price_usd_for(&holding.asset.key()) * fx;
            match kinds.get(holding.store_id.as_str()) {
                Some(StoreKind::Property | StoreKind::Loyalty) => out.non_financial += value,
                Some(StoreKind::Liability) => {
                    *loans.entry(holding.store_id.as_str()).or_default() += value
                }
                _ => out.financial += value,
            }
        }
        for value in loans.into_values() {
            if value.is_negative() {
                out.liabilities += value.abs();
            } else {
                out.financial += value;
            }
        }
        for balance in self.contact_balances() {
            let price = self.price_usd_for(&balance.asset.key()) * fx;
            out.receivables += balance.receivable.max(Amount::ZERO) * price;
            out.payables += balance.payable.max(Amount::ZERO) * price;
        }
        out.net =
            out.financial + out.non_financial + out.receivables - out.liabilities - out.payables;
        out
    }

    /// Stores that belong to at least one active goal.
    fn goal_store_ids(&self) -> HashSet<String> {
        self.doc
            .goals
            .iter()
            .filter(|goal| goal.active && !goal.scope.store_ids.is_empty())
            .flat_map(|goal| self.resolve_scope(&goal.scope))
            .collect()
    }

    fn category_group(&self, tx: &Transaction) -> Option<CategoryGroup> {
        tx.meta().categories.iter().find_map(|key| {
            self.doc
                .categories
                .iter()
                .find(|category| &category.key == key)
                .map(|category| category.group)
        })
    }

    fn store_in_budget(&self, id: &str) -> bool {
        self.doc
            .stores
            .iter()
            .any(|store| store.id == id && store.in_budget())
    }

    fn store_kind(&self, id: &str) -> Option<StoreKind> {
        self.doc
            .stores
            .iter()
            .find(|store| store.id == id)
            .map(|store| store.kind)
    }

    /// A spending flow by its category, `Daily` when it has none.
    fn spending(group: Option<CategoryGroup>) -> FlowClass {
        match group {
            Some(CategoryGroup::Fixed) => FlowClass::Fixed,
            Some(CategoryGroup::Savings) => FlowClass::Savings,
            Some(CategoryGroup::Income) => FlowClass::Ignored,
            _ => FlowClass::Daily,
        }
    }

    /// A move between two sides, at least one of them the user's: across the
    /// budget boundary, to a loan, to a goal, or out to someone.
    fn classify_move(
        &self,
        from: &AccountRef,
        to: &AccountRef,
        group: Option<CategoryGroup>,
    ) -> FlowClass {
        match (from.store_id(), to.store_id()) {
            (Some(source), Some(target)) => {
                match (self.store_in_budget(source), self.store_in_budget(target)) {
                    (true, true) | (false, false) => FlowClass::Ignored,
                    (true, false) if self.store_kind(target) == Some(StoreKind::Liability) => {
                        FlowClass::Fixed
                    }
                    (true, false) if self.goal_store_ids().contains(target) => FlowClass::Savings,
                    (true, false) => FlowClass::BudgetOut,
                    (false, true) => FlowClass::BudgetIn,
                }
            }
            (Some(source), None) if self.store_in_budget(source) => Self::spending(group),
            (None, Some(target)) if self.store_in_budget(target) => match group {
                Some(CategoryGroup::Income) => FlowClass::Income,
                _ => FlowClass::Ignored,
            },
            _ => FlowClass::Ignored,
        }
    }

    /// The one classification budgets, cash flow, month totals and the
    /// calendar share (plan §7.3).
    pub fn classify_flow(&self, tx: &Transaction) -> FlowClass {
        let group = self.category_group(tx);
        if group == Some(CategoryGroup::Service) || tx.is_internal(|id| self.root_of(id)) {
            return FlowClass::Ignored;
        }
        match tx {
            Transaction::Topup(inner) if self.store_in_budget(&inner.store_id) => match group {
                Some(CategoryGroup::Income) => FlowClass::Income,
                _ => FlowClass::Ignored,
            },
            Transaction::Payment(inner) if self.store_in_budget(&inner.store_id) => {
                Self::spending(group)
            }
            Transaction::Transfer(inner) if inner.debt.is_none() => {
                self.classify_move(&inner.from, &inner.to, group)
            }
            Transaction::Convert(inner) => {
                self.classify_move(&inner.from.account, &inner.to.account, group)
            }
            Transaction::AssetSell(inner) => match &inner.proceeds_to {
                Some(target) if self.store_in_budget(target) => FlowClass::BudgetIn,
                _ => FlowClass::Ignored,
            },
            _ => FlowClass::Ignored,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use super::*;
    use crate::model::planning::Goal;
    use crate::model::store::{AssetRef, LiabilityKind, LiabilityTerms, StoreScope};
    use crate::service::StoreExtras;

    /// A credit card: draw the loan, buy with the card, interest, pay back.
    #[test]
    fn a_loan_is_a_negative_balance_and_a_liability() {
        let (_dir, mut vault) = empty_vault();
        vault.set_base_currency("usd");
        quote_usd(&mut vault);
        let bank = store(&mut vault, "Bank", StoreKind::Bank, None);
        let card = vault
            .create_store_with(
                "Card",
                StoreKind::Liability,
                "",
                "",
                None,
                StoreExtras {
                    liability: Some(LiabilityTerms {
                        kind: LiabilityKind::CreditCard,
                        rate: None,
                        limit: Some(amount("1000")),
                        due_day: Some(25),
                        monthly_payment: None,
                    }),
                    ..StoreExtras::default()
                },
            )
            .unwrap()
            .id;
        let loan = transfer_between(
            AccountRef::Store(card.clone()),
            AccountRef::Store(bank.clone()),
            usd(),
            "500",
        );
        vault.add_transaction(loan).unwrap();
        vault
            .add_transaction(payment(&card, "200", "shop"))
            .unwrap();
        let mut interest = payment(&card, "10", "interest");
        interest.meta_mut().categories = vec!["loan_interest".into()];
        vault.add_transaction(interest.clone()).unwrap();
        let repay = transfer_between(
            AccountRef::Store(bank.clone()),
            AccountRef::Store(card.clone()),
            usd(),
            "300",
        );
        vault.add_transaction(repay.clone()).unwrap();

        assert_eq!(holding_of(&vault, &card, &usd()), amount("-410"));
        let worth = vault.net_worth();
        assert_eq!(worth.financial, amount("200"));
        assert_eq!(worth.liabilities, amount("410"));
        assert_eq!(worth.net, amount("-210"));
        let summary = vault.portfolio_summary();
        assert_eq!(summary.liabilities_base, amount("410"));
        assert_eq!(summary.net_worth_base, amount("-210"));

        // paying the loan off is a mandatory cost; the card is outside the budget
        assert_eq!(vault.classify_flow(&repay), FlowClass::Fixed);
        assert_eq!(vault.classify_flow(&interest), FlowClass::Ignored);
    }

    /// Plan §7.3, one row at a time.
    #[test]
    fn flows_are_classified_for_the_budget() {
        let (_dir, mut vault) = empty_vault();
        let bank = store(&mut vault, "Bank", StoreKind::Bank, None);
        let usd_account = store(&mut vault, "USD", StoreKind::Bank, Some(&bank));
        let cash = store(&mut vault, "Cash", StoreKind::Cash, None);
        let exchange = store(&mut vault, "Bybit", StoreKind::Exchange, None);
        let piggy = vault
            .create_store_with(
                "Piggy",
                StoreKind::Bank,
                "",
                "",
                None,
                StoreExtras {
                    purpose: Some(crate::model::store::Purpose::Investment),
                    ..StoreExtras::default()
                },
            )
            .unwrap()
            .id;
        vault.doc.goals.push(Goal {
            id: "goal".into(),
            name: "Car".into(),
            active: true,
            target_amount: amount("1000"),
            currency: "usd".into(),
            starts_at: "2026-01-01T00:00:00Z".into(),
            deadline: None,
            priority: 3,
            monthly_commitment: None,
            scope: StoreScope::store(&piggy, true),
            created_at: "2026-01-01T00:00:00Z".into(),
            scenario_id: None,
            expected_return: None,
        });
        let store_ref = |id: &str| AccountRef::Store(id.to_string());
        let with_category = |mut tx: Transaction, key: &str| {
            tx.meta_mut().categories = vec![key.to_string()];
            tx
        };
        let cases = [
            (income(&bank, "1", "salary"), FlowClass::Income),
            (
                with_category(income(&bank, "1", "x"), "trading"),
                FlowClass::Ignored,
            ),
            (
                with_category(payment(&bank, "1", "rent"), "rent_mortgage"),
                FlowClass::Fixed,
            ),
            (payment(&cash, "1", "food"), FlowClass::Daily),
            (
                with_category(payment(&bank, "1", "save"), "savings"),
                FlowClass::Savings,
            ),
            (
                payment(&exchange, "1", "outside the budget"),
                FlowClass::Ignored,
            ),
            (
                transfer_between(store_ref(&bank), store_ref(&usd_account), usd(), "1"),
                FlowClass::Ignored,
            ),
            (
                transfer_between(store_ref(&bank), store_ref(&cash), usd(), "1"),
                FlowClass::Ignored,
            ),
            (
                transfer_between(store_ref(&bank), store_ref(&piggy), usd(), "1"),
                FlowClass::Savings,
            ),
            (
                transfer_between(store_ref(&exchange), store_ref(&bank), usd(), "1"),
                FlowClass::BudgetIn,
            ),
            (
                transfer_between(store_ref(&bank), store_ref(&exchange), usd(), "1"),
                FlowClass::BudgetOut,
            ),
            (
                transfer_between(
                    store_ref(&cash),
                    AccountRef::Raw("market".into()),
                    usd(),
                    "1",
                ),
                FlowClass::Daily,
            ),
            (
                with_category(
                    transfer_between(
                        AccountRef::Raw("client".into()),
                        store_ref(&bank),
                        usd(),
                        "1",
                    ),
                    "freelance",
                ),
                FlowClass::Income,
            ),
            (
                transfer_between(
                    AccountRef::Raw("refund".into()),
                    store_ref(&bank),
                    usd(),
                    "1",
                ),
                FlowClass::Ignored,
            ),
        ];
        for (index, (tx, expected)) in cases.into_iter().enumerate() {
            assert_eq!(
                vault.classify_flow(&tx),
                expected,
                "case {index}: {}",
                tx.meta().comment
            );
        }
    }

    /// Futures notional is not the user's money.
    #[test]
    fn derivatives_never_count_in_value() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[1].id.clone();
        vault.doc.price_cache.push(crate::model::vault::PricePoint {
            asset_key: AssetRef::perp("bybit", "BTCUSDT").key(),
            value_usd: amount("60000"),
            ts: "2000-01-01T00:00:00Z".into(),
            source: "test".into(),
            change_24h_pct: None,
        });
        let open = Transaction::TradeFutures(crate::model::transaction::TradeFuturesTx {
            meta: meta("perp", &[]),
            store_id: store.clone(),
            contract: AssetRef::perp("bybit", "BTCUSDT"),
            settle: AssetRef::fiat("USDT", "Tether"),
            direction: crate::model::transaction::FuturesDirection::Long,
            action: crate::model::transaction::FuturesAction::Open,
            quantity: amount("2"),
            fill_price: amount("60000"),
            leverage: None,
            fee: None,
            closed_pnl: None,
            order_id: None,
            category: crate::model::transaction::FuturesCategory::Linear,
            price: crate::model::transaction::PriceStamp::default(),
        });
        vault.add_transaction(open).unwrap();
        assert_eq!(vault.portfolio_summary().total_usd, Amount::ZERO);
        assert_eq!(vault.net_worth().net, Amount::ZERO);
        let overview = vault.store_overview(&store, true).unwrap();
        assert!(overview.positions.is_empty());
        assert_eq!(overview.open_positions.len(), 1);
        assert!(
            vault
                .history_series("usd")
                .iter()
                .all(|point| point.value_usd.is_zero())
        );
    }

    /// A fee is a cost, not a sale: buying and selling at one price with fees
    /// realizes nothing, and the fees are reported on their own.
    #[test]
    fn fees_do_not_fake_a_profit() {
        let (_dir, mut vault) = setup();
        vault.set_base_currency("usd");
        let store = vault.doc.stores[1].id.clone();
        let mut deposit = income(&store, "1000", "in");
        at(&mut deposit, "2026-01-01T00:00:00Z");
        vault.add_transaction(deposit).unwrap();
        let Transaction::TradeSpot(mut buy) = trade(&store, "500", "0.01", "buy") else {
            unreachable!()
        };
        buy.sold.price = usd_price("1");
        buy.bought.price = usd_price("50000");
        buy.fee = fee_usd("2");
        let mut buy = Transaction::TradeSpot(buy);
        at(&mut buy, "2026-01-02T00:00:00Z");
        vault.add_transaction(buy).unwrap();
        let Transaction::TradeSpot(mut sell) = sell_btc(&store, "0.01", "500", "50000") else {
            unreachable!()
        };
        sell.fee = fee_usd("2");
        let mut sell = Transaction::TradeSpot(sell);
        at(&mut sell, "2026-01-03T00:00:00Z");
        vault.add_transaction(sell).unwrap();

        let pnl = vault.realized_pnl();
        assert_eq!(pnl.total_base, Amount::ZERO);
        assert_eq!(pnl.fees_base, amount("4"));
        assert_eq!(holding_of(&vault, &store, &usd()), amount("996"));
    }
}
