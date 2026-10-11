//! Interest on savings accounts and term deposits, the ПДС state
//! co-financing, and the tax on deposit interest (НДФЛ above the limit of
//! 1 000 000 ₽ × the highest key rate of the year).
//!
//! Interest is computed day by day on the real balance of the account, at the
//! rate in force that day. Posted interest is an ordinary top-up in the
//! `deposit_interest` category, so it raises the balance like a bank's
//! capitalization does; the projection compounds by the account's terms.

use std::collections::BTreeMap;

use chrono::{Datelike, Duration, NaiveDate};
use rust_decimal::Decimal;

use crate::Amount;
use crate::error::{Error, Result};
use crate::model::deposit::{Capitalization, DepositTerms, IncomeBracket, KeyRatePoint};
use crate::model::store::{AssetRef, Binding, Store, StoreKind, SubType};
use crate::model::transaction::{KindDetails, PriceStamp, TopupTx, Transaction, TxMeta, TxOrigin};
use crate::pricing::history::secs_or_now;

use super::UnlockedVault;

/// Category of posted interest and of the programme's co-financing.
pub const INTEREST_CATEGORY: &str = "deposit_interest";
/// Tax-free deposit income is this many rubles × the key rate (НК РФ ст. 214.2).
pub const TAX_FREE_PRINCIPAL: i64 = 1_000_000;
/// НДФЛ on the excess, percent.
pub const NDFL_RATE_PCT: i64 = 13;
/// ПДС: the state adds at most this much a year…
pub const PDS_MAX_COFINANCING: i64 = 36_000;
/// …to someone who put in at least this much that year…
pub const PDS_MIN_CONTRIBUTION: i64 = 2_000;
/// …for this many years from the first contribution.
pub const PDS_YEARS: i32 = 10;
/// The co-financing for a year arrives by this day of the next year.
pub const PDS_DUE_MONTH_DAY: (u32, u32) = (8, 31);
/// ПДС contributions are deductible up to this much a year.
pub const PDS_DEDUCTION_CAP: i64 = 400_000;

/// Origin tag of interest the app posted itself, so a month is posted once.
const INTEREST_ORIGIN_PREFIX: &str = "interest:";

fn amount(raw: i64) -> Amount {
    Amount::new(Decimal::from(raw))
}

fn day_of(raw: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(raw.get(..10)?, "%Y-%m-%d").ok()
}

fn days_in_year(year: i32) -> i64 {
    if NaiveDate::from_ymd_opt(year, 2, 29).is_some() {
        366
    } else {
        365
    }
}

fn month_start(day: NaiveDate) -> NaiveDate {
    day.with_day(1).unwrap_or(day)
}

fn next_month(day: NaiveDate) -> NaiveDate {
    let start = month_start(day);
    if start.month() == 12 {
        NaiveDate::from_ymd_opt(start.year() + 1, 1, 1).unwrap_or(start)
    } else {
        NaiveDate::from_ymd_opt(start.year(), start.month() + 1, 1).unwrap_or(start)
    }
}

/// Where an account stands and what it will earn.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct DepositReport {
    pub store_id: String,
    /// Lowercase code of the account's money.
    pub currency: String,
    pub balance: Amount,
    /// Rate in force today, percent; `None` before the first rate.
    pub rate: Option<Amount>,
    pub rate_since: Option<String>,
    /// Interest earned this month so far, by the day.
    pub accrued_month: Amount,
    /// Interest earned since the 1st of January, by the day.
    pub accrued_year: Amount,
    /// Interest actually booked this year (`deposit_interest` top-ups).
    pub posted_year: Amount,
    /// The next twelve months at today's rate, compounded by the terms.
    pub projected_year: Amount,
    /// Term deposits: the last day and the balance then.
    pub matures_at: Option<String>,
    pub at_maturity: Option<Amount>,
    pub pds: Option<PdsReport>,
}

