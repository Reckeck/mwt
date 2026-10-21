//! Passive income (plan D) and the freedom index (plan C3): what capital
//! brought in over the last year against mandatory spending, and what the
//! invested capital could bring at an expected rate.

use std::collections::{BTreeMap, HashMap};

use super::UnlockedVault;
use crate::calendar::{self, Calendar};
use crate::model::category::{
    CATEGORIES_REV, CATEGORY_ADDITIONS, Category, default_categories, is_passive_key,
};
use crate::model::store::Purpose;
use crate::model::transaction::Transaction;
use crate::pricing::history::secs_or_now;
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

impl UnlockedVault {
    /// Whether a category key books passive income: the user's flag on the
    /// category, or the default for keys with no category (`earn`).
    pub fn is_passive_category(&self, key: &str) -> bool {
        self.doc
            .categories
            .iter()
            .find(|category| category.key == key)
            .map_or_else(|| is_passive_key(key), Category::is_passive)
    }

    /// Passive income over the last finished months and the freedom index.
    /// `expected_pct` is the yearly return the potential assumes; `None`
    /// takes the key rate of today.
    pub fn passive_income(
        &self,
        now: i64,
        zone: &str,
        expected_pct: Option<Amount>,
    ) -> Result<PassiveIncome> {
        let cal = Calendar::new(zone);
        let today = cal.date_of(now);
        let currency = self.budget_currency();
        let book = self.rate_book();

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

        let investment: HashMap<&str, bool> = self
            .doc
            .stores
            .iter()
            .map(|store| (store.id.as_str(), store.purpose == Purpose::Investment))
            .collect();
        let mut per_month = vec![Amount::ZERO; bounds.len()];
        let mut by_category: BTreeMap<String, Amount> = BTreeMap::new();
        let mut reinvested = Amount::ZERO;
        for tx in &self.doc.transactions {
            let Transaction::Topup(_) = tx else { continue };
            let Some(category) = tx
                .meta()
                .categories
                .iter()
                .find(|key| self.is_passive_category(key))
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
                let Some(value) = book.convert(leg.amount, &leg.asset.symbol, &currency, secs)
                else {
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

        let fund = self.doc.emergency_fund.clone();
        let e_mand = self
            .income_basis(fund.avg_months as usize, now, zone)
            .map(|avg| avg.essential.round(2));
        let positive = e_mand.filter(|value| value.is_positive());
        let hundred = Amount::from(100u32);
        let twelve = Amount::from(PASSIVE_MONTHS);
        let freedom_fact_pct =
            positive.map(|value| (total_12m / twelve / value * hundred).round(1));

        let values = self.store_values_usd();
        let invested = self
            .doc
            .stores
            .iter()
            .filter(|store| !store.archived && !store.is_debt())
            .filter(|store| store.purpose == Purpose::Investment || store.deposit.is_some())
            .filter(|store| !fund.scope.store_ids.contains(&store.id))
            .filter_map(|store| values.get(&store.id))
            .filter_map(|usd| book.convert(*usd, "usd", &currency, now))
            .fold(Amount::ZERO, |sum, value| sum + value)
            .round(2);
        let expected_pct = expected_pct.or_else(|| self.key_rate_on(today));
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
    /// arrived does not come back. Returns whether the doc changed.
    pub(crate) fn ensure_default_categories(&mut self) -> bool {
        let seen = self.doc.settings.categories_rev;
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
                !self
                    .doc
                    .categories
                    .iter()
                    .any(|category| category.key == builtin.key)
            })
            .collect();
        self.doc.categories.extend(missing);
        self.doc.settings.categories_rev = CATEGORIES_REV;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::category::CategoryGroup;
    use crate::model::planning::EmergencyFund;
    use crate::model::store::{StoreKind, StoreScope};
    use crate::model::transaction::Transaction;
    use crate::pricing::history::unix_secs;
    use crate::service::test_support::*;

    fn booked(mut tx: Transaction, category: &str, ts: &str) -> Transaction {
        tx.meta_mut().categories = vec![category.into()];
        at(&mut tx, ts);
        tx
    }

    #[test]
    fn passive_follows_the_defaults_until_the_user_says_otherwise() {
        let (_dir, mut vault) = empty_vault();
        assert!(vault.is_passive_category("deposit_interest"));
        assert!(
            vault.is_passive_category("earn"),
            "Bybit Earn has no category record"
        );
        assert!(!vault.is_passive_category("cashback"));
        assert!(!vault.is_passive_category("salary"));
        vault
            .update_category("cashback", None, CategoryGroup::Income, None, Some(true))
            .unwrap();
        vault
            .update_category("dividends", None, CategoryGroup::Income, None, Some(false))
            .unwrap();
        assert!(vault.is_passive_category("cashback"));
        assert!(!vault.is_passive_category("dividends"));
        vault
            .update_category("cashback", None, CategoryGroup::Daily, None, None)
            .unwrap();
        assert!(
            !vault.is_passive_category("cashback"),
            "only income can be passive"
        );
    }

    #[test]
    fn old_vaults_get_the_new_categories_once() {
        let (_dir, mut vault) = empty_vault();
        vault.doc.categories.retain(|category| {
            !matches!(category.key.as_str(), "coupons" | "staking" | "rent_income")
        });
        let dividends = vault
            .doc
            .categories
            .iter_mut()
            .find(|category| category.key == "dividends")
            .unwrap();
        dividends.name = Some("Дивы".into());
        dividends.archived = true;
        vault.doc.settings.categories_rev = 0;
        let before = vault.doc.categories.len();

        assert!(vault.ensure_default_categories());
        assert_eq!(vault.doc.categories.len(), before + 3);
        let dividends = vault
            .doc
            .categories
            .iter()
            .find(|category| category.key == "dividends")
            .unwrap();
        assert_eq!(dividends.name.as_deref(), Some("Дивы"));
        assert!(dividends.archived);
        assert!(
            !vault.ensure_default_categories(),
            "a second open adds nothing"
        );
        assert_eq!(vault.doc.categories.len(), before + 3);

        // Deleted after its batch arrived: stays deleted.
        vault.delete_category("coupons").unwrap();
        assert!(!vault.ensure_default_categories());
        assert!(
            !vault
                .doc
                .categories
                .iter()
                .any(|category| category.key == "coupons")
        );
        // Only the batch's keys come back, not other deleted built-ins.
        vault.delete_category("taxi").unwrap();
        vault.doc.settings.categories_rev = 0;
        assert!(vault.ensure_default_categories());
        assert!(
            vault
                .doc
                .categories
                .iter()
                .any(|category| category.key == "coupons")
        );
        assert!(
            !vault
                .doc
                .categories
                .iter()
                .any(|category| category.key == "taxi")
        );
    }

    #[test]
    fn passive_income_and_the_freedom_index() {
        let (_dir, mut vault) = empty_vault();
        let bank = store(&mut vault, "Bank", StoreKind::Bank, None);
        let broker = store(&mut vault, "Broker", StoreKind::Exchange, None);
        vault.doc.stores.iter_mut().for_each(|store| {
            if store.id == broker {
                store.purpose = Purpose::Investment;
            }
        });
        vault.doc.emergency_fund.avg_months = 3;
        // Three finished months of mandatory spending: 1000 a month.
        for month in ["2026-07", "2026-08", "2026-09"] {
            let salary = income(&bank, "5000", "salary");
            vault
                .add_transaction(booked(salary, "salary", &format!("{month}-01T10:00:00Z")))
                .unwrap();
            let rent = payment(&bank, "1000", "rent");
            vault
                .add_transaction(booked(rent, "utilities", &format!("{month}-05T10:00:00Z")))
                .unwrap();
        }
        let interest = income(&bank, "60", "deposit_interest");
        vault
            .add_transaction(booked(interest, "deposit_interest", "2026-08-10T10:00:00Z"))
            .unwrap();
        let dividend = income(&broker, "60", "dividends");
        vault
            .add_transaction(booked(dividend, "dividends", "2026-09-10T10:00:00Z"))
            .unwrap();
        let early = income(&bank, "999", "deposit_interest");
        vault
            .add_transaction(booked(early, "deposit_interest", "2025-09-30T10:00:00Z"))
            .unwrap();
        let current = income(&bank, "7", "staking");
        vault
            .add_transaction(booked(current, "staking", "2026-10-02T10:00:00Z"))
            .unwrap();
        let cashback = income(&bank, "50", "cashback");
        vault
            .add_transaction(booked(cashback, "cashback", "2026-09-12T10:00:00Z"))
            .unwrap();

        let now = unix_secs("2026-10-15T12:00:00Z").unwrap();
        let report = vault
            .passive_income(now, "UTC", Some(amount("12")))
            .unwrap();
        assert_eq!(report.months.len(), 12);
        assert_eq!(report.months[0].month, "2025-10");
        assert_eq!(report.months[11].month, "2026-09");
        assert_eq!(report.total_12m, amount("120"));
        assert_eq!(report.this_month, amount("7"));
        assert_eq!(report.reinvested_12m, amount("60"));
        assert_eq!(report.by_category.len(), 2);
        assert_eq!(report.e_mand, Some(amount("1000")));
        // 120 / 12 = 10 a month against 1000.
        assert_eq!(report.freedom_fact_pct, Some(amount("1")));

        // The broker's 60 at 12% a year: 0.6 a month against 1000 = 0.06%.
        assert_eq!(report.invested, amount("60"));
        assert_eq!(report.freedom_potential_pct, Some(amount("0.1")));

        // An emergency fund account stays out of the potential.
        vault.doc.emergency_fund = EmergencyFund {
            scope: StoreScope {
                store_ids: vec![broker.clone()],
                include_children: false,
            },
            ..EmergencyFund::default()
        };
        let report = vault
            .passive_income(now, "UTC", Some(amount("12")))
            .unwrap();
        assert_eq!(report.invested, Amount::ZERO);
        assert_eq!(report.total_12m, amount("120"), "its income still counts");
    }
}
