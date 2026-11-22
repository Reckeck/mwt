//! The emergency fund (plan C1): savings accounts set aside for bad times,
//! measured in months of mandatory spending `E_mand`.

use mwt_base::calendar::{self, Calendar};
use mwt_base::time::secs_or_now;
use mwt_base::{Amount, Error, Result};
use mwt_model::EmergencyFund;
use mwt_model::VaultDoc;
use mwt_model::{Purpose, Store};
use mwt_money::books::Books;
use mwt_money::interest::INTEREST_CATEGORY;

use super::budget::{MonthAvg, budget_currency, income_basis};
use super::goals::{resolve_scope, store_values_usd};

const YEAR_SECS: i64 = 365 * 86_400;

/// The longest target and averaging window the fund accepts, months.
pub const MAX_FUND_MONTHS: u8 = 24;

/// One account of the fund, in the budget currency.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct EmergencyStore {
    pub store_id: String,
    pub value: Amount,
    /// Deposit rate in force today, percent; `None` without deposit terms.
    pub rate: Option<Amount>,
    /// Inflation of the account's currency this month, percent.
    pub inflation: Option<Amount>,
    /// The rate is known and below inflation: a hint, not a warning light.
    pub below_inflation: bool,
    /// Interest booked here over the last twelve months.
    pub interest_12m: Amount,
    /// Active goals that count this account too (decision 34).
    pub goal_ids: Vec<String>,
}

/// The fund at a glance (plan C1).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct EmergencyStatus {
    pub fund: EmergencyFund,
    /// The budget currency every amount here is in.
    pub currency: String,
    pub stores: Vec<EmergencyStore>,
    pub total: Amount,
    /// Average monthly mandatory spending; `None` without history or a
    /// hand-entered month.
    pub e_mand: Option<Amount>,
    /// Finished months behind `e_mand`; 0 when it is the hand-entered month.
    pub e_mand_months: u32,
    /// `total / e_mand`; `None` when `e_mand` is unknown or zero.
    pub months_covered: Option<Amount>,
    /// `target_months × e_mand`.
    pub target_amount: Option<Amount>,
    /// What lies above the target and could be invested (decision 33).
    pub surplus: Option<Amount>,
    pub interest_12m: Amount,
    /// Savings accounts not in the fund yet, for "Make it the fund".
    pub candidates: Vec<String>,
}

fn fits_fund(store: &Store) -> bool {
    store.purpose == Purpose::Savings && !store.archived
}

/// What the fund is now.
pub fn emergency_fund(doc: &VaultDoc) -> EmergencyFund {
    doc.emergency_fund.clone()
}

/// The fund that replaces the stored one. Only live savings accounts fit
/// (decision 29); an empty list removes the fund.
pub fn plan_set_emergency_fund(doc: &VaultDoc, fund: EmergencyFund) -> Result<EmergencyFund> {
    let months = 1..=MAX_FUND_MONTHS;
    if !months.contains(&fund.target_months) || !months.contains(&fund.avg_months) {
        return Err(Error::InvalidInput(format!(
            "emergency fund months: expected 1..={MAX_FUND_MONTHS}"
        )));
    }
    let mut ids: Vec<String> = Vec::new();
    for id in &fund.scope.store_ids {
        let store = doc
            .stores
            .iter()
            .find(|store| &store.id == id)
            .ok_or_else(|| Error::Missing(format!("store {id}")))?;
        if !fits_fund(store) {
            return Err(Error::InvalidInput(format!(
                "store {id}: only active savings accounts can be the emergency fund"
            )));
        }
        if !ids.contains(id) {
            ids.push(id.clone());
        }
    }
    let mut next = fund;
    next.scope.store_ids = ids;
    next.scope.include_children = false;
    Ok(next)
}

/// The stored fund with one account added or removed, the switch in the
/// store form; not checked yet.
pub fn toggled_fund(doc: &VaultDoc, store_id: &str, on: bool) -> EmergencyFund {
    let mut fund = doc.emergency_fund.clone();
    fund.scope.store_ids.retain(|id| id != store_id);
    if on {
        fund.scope.store_ids.push(store_id.to_string());
    }
    fund
}

/// Drop accounts that stopped being live savings (decision 36). The caller
/// saves.
pub fn prune_emergency_fund(doc: &mut VaultDoc) {
    let keep: Vec<String> = doc
        .emergency_fund
        .scope
        .store_ids
        .iter()
        .filter(|id| {
            doc.stores
                .iter()
                .find(|store| &store.id == *id)
                .is_some_and(fits_fund)
        })
        .cloned()
        .collect();
    doc.emergency_fund.scope.store_ids = keep;
}

