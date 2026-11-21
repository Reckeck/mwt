//! Passive income (plan D) and the freedom index (plan C3): what capital
//! brought in over the last year against mandatory spending, and what the
//! invested capital could bring at an expected rate.

use std::collections::{BTreeMap, HashMap};

use super::budget::{budget_currency, income_basis};
use crate::calendar::{self, Calendar};
use crate::kernel::books::Books;
use crate::model::category::{
    CATEGORIES_REV, CATEGORY_ADDITIONS, Category, default_categories, is_passive_key,
};
use crate::model::store::Purpose;
use crate::model::transaction::Transaction;
use crate::model::vault::VaultDoc;
use crate::time::secs_or_now;
use crate::{Amount, Result};

/// Finished months the passive income and the fact index are measured over.
const PASSIVE_MONTHS: u32 = 12;

/// Passive income booked in one finished month.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct PassiveMonth {
    /// `YYYY-MM`.
    pub month: String,
    pub amount: Amount,
}

/// Passive income from one category over the year.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct PassiveShare {
    pub category: String,
    pub amount: Amount,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct PassiveIncome {
    /// The budget currency every amount is in.
    pub currency: String,
    /// The last finished months, oldest first.
    pub months: Vec<PassiveMonth>,
    pub total_12m: Amount,
    /// The part that came into investment accounts: reinvested, not spent.
    pub reinvested_12m: Amount,
    pub by_category: Vec<PassiveShare>,
    /// So far in the current month.
    pub this_month: Amount,
    /// Mandatory monthly spending the index is measured against.
    pub e_mand: Option<Amount>,
    /// Passive income per month as a share of `e_mand`, in percent.
    pub freedom_fact_pct: Option<Amount>,
    /// Investment accounts and deposits, emergency fund accounts left out.
    pub invested: Amount,
    /// The yearly rate the potential assumes.
    pub expected_pct: Option<Amount>,
    /// What `invested` would bring at `expected_pct`, as a share of `e_mand`.
    pub freedom_potential_pct: Option<Amount>,
}

use super::goals::store_values_usd;

/// Whether a category key books passive income: the user's flag on the
/// category, or the default for keys with no category (`earn`).
pub fn is_passive_category(doc: &VaultDoc, key: &str) -> bool {
    doc.categories
        .iter()
        .find(|category| category.key == key)
        .map_or_else(|| is_passive_key(key), Category::is_passive)
}

