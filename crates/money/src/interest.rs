//! Interest on savings accounts and term deposits, the ПДС state
//! co-financing, and the tax on deposit interest (НДФЛ above the limit of
//! 1 000 000 ₽ × the highest key rate of the year).
//!
//! Interest is computed day by day on the real balance of the account, at the
//! rate in force that day. Posted interest is an ordinary top-up in the
//! `deposit_interest` category, so it raises the balance like a bank's
//! capitalization does; the projection compounds by the account's terms.
//! Everything here reads the document; the scenarios that write it live in
//! `usecases::money::interest`.

use std::collections::BTreeMap;

use chrono::{Datelike, Duration, NaiveDate};
use rust_decimal::Decimal;

use crate::books::Books;
use mwt_base::Amount;
use mwt_base::calendar::Calendar;
use mwt_base::error::{Error, Result};
use mwt_base::time::secs_or_now;
use mwt_model::StoreId;
use mwt_model::VaultDoc;
use mwt_model::{AssetRef, Binding, Store, StoreKind, SubType};
use mwt_model::{Capitalization, DepositTerms, IncomeBracket, KeyRatePoint};
use mwt_model::{KindDetails, PriceStamp, TopupTx, Transaction, TxMeta, TxOrigin};

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

pub fn day_of(raw: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(raw.get(..10)?, "%Y-%m-%d").ok()
}

/// The calendar of the user's day: the vault's zone (`schedule_timezone`,
/// the same on every device), UTC until a device wrote one.
pub fn calendar_of(doc: &VaultDoc) -> Calendar {
    Calendar::new(
        doc.settings
            .schedule_timezone
            .as_deref()
            .unwrap_or_default(),
    )
}