/// The ПДС year at a glance.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct PdsReport {
    pub year: i32,
    /// Put in this year (top-ups and transfers in, without interest and co-financing).
    pub contributions: Amount,
    /// What the state adds for this year as things stand.
    pub cofinancing: Amount,
    /// How much more to put in to get the full 36 000 ₽.
    pub to_full_cofinancing: Amount,
    /// 1, 2 or 4 ₽ of contributions per 1 ₽ from the state.
    pub ratio: u32,
    /// The co-financing for this year arrives by this day.
    pub cofinancing_due: String,
    /// The programme's state support ends after this day.
    pub ends_at: String,
    /// Tax refund for this year's contributions (13 % of up to 400 000 ₽).
    pub deduction: Amount,
}

/// The tax on deposit interest for a year.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct InterestTaxReport {
    pub year: i32,
    /// The highest key rate on the 1st of each month of the year, percent.
    pub max_key_rate: Option<Amount>,
    /// Tax-free interest, rubles; `None` while the key rate is unknown.
    pub limit: Option<Amount>,
    /// Interest received this year, rubles.
    pub income: Amount,
    pub taxable: Amount,
    pub tax: Amount,
    /// Interest that can still be earned tax-free.
    pub remaining_free: Option<Amount>,
}

impl UnlockedVault {
    // -- terms ----------------------------------------------------------------

    /// Set or clear the terms of a savings account, deposit or ПДС.
    pub fn set_deposit_terms(
        &mut self,
        store_id: &str,
        terms: Option<DepositTerms>,
    ) -> Result<Store> {
        let store = self
            .find_store(store_id)
            .ok_or_else(|| Error::Missing(format!("store {store_id}")))?;
        let sub_type = store.sub_type;
        if store.kind != StoreKind::Bank
            || !matches!(
                sub_type,
                Some(SubType::Savings | SubType::Deposit | SubType::Pds)
            )
        {
            return Err(Error::InvalidInput(
                "interest terms belong to a savings account, deposit or ПДС".into(),
            ));
        }
        let terms = match terms {
            Some(mut terms) => {
                for period in &terms.rates {
                    if day_of(&period.from).is_none() {
                        return Err(Error::InvalidInput(format!(
                            "rate date {}: expected YYYY-MM-DD",
                            period.from
                        )));
                    }
                    if period.rate.is_negative() || period.rate > amount(100) {
                        return Err(Error::InvalidInput("a rate is between 0 and 100 %".into()));
                    }
                }
                terms
                    .rates
                    .sort_by(|left, right| left.from.cmp(&right.from));
                terms
                    .rates
                    .dedup_by(|later, earlier| later.from == earlier.from);
                if let Some(opened) = terms.opened_at.as_deref().filter(|raw| !raw.is_empty())
                    && day_of(opened).is_none()
                {
                    return Err(Error::InvalidInput(format!(
                        "opened {opened}: expected YYYY-MM-DD"
                    )));
                }
                if terms.pds.is_some() && sub_type != Some(SubType::Pds) {
                    return Err(Error::InvalidInput(
                        "co-financing terms belong to a ПДС account".into(),
                    ));
                }
                if let Some(pds) = &terms.pds
                    && day_of(&pds.first_contribution_at).is_none()
                {
                    return Err(Error::InvalidInput(
                        "first contribution: expected YYYY-MM-DD".into(),
                    ));
                }
                Some(terms)
            }
            None => None,
        };
        let store = self
            .doc
            .stores
            .iter_mut()
            .find(|store| store.id == store_id)
            .ok_or_else(|| Error::Missing(format!("store {store_id}")))?;
        store.deposit = terms;
        let saved = store.clone();
        self.persist()?;
        Ok(saved)
    }

    /// The money an account holds: its bound currency, else the base one.
    fn deposit_asset(&self, store: &Store) -> AssetRef {
        match &store.binding {
            Binding::Asset(asset) => asset.clone(),
            _ => {
                let base = self.base_and_fx().0.to_uppercase();
                AssetRef::fiat(&base, &base)
            }
        }
    }