/// The fund's value, coverage and hints. `basis` is the monthly basis of
/// `fund.avg_months` months (the income basis, hand-entered figures included,
/// as for the other readers); `interest_12m_of` is what a store booked as
/// interest over the last twelve months, given its id and the budget currency.
pub fn fund_status(
    books: &Books<'_>,
    basis: Option<MonthAvg>,
    interest_12m_of: impl Fn(&str, &str) -> Amount,
    now: i64,
    zone: &str,
) -> EmergencyStatus {
    let doc = books.doc();
    let fund = doc.emergency_fund.clone();
    let cal = Calendar::new(zone);
    let today = cal.date_of(now);
    let today_raw = today.format("%Y-%m-%d").to_string();
    let month = calendar::month_key(today);
    let currency = budget_currency(books);
    let book = books.rate_book();
    let values = store_values_usd(books);
    let to_budget = |usd: Amount| {
        book.convert(usd, "usd", &currency, now)
            .unwrap_or(Amount::ZERO)
            .round(2)
    };

    let stores: Vec<EmergencyStore> = fund
        .scope
        .store_ids
        .iter()
        .filter_map(|id| doc.stores.iter().find(|store| &store.id == id))
        .map(|store| {
            let rate = store
                .deposit
                .as_ref()
                .and_then(|terms| terms.rate_on(&today_raw));
            let own_currency = mwt_money::interest::deposit_asset(books, store).symbol.to_lowercase();
            let inflation = mwt_money::inflation::inflation_on(books.doc(), &own_currency, &month);
            EmergencyStore {
                store_id: store.id.clone(),
                value: to_budget(values.get(&store.id).copied().unwrap_or_default()),
                rate,
                inflation,
                below_inflation: matches!((rate, inflation), (Some(rate), Some(infl)) if rate < infl),
                interest_12m: interest_12m_of(&store.id, &currency),
                goal_ids: goals_counting(books, &store.id),
            }
        })
        .collect();
    let total = stores
        .iter()
        .fold(Amount::ZERO, |sum, store| sum + store.value);
    let interest_12m = stores
        .iter()
        .fold(Amount::ZERO, |sum, store| sum + store.interest_12m);

    let e_mand = basis.as_ref().map(|avg| avg.essential.round(2));
    let positive = e_mand.filter(|value| value.is_positive());
    let target_amount =
        e_mand.map(|value| (value * Amount::from(fund.target_months as u32)).round(2));
    let months_covered = positive
        .filter(|_| fund.is_set())
        .map(|value| (total / value).round(1));
    let surplus = target_amount
        .filter(|_| positive.is_some() && fund.is_set())
        .map(|target| total - target)
        .filter(|value| value.is_positive());
    let candidates = doc
        .stores
        .iter()
        .filter(|store| fits_fund(store) && !fund.scope.store_ids.contains(&store.id))
        .map(|store| store.id.clone())
        .collect();

    EmergencyStatus {
        currency,
        stores,
        total,
        e_mand,
        e_mand_months: basis.as_ref().map_or(0, |avg| avg.months),
        months_covered,
        target_amount,
        surplus,
        interest_12m,
        candidates,
        fund,
    }
}

/// Active goals with an explicit scope that includes the store.
fn goals_counting(books: &Books<'_>, store_id: &str) -> Vec<String> {
    books
        .doc()
        .goals
        .iter()
        .filter(|goal| goal.active && !goal.scope.store_ids.is_empty())
        .filter(|goal| resolve_scope(books, &goal.scope).contains(store_id))
        .map(|goal| goal.id.clone())
        .collect()
}

/// The fund's value, coverage and hints.
pub fn emergency_fund_status(books: &Books<'_>, now: i64, zone: &str) -> EmergencyStatus {
    let basis = income_basis(
        books,
        books.doc().emergency_fund.avg_months as usize,
        now,
        zone,
    );
    fund_status(
        books,
        basis,
        |store_id, currency| interest_since(books, store_id, currency, now - YEAR_SECS, now),
        now,
        zone,
    )
}

/// Interest booked into a store in `[from, to)`, in `currency`.
fn interest_since(books: &Books<'_>, store_id: &str, currency: &str, from: i64, to: i64) -> Amount {
    let book = books.rate_book();
    books
        .doc()
        .transactions
        .iter()
        .filter(|tx| {
            tx.meta()
                .categories
                .iter()
                .any(|key| key == INTEREST_CATEGORY)
        })
        .filter_map(|tx| {
            let secs = secs_or_now(&tx.meta().timestamp);
            (secs >= from && secs < to).then_some((tx, secs))
        })
        .flat_map(|(tx, secs)| tx.legs().into_iter().map(move |leg| (leg, secs)))
        .filter(|(leg, _)| leg.store_id == store_id && leg.amount.is_positive())
        .filter_map(|(leg, secs)| book.convert(leg.amount, &leg.asset.symbol, currency, secs))
        .fold(Amount::ZERO, |sum, value| sum + value)
        .round(2)
}
