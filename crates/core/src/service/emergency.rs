//! The emergency fund (plan C1): savings accounts set aside for bad times,
//! measured in months of mandatory spending `E_mand`.

use super::UnlockedVault;
use super::interest::INTEREST_CATEGORY;
use crate::calendar::{self, Calendar};
use crate::model::planning::EmergencyFund;
use crate::model::store::{Purpose, Store};
use crate::pricing::history::secs_or_now;
use crate::{Amount, Error, Result};

/// The longest target and averaging window the fund accepts, months.
pub const MAX_FUND_MONTHS: u8 = 24;
const YEAR_SECS: i64 = 365 * 86_400;

/// One account of the fund, in the budget currency.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
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
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
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

impl UnlockedVault {
    pub fn emergency_fund(&self) -> EmergencyFund {
        self.doc.emergency_fund.clone()
    }

    /// Replace the fund. Only live savings accounts fit (decision 29); an
    /// empty list removes the fund.
    pub fn set_emergency_fund(&mut self, fund: EmergencyFund) -> Result<EmergencyFund> {
        let months = 1..=MAX_FUND_MONTHS;
        if !months.contains(&fund.target_months) || !months.contains(&fund.avg_months) {
            return Err(Error::InvalidInput(format!(
                "emergency fund months: expected 1..={MAX_FUND_MONTHS}"
            )));
        }
        let mut ids: Vec<String> = Vec::new();
        for id in &fund.scope.store_ids {
            let store = self
                .find_store(id)
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
        self.doc.emergency_fund = next.clone();
        self.persist()?;
        Ok(next)
    }

    /// Add or remove one account, the switch in the store form.
    pub fn emergency_fund_toggle(&mut self, store_id: &str, on: bool) -> Result<EmergencyFund> {
        let mut fund = self.doc.emergency_fund.clone();
        fund.scope.store_ids.retain(|id| id != store_id);
        if on {
            fund.scope.store_ids.push(store_id.to_string());
        }
        self.set_emergency_fund(fund)
    }

    /// Drop accounts that stopped being live savings (decision 36). Callers
    /// persist.
    pub(crate) fn prune_emergency_fund(&mut self) {
        let keep: Vec<String> = self
            .doc
            .emergency_fund
            .scope
            .store_ids
            .iter()
            .filter(|id| self.find_store(id).is_some_and(fits_fund))
            .cloned()
            .collect();
        self.doc.emergency_fund.scope.store_ids = keep;
    }

    /// The fund's value, coverage and hints. `manual` is the hand-entered
    /// inflation table, as for the other inflation readers.
    pub fn emergency_fund_status(&self, now: i64, zone: &str) -> EmergencyStatus {
        let fund = self.doc.emergency_fund.clone();
        let cal = Calendar::new(zone);
        let today = cal.date_of(now);
        let today_raw = today.format("%Y-%m-%d").to_string();
        let month = calendar::month_key(today);
        let currency = self.budget_currency();
        let book = self.rate_book();
        let values = self.store_values_usd();
        let to_budget = |usd: Amount| {
            book.convert(usd, "usd", &currency, now)
                .unwrap_or(Amount::ZERO)
                .round(2)
        };

        let stores: Vec<EmergencyStore> = fund
            .scope
            .store_ids
            .iter()
            .filter_map(|id| self.find_store(id))
            .map(|store| {
                let rate = store
                    .deposit
                    .as_ref()
                    .and_then(|terms| terms.rate_on(&today_raw));
                let own_currency = self.deposit_asset(store).symbol.to_lowercase();
                let inflation = self.inflation_on(&own_currency, &month);
                EmergencyStore {
                    store_id: store.id.clone(),
                    value: to_budget(values.get(&store.id).copied().unwrap_or_default()),
                    rate,
                    inflation,
                    below_inflation: matches!((rate, inflation), (Some(rate), Some(infl)) if rate < infl),
                    interest_12m: self.interest_since(&store.id, &currency, now - YEAR_SECS, now),
                    goal_ids: self.goals_counting(&store.id),
                }
            })
            .collect();
        let total = stores
            .iter()
            .fold(Amount::ZERO, |sum, store| sum + store.value);
        let interest_12m = stores
            .iter()
            .fold(Amount::ZERO, |sum, store| sum + store.interest_12m);

        let basis = self.income_basis(fund.avg_months as usize, now, zone);
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
        let candidates = self
            .doc
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

    /// Interest booked into a store in `[from, to)`, in `currency`.
    fn interest_since(&self, store_id: &str, currency: &str, from: i64, to: i64) -> Amount {
        let book = self.rate_book();
        self.doc
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

    /// Active goals with an explicit scope that includes the store.
    fn goals_counting(&self, store_id: &str) -> Vec<String> {
        self.doc
            .goals
            .iter()
            .filter(|goal| goal.active && !goal.scope.store_ids.is_empty())
            .filter(|goal| self.resolve_scope(&goal.scope).contains(store_id))
            .map(|goal| goal.id.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::category::CategoryGroup;
    use crate::model::deposit::{Capitalization, DepositTerms, RatePeriod};
    use crate::model::store::{Binding, StoreKind, StoreScope, SubType};
    use crate::model::transaction::{AccountRef, Transaction};
    use crate::pricing::history::unix_secs;
    use crate::service::StoreExtras;
    use crate::service::test_support::*;

    fn savings(vault: &mut UnlockedVault, name: &str) -> String {
        let bank = store(vault, &format!("{name} bank"), StoreKind::Bank, None);
        let extras = StoreExtras {
            sub_type: Some(SubType::Savings),
            binding: Some(Binding::Asset(usd())),
            ..StoreExtras::default()
        };
        vault
            .create_store_with(name, StoreKind::Bank, "#000", "", Some(bank), extras)
            .unwrap()
            .id
    }

    fn dated(mut tx: Transaction, ts: &str) -> Transaction {
        at(&mut tx, ts);
        tx
    }

    fn spend(store: &str, raw: &str, category: &str, ts: &str) -> Transaction {
        let mut tx = payment(store, raw, category);
        tx.meta_mut().categories = vec![category.into()];
        dated(tx, ts)
    }

    fn fund_of(ids: &[&String]) -> EmergencyFund {
        EmergencyFund {
            scope: StoreScope {
                store_ids: ids.iter().map(|id| id.to_string()).collect(),
                include_children: false,
            },
            ..EmergencyFund::default()
        }
    }

    #[test]
    fn essential_follows_the_defaults_until_the_user_says_otherwise() {
        let (_dir, mut vault) = empty_vault();
        let find = |vault: &UnlockedVault, key: &str| {
            vault
                .doc
                .categories
                .iter()
                .find(|category| category.key == key)
                .unwrap()
                .clone()
        };
        assert!(find(&vault, "groceries").is_essential());
        assert!(!find(&vault, "entertainment").is_essential());
        assert!(find(&vault, "utilities").is_essential());
        assert!(!find(&vault, "salary").is_essential());

        vault
            .update_category("groceries", None, CategoryGroup::Daily, Some(false), None)
            .unwrap();
        vault
            .update_category(
                "entertainment",
                None,
                CategoryGroup::Daily,
                Some(true),
                None,
            )
            .unwrap();
        assert!(!find(&vault, "groceries").is_essential());
        assert!(find(&vault, "entertainment").is_essential());
        // a rename without the flag keeps the choice
        vault
            .update_category(
                "entertainment",
                Some("Fun"),
                CategoryGroup::Daily,
                None,
                None,
            )
            .unwrap();
        assert!(find(&vault, "entertainment").is_essential());
        // a mandatory category is mandatory whatever the flag says
        vault
            .update_category("utilities", None, CategoryGroup::Fixed, Some(false), None)
            .unwrap();
        assert!(find(&vault, "utilities").is_essential());
    }

    #[test]
    fn mandatory_spending_is_fixed_plus_essential_everyday() {
        let (_dir, mut vault) = empty_vault();
        vault.set_base_currency("usd");
        quote_usd(&mut vault);
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        for month in ["01", "02"] {
            let ts = format!("2026-{month}-10T10:00:00Z");
            for (raw, category) in [
                ("500", "rent_mortgage"),
                ("300", "groceries"),
                ("200", "entertainment"),
            ] {
                vault
                    .add_transaction(spend(&card, raw, category, &ts))
                    .unwrap();
            }
        }
        let summary = vault.month_summary("2026-01", "UTC").unwrap();
        assert_eq!(summary.fixed, amount("500"));
        assert_eq!(summary.daily, amount("500"));
        assert_eq!(summary.essential_daily, amount("300"));

        let now = unix_secs("2026-03-05T10:00:00Z").unwrap();
        let avg = vault.avg_month(6, now, "UTC").unwrap();
        assert_eq!(avg.essential, amount("800"));
    }

    #[test]
    fn the_fund_takes_only_live_savings_accounts() {
        let (_dir, mut vault) = empty_vault();
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        let reserve = savings(&mut vault, "Reserve");
        assert!(vault.set_emergency_fund(fund_of(&[&card])).is_err());
        let mut long = fund_of(&[&reserve]);
        long.target_months = 25;
        assert!(vault.set_emergency_fund(long).is_err());
        let mut zero = fund_of(&[&reserve]);
        zero.avg_months = 0;
        assert!(vault.set_emergency_fund(zero).is_err());

        let saved = vault
            .set_emergency_fund(fund_of(&[&reserve, &reserve]))
            .unwrap();
        assert_eq!(saved.scope.store_ids, vec![reserve.clone()]);
        assert!(!saved.scope.include_children);
        assert!(vault.emergency_fund_toggle(&card, true).is_err());
    }

    #[test]
    fn switching_off_the_last_account_leaves_no_fund() {
        let (_dir, mut vault) = empty_vault();
        let first = savings(&mut vault, "First");
        let second = savings(&mut vault, "Second");
        vault.emergency_fund_toggle(&first, true).unwrap();
        let both = vault.emergency_fund_toggle(&second, true).unwrap();
        assert_eq!(both.scope.store_ids, vec![first.clone(), second.clone()]);
        vault.emergency_fund_toggle(&first, false).unwrap();
        let none = vault.emergency_fund_toggle(&second, false).unwrap();
        assert!(!none.is_set());
        assert_eq!(none.target_months, 6);
        // an empty fund is not «every store»
        let status = vault.emergency_fund_status(0, "UTC");
        assert!(status.stores.is_empty());
        assert_eq!(status.months_covered, None);
    }

    #[test]
    fn a_new_purpose_or_the_archive_takes_the_account_out() {
        let (_dir, mut vault) = empty_vault();
        let first = savings(&mut vault, "First");
        let second = savings(&mut vault, "Second");
        vault
            .set_emergency_fund(fund_of(&[&first, &second]))
            .unwrap();
        vault
            .update_store(
                &first,
                "First",
                "#000",
                "",
                StoreExtras {
                    purpose: Some(Purpose::Everyday),
                    ..StoreExtras::default()
                },
            )
            .unwrap();
        assert_eq!(vault.emergency_fund().scope.store_ids, vec![second.clone()]);
        vault.archive_store(&second, true).unwrap();
        assert!(!vault.emergency_fund().is_set());
    }

    #[test]
    fn coverage_surplus_interest_and_goal_overlap() {
        let (_dir, mut vault) = empty_vault();
        vault.set_base_currency("usd");
        quote_usd(&mut vault);
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        let reserve = savings(&mut vault, "Reserve");
        let spare = savings(&mut vault, "Spare");
        vault
            .set_deposit_terms(
                &reserve,
                Some(DepositTerms {
                    rates: vec![RatePeriod {
                        from: "2025-01-01".into(),
                        rate: amount("2"),
                    }],
                    capitalization: Capitalization::Monthly,
                    opened_at: None,
                    term_months: None,
                    auto_accrue: false,
                    pds: None,
                }),
            )
            .unwrap();
        for month in ["01", "02", "03"] {
            let ts = format!("2026-{month}-10T10:00:00Z");
            vault
                .add_transaction(spend(&card, "1000", "rent_mortgage", &ts))
                .unwrap();
        }
        vault
            .add_transaction(dated(
                transfer_between(
                    AccountRef::Store(card.clone()),
                    AccountRef::Store(reserve.clone()),
                    usd(),
                    "7000",
                ),
                "2026-03-20T10:00:00Z",
            ))
            .unwrap();
        let mut interest = income(&reserve, "80", "interest");
        interest.meta_mut().categories = vec![INTEREST_CATEGORY.into()];
        vault
            .add_transaction(dated(interest, "2026-03-31T10:00:00Z"))
            .unwrap();
        vault.emergency_fund_toggle(&reserve, true).unwrap();
        let goal = vault
            .create_goal(crate::service::GoalDraft {
                name: "Trip".into(),
                active: true,
                target_amount: amount("10000"),
                currency: "usd".into(),
                starts_at: "2026-01-01T00:00:00Z".into(),
                deadline: None,
                priority: 3,
                monthly_commitment: None,
                scope: StoreScope::store(&reserve, false),
                scenario_id: None,
                expected_return: None,
            })
            .unwrap();

        let now = unix_secs("2026-04-05T10:00:00Z").unwrap();
        let status = vault.emergency_fund_status(now, "UTC");
        assert_eq!(status.total, amount("7080"));
        assert_eq!(status.e_mand, Some(amount("1000")));
        assert_eq!(status.months_covered, Some(amount("7.1")));
        assert_eq!(status.target_amount, Some(amount("6000")));
        assert_eq!(status.surplus, Some(amount("1080")));
        assert_eq!(status.interest_12m, amount("80"));
        assert_eq!(status.candidates, vec![spare]);
        let card_row = &status.stores[0];
        assert_eq!(card_row.rate, Some(amount("2")));
        assert!(card_row.below_inflation);
        assert_eq!(card_row.goal_ids, vec![goal.id]);
    }
}