/// The day a transaction falls on for the user. A moment is cut at the local
/// midnight, so a deposit at 01:00 in UTC+3 belongs to that day and not to
/// the UTC day before; a bare date keeps the day it names (`L14`).
fn tx_day(calendar: &Calendar, timestamp: &str) -> Option<NaiveDate> {
    calendar
        .date_of_iso(timestamp)
        .or_else(|| day_of(timestamp))
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
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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

// -- terms ----------------------------------------------------------------

/// Check terms before they are saved on a savings account, deposit or ПДС
/// (`None` clears them). Returns the terms to store: rates sorted by date,
/// one per day.
pub fn plan_deposit_terms(
    doc: &VaultDoc,
    store_id: &StoreId,
    terms: Option<DepositTerms>,
) -> Result<Option<DepositTerms>> {
    let store = doc
        .stores
        .iter()
        .find(|store| store.id == store_id.as_str())
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
    let Some(mut terms) = terms else {
        return Ok(None);
    };
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
    Ok(Some(terms))
}

/// The money an account holds: its bound currency, else the base one.
pub fn deposit_asset(books: &Books<'_>, store: &Store) -> AssetRef {
    match &store.binding {
        Binding::Asset(asset) => asset.clone(),
        _ => {
            let base = books.base_and_fx().0.to_uppercase();
            AssetRef::fiat(&base, &base)
        }
    }
}

/// Balance of one asset at the end of each day of `[from, to]`.
pub fn daily_balances(
    doc: &VaultDoc,
    store_id: &str,
    symbol: &str,
    from: NaiveDate,
    to: NaiveDate,
) -> Vec<(NaiveDate, Amount)> {
    let mut opening = Amount::ZERO;
    let mut moves: BTreeMap<NaiveDate, Amount> = BTreeMap::new();
    let calendar = calendar_of(doc);
    for tx in &doc.transactions {
        let Some(day) = tx_day(&calendar, &tx.meta().timestamp) else {
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
pub fn interest_between(
    books: &Books<'_>,
    store: &Store,
    terms: &DepositTerms,
    from: NaiveDate,
    to: NaiveDate,
) -> Amount {
    if to < from {
        return Amount::ZERO;
    }
    let symbol = deposit_asset(books, store).symbol;
    let hundred = amount(100);
    daily_balances(books.doc(), &store.id, &symbol, from, to)
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
pub fn grow(balance: Amount, rate: Amount, days: i64, capitalization: Capitalization) -> Amount {
    let yearly = rate / amount(100);
    let day_count = u32::try_from(days).unwrap_or(0);
    let years = amount(i64::from(day_count)) / amount(365);
    let one = amount(1);
    let grown = match capitalization {
        Capitalization::None | Capitalization::AtEnd => balance * (one + yearly * years),
        Capitalization::Daily => balance * (one + yearly / amount(365)).pow_int(day_count),
        // (1 + r/12)^(12 * days / 365), as the 365th root raised to 12 * days.
        Capitalization::Monthly => {
            let factor = (one + yearly / amount(12)).root_n(365);
            balance * factor.pow_int(day_count.saturating_mul(12))
        }
    };
    grown.round(2)
}

/// Interest booked this year on a store (`deposit_interest` top-ups).
pub fn posted_interest(doc: &VaultDoc, store_id: &str, symbol: &str, year: i32) -> Amount {
    let calendar = calendar_of(doc);
    doc.transactions
        .iter()
        .filter(|tx| {
            tx.meta()
                .categories
                .iter()
                .any(|key| key == INTEREST_CATEGORY)
        })
        .filter(|tx| tx_day(&calendar, &tx.meta().timestamp).is_some_and(|day| day.year() == year))
        .flat_map(Transaction::legs)
        .filter(|leg| {
            leg.store_id == store_id
                && leg.asset.symbol.eq_ignore_ascii_case(symbol)
                && leg.amount.is_positive()
        })
        .fold(Amount::ZERO, |sum, leg| sum + leg.amount)
}

/// The account at a glance: rate, earned so far, what is ahead.
pub fn deposit_report(
    books: &Books<'_>,
    store_id: &StoreId,
    today: NaiveDate,
) -> Result<DepositReport> {
    let doc = books.doc();
    let store = doc
        .stores
        .iter()
        .find(|store| store.id == store_id.as_str())
        .cloned()
        .ok_or_else(|| Error::Missing(format!("store {store_id}")))?;
    let asset = deposit_asset(books, &store);
    let balance = daily_balances(doc, &store.id, &asset.symbol, today, today)
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
            grow(
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
        accrued_month: interest_between(books, &store, &terms, month_start(today), today),
        accrued_year: interest_between(books, &store, &terms, year_start, today),
        posted_year: posted_interest(doc, &store.id, &asset.symbol, today.year()),
        projected_year: rate
            .map(|rate| grow(balance, rate, 365, terms.capitalization) - balance)
            .unwrap_or(Amount::ZERO),
        matures_at: matures.map(|day| day.format("%Y-%m-%d").to_string()),
        at_maturity,
        pds: terms.pds.as_ref().map(|pds| {
            pds_report(
                doc,
                &store,
                &asset.symbol,
                pds.income_bracket,
                &pds.first_contribution_at,
                today,
            )
        }),
    })
}

pub fn pds_report(
    doc: &VaultDoc,
    store: &Store,
    symbol: &str,
    bracket: IncomeBracket,
    first: &str,
    today: NaiveDate,
) -> PdsReport {
    let year = today.year();
    let calendar = calendar_of(doc);
    let contributions = doc
        .transactions
        .iter()
        .filter(|tx| {
            !tx.meta()
                .categories
                .iter()
                .any(|key| key == INTEREST_CATEGORY || key == "income_other")
        })
        .filter(|tx| tx_day(&calendar, &tx.meta().timestamp).is_some_and(|day| day.year() == year))
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

/// `known` key-rate points with the `fresh` ones merged in: one per day,
/// the fresh value wins, oldest first.
pub fn merge_key_rates(known: &[KeyRatePoint], fresh: &[KeyRatePoint]) -> Vec<KeyRatePoint> {
    let mut by_day: BTreeMap<String, Amount> = known
        .iter()
        .map(|point| (point.date.clone(), point.rate))
        .collect();
    for point in fresh {
        by_day.insert(point.date.clone(), point.rate);
    }
    by_day
        .into_iter()
        .map(|(date, rate)| KeyRatePoint { date, rate })
        .collect()
}

/// The key rate in force on `day`.
pub fn key_rate_on(doc: &VaultDoc, day: NaiveDate) -> Option<Amount> {
    let raw = day.format("%Y-%m-%d").to_string();
    doc.key_rates
        .iter()
        .filter(|point| point.date <= raw)
        .max_by(|left, right| left.date.cmp(&right.date))
        .map(|point| point.rate)
}

/// НДФЛ on deposit interest received in `year`, in rubles: 13 % of what
/// exceeds 1 000 000 ₽ × the highest key rate on the 1st of a month.
pub fn interest_tax(books: &Books<'_>, year: i32) -> InterestTaxReport {
    let doc = books.doc();
    let max_key_rate = (1..=12)
        .filter_map(|month| NaiveDate::from_ymd_opt(year, month, 1))
        .filter_map(|day| key_rate_on(doc, day))
        .max();
    let limit = max_key_rate.map(|rate| (amount(TAX_FREE_PRINCIPAL) * rate / amount(100)).round(2));
    let book = books.rate_book();
    let calendar = calendar_of(doc);
    let bank_stores: std::collections::HashSet<&str> = doc
        .stores
        .iter()
        .filter(|store| store.kind == StoreKind::Bank)
        .map(|store| store.id.as_str())
        .collect();
    let income = doc
        .transactions
        .iter()
        .filter(|tx| {
            tx.meta()
                .categories
                .iter()
                .any(|key| key == INTEREST_CATEGORY)
        })
        .filter(|tx| tx_day(&calendar, &tx.meta().timestamp).is_some_and(|day| day.year() == year))
        .flat_map(|tx| {
            let secs = secs_or_now(&tx.meta().timestamp);
            tx.legs().into_iter().map(move |leg| (secs, leg))
        })
        .filter(|(_, leg)| bank_stores.contains(leg.store_id.as_str()) && leg.amount.is_positive())
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

/// The top-ups that book last month's interest on every account that asks
/// for it and has not got it yet. Only the month just ended is posted: older
/// months may already be in the ledger by hand, and guessing them would count
/// them twice.
pub fn plan_due_interest(books: &Books<'_>, today: NaiveDate) -> Vec<Transaction> {
    let doc = books.doc();
    let last_month = month_start(month_start(today) - Duration::days(1));
    let period = last_month.format("%Y-%m").to_string();
    let mut drafts = Vec::new();
    for store in doc.stores.iter().filter(|store| !store.archived) {
        let Some(terms) = store.deposit.as_ref().filter(|terms| terms.auto_accrue) else {
            continue;
        };
        let tag = format!("{INTEREST_ORIGIN_PREFIX}{}", store.id);
        let posted = doc.transactions.iter().any(|tx| {
            matches!(&tx.meta().origin, TxOrigin::Automation { automation_id, planned_at } if *automation_id == tag && *planned_at == period)
        });
        if posted {
            continue;
        }
        let earned = interest_between(
            books,
            store,
            terms,
            last_month,
            next_month(last_month) - Duration::days(1),
        );
        if !earned.is_positive() {
            continue;
        }
        let asset = deposit_asset(books, store);
        let mut meta = TxMeta::new(&format!("% {period}"));
        meta.categories = vec![INTEREST_CATEGORY.to_string()];
        meta.timestamp = format!("{}T09:00:00Z", next_month(last_month).format("%Y-%m-%d"));
        meta.origin = TxOrigin::Automation {
            automation_id: tag,
            planned_at: period.clone(),
        };
        drafts.push(Transaction::Topup(TopupTx {
            parts: Vec::new(),
            distribution: None,
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
    drafts
}