/// Passive income over the last finished months and the freedom index.
/// `expected_pct` is the yearly return the potential assumes; `None`
/// takes the key rate of today.
pub fn passive_income(
    books: &Books<'_>,
    now: i64,
    zone: &str,
    expected_pct: Option<Amount>,
) -> Result<PassiveIncome> {
    let doc = books.doc();
    let cal = Calendar::new(zone);
    let today = cal.date_of(now);
    let currency = budget_currency(books);
    let book = books.rate_book();

    let this_start = calendar::first_of_month(today);
    let mut starts = vec![this_start];
    for _ in 0..PASSIVE_MONTHS {
        let previous =
            calendar::first_of_month(starts[starts.len() - 1] - chrono::Duration::days(1));
        starts.push(previous);
    }
    starts.reverse();
    let bounds: Vec<i64> = starts.iter().map(|day| cal.start_of(*day)).collect();
    let year_from = bounds[0];
    let month_of = |secs: i64| bounds.iter().rposition(|start| secs >= *start);

    let investment: HashMap<&str, bool> = doc
        .stores
        .iter()
        .map(|store| (store.id.as_str(), store.purpose == Purpose::Investment))
        .collect();
    let mut per_month = vec![Amount::ZERO; bounds.len()];
    let mut by_category: BTreeMap<String, Amount> = BTreeMap::new();
    let mut reinvested = Amount::ZERO;
    for tx in &doc.transactions {
        let Transaction::Topup(_) = tx else { continue };
        let Some(category) = tx
            .meta()
            .categories
            .iter()
            .find(|key| is_passive_category(doc, key))
        else {
            continue;
        };
        let secs = secs_or_now(&tx.meta().timestamp);
        if secs < year_from || secs > now {
            continue;
        }
        let Some(slot) = month_of(secs) else { continue };
        for leg in tx.legs() {
            if !leg.amount.is_positive() {
                continue;
            }
            let Some(value) = book.convert(leg.amount, &leg.asset.symbol, &currency, secs) else {
                continue;
            };
            per_month[slot] += value;
            if slot + 1 < bounds.len() {
                *by_category.entry(category.clone()).or_default() += value;
                if investment
                    .get(leg.store_id.as_str())
                    .copied()
                    .unwrap_or(false)
                {
                    reinvested += value;
                }
            }
        }
    }

    let this_month = per_month.pop().unwrap_or_default().round(2);
    let months: Vec<PassiveMonth> = starts
        .iter()
        .zip(per_month)
        .map(|(start, amount)| PassiveMonth {
            month: calendar::month_key(*start),
            amount: amount.round(2),
        })
        .collect();
    let total_12m = months
        .iter()
        .fold(Amount::ZERO, |sum, month| sum + month.amount);
    let mut by_category: Vec<PassiveShare> = by_category
        .into_iter()
        .map(|(category, amount)| PassiveShare {
            category,
            amount: amount.round(2),
        })
        .collect();
    by_category.sort_by_key(|share| std::cmp::Reverse(share.amount));

    let fund = &doc.emergency_fund;
    let e_mand =
        income_basis(books, fund.avg_months as usize, now, zone).map(|avg| avg.essential.round(2));
    let positive = e_mand.filter(|value| value.is_positive());
    let hundred = Amount::from(100u32);
    let twelve = Amount::from(PASSIVE_MONTHS);
    let freedom_fact_pct = positive.map(|value| (total_12m / twelve / value * hundred).round(1));

    let values = store_values_usd(books);
    let invested = doc
        .stores
        .iter()
        .filter(|store| !store.archived && !store.is_debt())
        .filter(|store| store.purpose == Purpose::Investment || store.deposit.is_some())
        .filter(|store| !fund.scope.store_ids.contains(&store.id))
        .filter_map(|store| values.get(&store.id))
        .filter_map(|usd| book.convert(*usd, "usd", &currency, now))
        .fold(Amount::ZERO, |sum, value| sum + value)
        .round(2);
    let expected_pct =
        expected_pct.or_else(|| crate::money::interest::key_rate_on(books.doc(), today));
    let freedom_potential_pct = match (positive, expected_pct) {
        (Some(value), Some(rate)) => {
            Some((invested * rate / hundred / twelve / value * hundred).round(1))
        }
        _ => None,
    };

    Ok(PassiveIncome {
        currency,
        months,
        total_12m,
        reinvested_12m: reinvested.round(2),
        by_category,
        this_month,
        e_mand,
        freedom_fact_pct,
        invested,
        expected_pct,
        freedom_potential_pct,
    })
}

/// Add the built-in categories of every batch the vault has not had yet
/// ([`CATEGORY_ADDITIONS`]), skipping keys it already has: archived and
/// renamed ones stay as they are, and a category deleted after its batch
/// arrived (here or on another device, by its tombstone) does not come
/// back. Returns whether the doc changed.
pub(crate) fn ensure_default_categories(doc: &mut VaultDoc) -> bool {
    let seen = doc.settings.categories_rev;
    if seen >= CATEGORIES_REV {
        return false;
    }
    let wanted: Vec<&str> = CATEGORY_ADDITIONS
        .iter()
        .filter(|(rev, _)| *rev > seen)
        .flat_map(|(_, keys)| keys.iter().copied())
        .collect();
    let missing: Vec<Category> = default_categories()
        .into_iter()
        .filter(|builtin| wanted.contains(&builtin.key.as_str()))
        .filter(|builtin| {
            !doc.categories
                .iter()
                .any(|category| category.key == builtin.key)
        })
        // Deleted on a device whose edit reached this one: a late update
        // must not bring it back with a stamp newer than the tombstone.
        .filter(|builtin| {
            !doc.sync
                .tombstones
                .contains_key(&format!("categories:{}", builtin.key))
        })
        .collect();
    doc.categories.extend(missing);
    doc.settings.categories_rev = CATEGORIES_REV;
    true
}