    /// Balance of one asset at the end of each day of `[from, to]`.
    fn daily_balances(
        &self,
        store_id: &str,
        symbol: &str,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Vec<(NaiveDate, Amount)> {
        let mut opening = Amount::ZERO;
        let mut moves: BTreeMap<NaiveDate, Amount> = BTreeMap::new();
        for tx in &self.doc.transactions {
            let Some(day) = day_of(&tx.meta().timestamp) else {
                continue;
            };
            for leg in tx.legs() {
                if leg.store_id != store_id || !leg.asset.symbol.eq_ignore_ascii_case(symbol) {
                    continue;
                }
                if day < from {
                    opening += leg.amount;
                } else if day <= to {
                    *moves.entry(day).or_default() += leg.amount;
                }
            }
        }
        let mut out = Vec::new();
        let mut balance = opening;
        let mut day = from;
        while day <= to {
            balance += moves.get(&day).copied().unwrap_or(Amount::ZERO);
            out.push((day, balance));
            day += Duration::days(1);
        }
        out
    }

    /// Interest earned on `[from, to]` by the day, at each day's rate.
    fn interest_between(
        &self,
        store: &Store,
        terms: &DepositTerms,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Amount {
        if to < from {
            return Amount::ZERO;
        }
        let symbol = self.deposit_asset(store).symbol;
        let hundred = amount(100);
        self.daily_balances(&store.id, &symbol, from, to)
            .into_iter()
            .filter(|(_, balance)| balance.is_positive())
            .map(|(day, balance)| {
                let rate = terms
                    .rate_on(&day.format("%Y-%m-%d").to_string())
                    .unwrap_or(Amount::ZERO);
                balance * rate / hundred / amount(days_in_year(day.year()))
            })
            .fold(Amount::ZERO, |sum, value| sum + value)
            .round(2)
    }

    /// `balance` after `days` at `rate` percent, compounded by `capitalization`.
    fn grow(balance: Amount, rate: Amount, days: i64, capitalization: Capitalization) -> Amount {
        let yearly = rate.to_f64() / 100.0;
        let principal = balance.to_f64();
        let years = days as f64 / 365.0;
        let grown = match capitalization {
            Capitalization::None | Capitalization::AtEnd => principal * (1.0 + yearly * years),
            Capitalization::Daily => principal * (1.0 + yearly / 365.0).powf(days as f64),
            Capitalization::Monthly => principal * (1.0 + yearly / 12.0).powf(years * 12.0),
        };
        Amount::from_f64(grown).round(2)
    }

    /// Interest booked this year on a store (`deposit_interest` top-ups).
    fn posted_interest(&self, store_id: &str, symbol: &str, year: i32) -> Amount {
        self.doc
            .transactions
            .iter()
            .filter(|tx| {
                tx.meta()
                    .categories
                    .iter()
                    .any(|key| key == INTEREST_CATEGORY)
            })
            .filter(|tx| day_of(&tx.meta().timestamp).is_some_and(|day| day.year() == year))
            .flat_map(Transaction::legs)
            .filter(|leg| {
                leg.store_id == store_id
                    && leg.asset.symbol.eq_ignore_ascii_case(symbol)
                    && leg.amount.is_positive()
            })
            .fold(Amount::ZERO, |sum, leg| sum + leg.amount)
    }

    /// The account at a glance: rate, earned so far, what is ahead.
    pub fn deposit_report(&self, store_id: &str, today: NaiveDate) -> Result<DepositReport> {
        let store = self
            .find_store(store_id)
            .cloned()
            .ok_or_else(|| Error::Missing(format!("store {store_id}")))?;
        let asset = self.deposit_asset(&store);
        let balance = self
            .daily_balances(&store.id, &asset.symbol, today, today)
            .first()
            .map(|(_, value)| *value)
            .unwrap_or(Amount::ZERO);
        let terms = store.deposit.clone().unwrap_or(DepositTerms {
            rates: Vec::new(),
            capitalization: Capitalization::Monthly,
            opened_at: None,
            term_months: None,
            auto_accrue: false,
            pds: None,
        });
        let today_raw = today.format("%Y-%m-%d").to_string();
        let rate = terms.rate_on(&today_raw);
        let rate_since = terms
            .rates
            .iter()
            .rfind(|period| period.from <= today_raw)
            .map(|period| period.from.clone());
        let year_start = NaiveDate::from_ymd_opt(today.year(), 1, 1).unwrap_or(today);
        let matures = terms
            .opened_at
            .as_deref()
            .and_then(day_of)
            .zip(terms.term_months)
            .and_then(|(opened, months)| opened.checked_add_months(chrono::Months::new(months)));
        let at_maturity = matures
            .filter(|end| *end > today)
            .zip(rate)
            .map(|(end, rate)| {
                Self::grow(
                    balance,
                    rate,
                    (end - today).num_days(),
                    terms.capitalization,
                )
            });
        Ok(DepositReport {
            store_id: store.id.clone(),
            currency: asset.symbol.to_lowercase(),
            balance,
            rate,
            rate_since,
            accrued_month: self.interest_between(&store, &terms, month_start(today), today),
            accrued_year: self.interest_between(&store, &terms, year_start, today),
            posted_year: self.posted_interest(&store.id, &asset.symbol, today.year()),
            projected_year: rate
                .map(|rate| Self::grow(balance, rate, 365, terms.capitalization) - balance)
                .unwrap_or(Amount::ZERO),
            matures_at: matures.map(|day| day.format("%Y-%m-%d").to_string()),
            at_maturity,
            pds: terms.pds.as_ref().map(|pds| {
                self.pds_report(
                    &store,
                    &asset.symbol,
                    pds.income_bracket,
                    &pds.first_contribution_at,
                    today,
                )
            }),
        })
    }

    fn pds_report(
        &self,
        store: &Store,
        symbol: &str,
        bracket: IncomeBracket,
        first: &str,
        today: NaiveDate,
    ) -> PdsReport {
        let year = today.year();
        let contributions = self
            .doc
            .transactions
            .iter()
            .filter(|tx| {
                !tx.meta()
                    .categories
                    .iter()
                    .any(|key| key == INTEREST_CATEGORY || key == "income_other")
            })
            .filter(|tx| day_of(&tx.meta().timestamp).is_some_and(|day| day.year() == year))
            .flat_map(Transaction::legs)
            .filter(|leg| {
                leg.store_id == store.id
                    && leg.asset.symbol.eq_ignore_ascii_case(symbol)
                    && leg.amount.is_positive()
            })
            .fold(Amount::ZERO, |sum, leg| sum + leg.amount);
        let ratio: u32 = match bracket {
            IncomeBracket::Low => 1,
            IncomeBracket::Mid => 2,
            IncomeBracket::High => 4,
        };
        let cap = amount(PDS_MAX_COFINANCING);
        let first_day = day_of(first).unwrap_or(today);
        let ends = first_day
            .checked_add_months(chrono::Months::new(12 * PDS_YEARS as u32))
            .unwrap_or(first_day);
        let active = today <= ends && today >= first_day;
        let cofinancing = if active && contributions >= amount(PDS_MIN_CONTRIBUTION) {
            (contributions / amount(i64::from(ratio))).min(cap).round(2)
        } else {
            Amount::ZERO
        };
        let full_at = cap * amount(i64::from(ratio));
        let (due_month, due_day) = PDS_DUE_MONTH_DAY;
        PdsReport {
            year,
            contributions,
            cofinancing,
            to_full_cofinancing: if active {
                (full_at - contributions).max(Amount::ZERO)
            } else {
                Amount::ZERO
            },
            ratio,
            cofinancing_due: NaiveDate::from_ymd_opt(year + 1, due_month, due_day)
                .map(|day| day.format("%Y-%m-%d").to_string())
                .unwrap_or_default(),
            ends_at: ends.format("%Y-%m-%d").to_string(),
            deduction: (contributions.min(amount(PDS_DEDUCTION_CAP)) * amount(NDFL_RATE_PCT)
                / amount(100))
            .round(2),
        }
    }

    // -- the key rate and the tax -------------------------------------------------

    /// Merge fetched key-rate points into the cache. Does not persist.
    pub fn apply_key_rates(&mut self, fresh: &[KeyRatePoint]) {
        let mut by_day: BTreeMap<String, Amount> = self
            .doc
            .key_rates
            .iter()
            .map(|point| (point.date.clone(), point.rate))
            .collect();
        for point in fresh {
            by_day.insert(point.date.clone(), point.rate);
        }
        self.doc.key_rates = by_day
            .into_iter()
            .map(|(date, rate)| KeyRatePoint { date, rate })
            .collect();
    }

    /// The key rate in force on `day`.
    pub fn key_rate_on(&self, day: NaiveDate) -> Option<Amount> {
        let raw = day.format("%Y-%m-%d").to_string();
        self.doc
            .key_rates
            .iter()
            .filter(|point| point.date <= raw)
            .max_by(|left, right| left.date.cmp(&right.date))
            .map(|point| point.rate)
    }

    /// НДФЛ on deposit interest received in `year`, in rubles: 13 % of what
    /// exceeds 1 000 000 ₽ × the highest key rate on the 1st of a month.
    pub fn interest_tax(&self, year: i32) -> InterestTaxReport {
        let max_key_rate = (1..=12)
            .filter_map(|month| NaiveDate::from_ymd_opt(year, month, 1))
            .filter_map(|day| self.key_rate_on(day))
            .max();
        let limit =
            max_key_rate.map(|rate| (amount(TAX_FREE_PRINCIPAL) * rate / amount(100)).round(2));
        let book = self.rate_book();
        let bank_stores: std::collections::HashSet<&str> = self
            .doc
            .stores
            .iter()
            .filter(|store| store.kind == StoreKind::Bank)
            .map(|store| store.id.as_str())
            .collect();
        let income = self
            .doc
            .transactions
            .iter()
            .filter(|tx| {
                tx.meta()
                    .categories
                    .iter()
                    .any(|key| key == INTEREST_CATEGORY)
            })
            .filter(|tx| day_of(&tx.meta().timestamp).is_some_and(|day| day.year() == year))
            .flat_map(|tx| {
                let secs = secs_or_now(&tx.meta().timestamp);
                tx.legs().into_iter().map(move |leg| (secs, leg))
            })
            .filter(|(_, leg)| {
                bank_stores.contains(leg.store_id.as_str()) && leg.amount.is_positive()
            })
            .map(|(secs, leg)| {
                if leg.asset.symbol.eq_ignore_ascii_case("rub") {
                    leg.amount
                } else {
                    book.convert(leg.amount, &leg.asset.symbol, "rub", secs)
                        .unwrap_or(Amount::ZERO)
                }
            })
            .fold(Amount::ZERO, |sum, value| sum + value)
            .round(2);
        let taxable = limit.map_or(Amount::ZERO, |limit| (income - limit).max(Amount::ZERO));
        InterestTaxReport {
            year,
            max_key_rate,
            limit,
            income,
            taxable,
            tax: (taxable * amount(NDFL_RATE_PCT) / amount(100)).round(2),
            remaining_free: limit.map(|limit| (limit - income).max(Amount::ZERO)),
        }
    }

    // -- posting interest -------------------------------------------------------

    /// Book last month's interest on every account that asks for it, once.
    /// Only the month just ended is posted: older months may already be in
    /// the ledger by hand, and guessing them would count them twice.
    pub fn post_due_interest(&mut self, today: NaiveDate) -> Result<u32> {
        let last_month = month_start(month_start(today) - Duration::days(1));
        let period = last_month.format("%Y-%m").to_string();
        let mut drafts = Vec::new();
        for store in self.doc.stores.iter().filter(|store| !store.archived) {
            let Some(terms) = store.deposit.as_ref().filter(|terms| terms.auto_accrue) else {
                continue;
            };
            let tag = format!("{INTEREST_ORIGIN_PREFIX}{}", store.id);
            let posted = self.doc.transactions.iter().any(|tx| {
                matches!(&tx.meta().origin, TxOrigin::Automation { automation_id, planned_at } if *automation_id == tag && *planned_at == period)
            });
            if posted {
                continue;
            }
            let earned = self.interest_between(
                store,
                terms,
                last_month,
                next_month(last_month) - Duration::days(1),
            );
            if !earned.is_positive() {
                continue;
            }
            let asset = self.deposit_asset(store);
            let mut meta = TxMeta::new(&format!("% {period}"));
            meta.categories = vec![INTEREST_CATEGORY.to_string()];
            meta.timestamp = format!("{}T09:00:00Z", next_month(last_month).format("%Y-%m-%d"));
            meta.origin = TxOrigin::Automation {
                automation_id: tag,
                planned_at: period.clone(),
            };
            drafts.push(Transaction::Topup(TopupTx {
                meta,
                store_id: store.id.clone(),
                asset,
                amount: earned,
                fee: None,
                from: None,
                details: KindDetails::default(),
                price: PriceStamp::default(),
            }));
        }
        let count = drafts.len() as u32;
        for mut tx in drafts {
            self.prepare_transaction(&mut tx)?;
            self.doc.transactions.push(tx);
        }
        if count > 0 {
            self.recompute_holdings();
            self.persist()?;
        }
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::deposit::{PdsTerms, RatePeriod};
    use crate::service::StoreExtras;
    use crate::service::test_support::*;

    fn date(raw: &str) -> NaiveDate {
        day_of(raw).unwrap()
    }

    /// A ruble savings account with 100 000 ₽ from the 1st of January 2026.
    fn savings(vault: &mut UnlockedVault, sub_type: SubType) -> String {
        let bank = store(vault, "Bank", StoreKind::Bank, None);
        let rub = AssetRef::fiat("RUB", "RUB");
        let account = vault
            .create_store_with(
                "Savings",
                StoreKind::Bank,
                "#000",
                "",
                Some(bank),
                StoreExtras {
                    sub_type: Some(sub_type),
                    binding: Some(Binding::Asset(rub.clone())),
                    ..StoreExtras::default()
                },
            )
            .unwrap()
            .id;
        let mut seed = income_of(&account, rub, "100000", "0.01", "salary");
        seed.meta_mut().timestamp = "2026-01-01T00:00:00Z".into();
        vault.add_transaction(seed).unwrap();
        account
    }

    fn terms(rates: &[(&str, &str)]) -> DepositTerms {
        DepositTerms {
            rates: rates
                .iter()
                .map(|(from, rate)| RatePeriod {
                    from: (*from).into(),
                    rate: amount_str(rate),
                })
                .collect(),
            capitalization: Capitalization::Monthly,
            opened_at: None,
            term_months: None,
            auto_accrue: true,
            pds: None,
        }
    }

    fn amount_str(raw: &str) -> Amount {
        Amount::parse(raw).unwrap()
    }

    #[test]
    fn interest_follows_the_rate_history_day_by_day() {
        let (_dir, mut vault) = empty_vault();
        let account = savings(&mut vault, SubType::Savings);
        // 18.25 % for January's 31 days, then 0 %.
        vault
            .set_deposit_terms(
                &account,
                Some(terms(&[("2026-01-01", "18.25"), ("2026-02-01", "0")])),
            )
            .unwrap();
        let report = vault.deposit_report(&account, date("2026-03-10")).unwrap();
        // 100 000 × 18.25 % / 365 = 50 ₽ a day × 31 days.
        assert_eq!(report.accrued_year, amount_str("1550"));
        assert_eq!(report.rate, Some(amount_str("0")));
        assert_eq!(report.rate_since.as_deref(), Some("2026-02-01"));
    }

    #[test]
    fn last_month_is_posted_once() {
        let (_dir, mut vault) = empty_vault();
        let account = savings(&mut vault, SubType::Savings);
        vault
            .set_deposit_terms(&account, Some(terms(&[("2026-01-01", "18.25")])))
            .unwrap();
        assert_eq!(vault.post_due_interest(date("2026-02-03")).unwrap(), 1);
        assert_eq!(vault.post_due_interest(date("2026-02-20")).unwrap(), 0);
        let report = vault.deposit_report(&account, date("2026-02-03")).unwrap();
        assert_eq!(report.posted_year, amount_str("1550"));
        assert_eq!(report.balance, amount_str("101550"));
    }

    #[test]
    fn terms_belong_to_savings_only() {
        let (_dir, mut vault) = empty_vault();
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        assert!(
            vault
                .set_deposit_terms(&card, Some(terms(&[("2026-01-01", "10")])))
                .is_err()
        );
        let account = savings(&mut vault, SubType::Deposit);
        assert!(
            vault
                .set_deposit_terms(&account, Some(terms(&[("2026-13-01", "10")])))
                .is_err()
        );
        assert!(
            vault
                .set_deposit_terms(&account, Some(terms(&[("2026-01-01", "150")])))
                .is_err()
        );
    }

    #[test]
    fn pds_cofinancing_follows_the_bracket_and_the_cap() {
        let (_dir, mut vault) = empty_vault();
        let account = savings(&mut vault, SubType::Pds);
        let mut pds_terms = terms(&[]);
        pds_terms.pds = Some(PdsTerms {
            first_contribution_at: "2026-01-01".into(),
            income_bracket: IncomeBracket::Mid,
        });
        vault.set_deposit_terms(&account, Some(pds_terms)).unwrap();
        let pds = vault
            .deposit_report(&account, date("2026-06-01"))
            .unwrap()
            .pds
            .unwrap();
        // 100 000 ₽ in at 1:2 → 50 000, capped at 36 000.
        assert_eq!(pds.contributions, amount_str("100000"));
        assert_eq!(pds.cofinancing, amount_str("36000"));
        assert_eq!(pds.ratio, 2);
        assert_eq!(pds.ends_at, "2036-01-01");
        assert_eq!(pds.cofinancing_due, "2027-08-31");
        assert_eq!(pds.deduction, amount_str("13000"));
    }

    #[test]
    fn interest_tax_uses_the_highest_key_rate_of_the_year() {
        let (_dir, mut vault) = empty_vault();
        let account = savings(&mut vault, SubType::Savings);
        vault.apply_key_rates(&[
            KeyRatePoint {
                date: "2025-12-20".into(),
                rate: amount_str("16"),
            },
            KeyRatePoint {
                date: "2026-03-20".into(),
                rate: amount_str("21"),
            },
            KeyRatePoint {
                date: "2026-08-01".into(),
                rate: amount_str("17"),
            },
        ]);
        let mut paid = income_of(
            &account,
            AssetRef::fiat("RUB", "RUB"),
            "250000",
            "0.01",
            "interest",
        );
        paid.meta_mut().categories = vec![INTEREST_CATEGORY.into()];
        paid.meta_mut().timestamp = "2026-06-30T00:00:00Z".into();
        vault.add_transaction(paid).unwrap();
        let tax = vault.interest_tax(2026);
        // The highest rate on a 1st of a month is 21 % (April–August).
        assert_eq!(tax.max_key_rate, Some(amount_str("21")));
        assert_eq!(tax.limit, Some(amount_str("210000")));
        assert_eq!(tax.taxable, amount_str("40000"));
        assert_eq!(tax.tax, amount_str("5200"));
    }
}
