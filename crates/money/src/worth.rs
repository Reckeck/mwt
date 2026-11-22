//! Net worth figures and the classification of money flows the budget is
//! built on (plan §4.8, §4.9, §7.3). Everything here reads the document or
//! the books; the assembly of `NetWorth` and the scenarios that write
//! hand-set prices live in `usecases::money::capital`.

use std::collections::HashSet;

use crate::books::Books;
use crate::rates::per_usd;
use crate::tree::{resolve_scope, root_of};
use mwt_base::Amount;
use mwt_base::error::{Error, Result};
use mwt_model::CategoryGroup;
use mwt_model::{AccountRef, Transaction};
use mwt_model::{LiabilityKind, StoreKind};
use mwt_model::{ManualPrice, VaultDoc};

/// Everything the user owns and owes, in the base currency. All parts are
/// non-negative; `net = financial + non_financial + receivables − liabilities − payables`.
#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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
    /// `liabilities` by loan kind, largest first (plan E); sums to it.
    pub liabilities_by_kind: Vec<LiabilityShare>,
    /// What I owe people.
    pub payables: Amount,
    pub net: Amount,
    /// `financial` by portfolio basket (plan E/F), largest first; the
    /// emergency fund is apart in `financial_fund`. Sums to `financial`
    /// with it.
    pub financial_by_class: Vec<ClassShare>,
    /// The emergency fund's part of `financial`.
    pub financial_fund: Amount,
}

/// The part of the financial capital in one basket; `class_id: None` is
/// what is in no basket.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ClassShare {
    pub class_id: Option<String>,
    pub amount: Amount,
}

/// The part of the debts that is one kind of loan.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LiabilityShare {
    pub kind: LiabilityKind,
    pub amount: Amount,
}

/// Where a transaction lands in the budget (plan §7.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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

/// USD value of one unit of a hand-priced asset, if it has a price and the
/// rate of its currency is known. `now_secs` is the moment a valuation curve
/// is read at.
pub fn manual_price_usd(books: &Books<'_>, asset_key: &str, now_secs: i64) -> Option<Amount> {
    let doc = books.doc();
    if let Some(curve) = crate::valuation::valuation_curve(books, asset_key) {
        let value = curve.value_at(now_secs)?;
        let rate = per_usd(doc, &curve.currency)?;
        return (!rate.is_zero()).then(|| value / rate);
    }
    let manual = doc
        .manual_prices
        .iter()
        .find(|price| price.asset_key == asset_key)?;
    let rate = per_usd(doc, &manual.currency)?;
    Some(manual.unit_price / rate)
}

/// The hand-set price a property asset gets, checked against the document.
/// Applies to its whole history (plan D24), so revaluing does not make the
/// chart jump. `updated_at` is the moment of the user's action.
pub fn plan_manual_price(
    doc: &VaultDoc,
    asset_key: &str,
    unit_price: Amount,
    currency: &str,
    updated_at: String,
) -> Result<ManualPrice> {
    if !unit_price.is_positive() {
        return Err(Error::InvalidInput("the price must be positive".into()));
    }
    let currency = currency.trim().to_lowercase();
    if currency.is_empty() {
        return Err(Error::InvalidInput("choose a currency".into()));
    }
    let held_in_property = doc.holdings.iter().any(|holding| {
        holding.asset.key() == asset_key
            && doc.stores.iter().any(|store| {
                store.id == holding.store_id
                    && matches!(store.kind, StoreKind::Property | StoreKind::Loyalty)
            })
    });
    if !held_in_property {
        return Err(Error::InvalidInput(
            "a hand-set price is only for property held in a property wallet".into(),
        ));
    }
    Ok(ManualPrice {
        asset_key: asset_key.to_string(),
        unit_price,
        currency,
        updated_at,
    })
}

/// Stores that belong to at least one active goal.
pub fn goal_store_ids(doc: &VaultDoc) -> HashSet<String> {
    doc.goals
        .iter()
        .filter(|goal| goal.active && !goal.scope.store_ids.is_empty())
        .flat_map(|goal| resolve_scope(doc, &goal.scope))
        .collect()
}

pub fn category_group(doc: &VaultDoc, tx: &Transaction) -> Option<CategoryGroup> {
    tx.meta().categories.iter().find_map(|key| {
        doc.categories
            .iter()
            .find(|category| &category.key == key)
            .map(|category| category.group)
    })
}

pub fn store_in_budget(doc: &VaultDoc, id: &str) -> bool {
    doc.stores
        .iter()
        .any(|store| store.id == id && store.in_budget())
}

pub fn store_kind(doc: &VaultDoc, id: &str) -> Option<StoreKind> {
    doc.stores
        .iter()
        .find(|store| store.id == id)
        .map(|store| store.kind)
}

/// A spending flow by its category, `Daily` when it has none.
pub fn spending(group: Option<CategoryGroup>) -> FlowClass {
    match group {
        Some(CategoryGroup::Fixed) => FlowClass::Fixed,
        Some(CategoryGroup::Savings) => FlowClass::Savings,
        Some(CategoryGroup::Income) => FlowClass::Ignored,
        _ => FlowClass::Daily,
    }
}

/// A move between two sides, at least one of them the user's: across the
/// budget boundary, to a loan, to a goal, or out to someone.
pub fn classify_move(
    doc: &VaultDoc,
    from: &AccountRef,
    to: &AccountRef,
    group: Option<CategoryGroup>,
) -> FlowClass {
    match (from.store_id(), to.store_id()) {
        (Some(source), Some(target)) => {
            match (store_in_budget(doc, source), store_in_budget(doc, target)) {
                (true, true) | (false, false) => FlowClass::Ignored,
                (true, false) if store_kind(doc, target) == Some(StoreKind::Liability) => {
                    FlowClass::Fixed
                }
                (true, false) if goal_store_ids(doc).contains(target) => FlowClass::Savings,
                (true, false) => FlowClass::BudgetOut,
                (false, true) => FlowClass::BudgetIn,
            }
        }
        (Some(source), None) if store_in_budget(doc, source) => spending(group),
        (None, Some(target)) if store_in_budget(doc, target) => match group {
            Some(CategoryGroup::Income) => FlowClass::Income,
            _ => FlowClass::Ignored,
        },
        _ => FlowClass::Ignored,
    }
}

/// The one classification budgets, cash flow, month totals and the
/// calendar share (plan §7.3).
pub fn classify_flow(doc: &VaultDoc, tx: &Transaction) -> FlowClass {
    let group = category_group(doc, tx);
    if group == Some(CategoryGroup::Service) || tx.is_internal(|id| root_of(doc, id)) {
        return FlowClass::Ignored;
    }
    match tx {
        Transaction::Topup(inner) if store_in_budget(doc, &inner.store_id) => match group {
            Some(CategoryGroup::Income) => FlowClass::Income,
            _ => FlowClass::Ignored,
        },
        Transaction::Payment(inner) if store_in_budget(doc, &inner.store_id) => spending(group),
        Transaction::Transfer(inner) if inner.debt.is_none() => {
            classify_move(doc, &inner.from, &inner.to, group)
        }
        Transaction::Convert(inner) => {
            classify_move(doc, &inner.from.account, &inner.to.account, group)
        }
        Transaction::AssetSell(inner) => match &inner.proceeds_to {
            Some(target) if store_in_budget(doc, target) => FlowClass::BudgetIn,
            _ => FlowClass::Ignored,
        },
        _ => FlowClass::Ignored,
    }
}
