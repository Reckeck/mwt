//! Money flows of the budget: every operation of a period classified once
//! (`classify_flow`) and valued in one currency at the rate of its day, plus
//! the spending of shared events that has no payment of mine behind it. The
//! queries take the `Books` of one request, so a request builds its rates
//! once.

use chrono::{Months, NaiveDate};
use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};

use mwt_base::Amount;
use mwt_base::calendar::{Calendar, first_of_month, month_key, parse_month};
use mwt_base::time::secs_or_now;
use mwt_household::shared_events::analytics::{category_group, extras_between, value_in};
use mwt_model::VaultDoc;
use mwt_model::{Leg, LegRole, MonthShare, Transaction};
use mwt_money::books::Books;
use mwt_money::rates::RateBook;
use mwt_money::shared_ratios::SharedExtra;
use mwt_money::worth::{FlowClass, classify_flow, spending};

/// Which date a flow counts by: the month its operation is «for» (`Period`)
/// or the month of the money date (`MoneyDate`, the reading before periods).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Basis {
    Period,
    MoneyDate,
}

#[derive(Debug, Clone)]
pub struct Flow {
    /// The money date.
    pub date: NaiveDate,
    /// The month `YYYY-MM` the flow counts in under the basis it was read by.
    pub month: String,
    pub class: FlowClass,
    /// In the requested currency.
    pub value: Amount,
    pub categories: Vec<String>,
    pub stores: Vec<String>,
    pub tx_id: String,
    /// Currencies (upper case) of the sides that had no rate; empty = priced.
    pub missing: Vec<String>,
}

impl Flow {
    pub fn priced(&self) -> bool {
        self.missing.is_empty()
    }
}

/// Where a transaction lands in the budget: the one classification.
pub(crate) fn classify(books: &Books<'_>, tx: &Transaction) -> FlowClass {
    classify_flow(books.doc(), tx)
}

/// Value of one leg in `currency` at `secs`; `None` without a rate.
pub fn leg_value_in(book: &RateBook, leg: &Leg, currency: &str, secs: i64) -> Option<Amount> {
    let qty = leg.amount.abs();
    if leg.asset.is_fiat() && leg.asset.symbol.eq_ignore_ascii_case(currency) {
        return Some(qty);
    }
    let usd = leg
        .price_usd
        .or_else(|| book.price_at(&leg.asset.key(), secs))?;
    book.convert(qty * usd, "usd", currency, secs)
}

/// What a flow is worth to the budget: the money that came in for income
/// and money entering the budget, the money (and fees) that went out for
/// everything else. Also the currencies of the sides without a rate,
/// which count as zero.
pub(crate) fn flow_value(
    book: &RateBook,
    tx: &Transaction,
    class: FlowClass,
    currency: &str,
) -> (Amount, Vec<String>) {
    let secs = secs_or_now(&tx.meta().timestamp);
    let incoming = matches!(class, FlowClass::Income | FlowClass::BudgetIn);
    let mut total = Amount::ZERO;
    let mut missing: Vec<String> = Vec::new();
    for leg in tx.legs() {
        let counts = match leg.role {
            LegRole::Principal => leg.amount.is_positive() == incoming,
            LegRole::Fee => !incoming,
            // A correction is neither income nor spending.
            LegRole::Pnl | LegRole::Adjustment => false,
        };
        if !counts || leg.amount.is_zero() {
            continue;
        }
        match leg_value_in(book, &leg, currency, secs) {
            Some(value) => total += value,
            None => {
                let code = leg.asset.symbol.to_uppercase();
                if !missing.contains(&code) {
                    missing.push(code);
                }
            }
        }
    }
    (total, missing)
}

/// The flow of one operation of mine, counted in the month of its money date;
/// a payment made for a shared event counts for my part of it only.
fn payment_flow(
    books: &Books<'_>,
    cal: &Calendar,
    currency: &str,
    tx: &Transaction,
) -> Option<Flow> {
    let class = classify(books, tx);
    if class == FlowClass::Ignored {
        return None;
    }
    let secs = secs_or_now(&tx.meta().timestamp);
    let (mut value, missing) = flow_value(books.rate_book(), tx, class, currency);
    if let Some(ratio) = books.shared_ratios().get(&tx.meta().id) {
        value = value * *ratio;
    }
    let date = cal.date_of(secs);
    Some(Flow {
        date,
        month: month_key(date),
        class,
        value,
        categories: tx.meta().categories.clone(),
        stores: tx.legs().into_iter().map(|leg| leg.store_id).collect(),
        tx_id: tx.meta().id.clone(),
        missing,
    })
}

/// The flows of my own operations paid in `[from, to)`.
fn payment_flows(
    books: &Books<'_>,
    cal: &Calendar,
    currency: &str,
    from: i64,
    to: i64,
) -> Vec<Flow> {
    books
        .doc()
        .transactions
        .iter()
        .filter(|tx| {
            let secs = secs_or_now(&tx.meta().timestamp);
            secs >= from && secs < to
        })
        .filter_map(|tx| payment_flow(books, cal, currency, tx))
        .collect()
}

/// One spending flow of mine in the event currency, valued in `currency`.
fn event_flow(
    doc: &VaultDoc,
    book: &RateBook,
    cal: &Calendar,
    currency: &str,
    extra: &SharedExtra,
) -> Flow {
    let (value, missing) = value_in(book, extra, currency);
    let date: NaiveDate = cal.date_of(extra.secs);
    Flow {
        date,
        month: month_key(date),
        class: spending(category_group(doc, &extra.category)),
        value,
        categories: vec![extra.category.clone()],
        stores: Vec::new(),
        tx_id: extra.tag.clone(),
        missing,
    }
}

/// Spending that has no payment of mine behind it: my part of other
/// people's purchases and what I wrote off, over entries the caller already
/// collected (`Books` keeps them, so a request collects them once).
pub(crate) fn extra_flows_from(
    doc: &VaultDoc,
    extras: &[SharedExtra],
    book: &RateBook,
    cal: &Calendar,
    currency: &str,
    from: i64,
    to: i64,
) -> Vec<Flow> {
    extras_between(extras, from, to)
        .map(|extra| event_flow(doc, book, cal, currency, extra))
        .filter(|flow| flow.class != FlowClass::Ignored)
        .collect()
}

/// Classified flows in `[from, to)`, valued in `currency`, over the books of
/// one request: the rates and the shared-event figures are built once and
/// shared.
pub fn flows_between(
    books: &Books<'_>,
    cal: &Calendar,
    currency: &str,
    from: i64,
    to: i64,
) -> Vec<Flow> {
    let mut flows = payment_flows(books, cal, currency, from, to);
    flows.extend(extra_flows_from(
        books.doc(),
        books.shared_extras(),
        books.rate_book(),
        cal,
        currency,
        from,
        to,
    ));
    flows
}

/// Places of a month's part of a flow value. Always two: the app rounds money
/// to two places everywhere (`round_places(.., 2)`) and keeps no per-currency
/// places table.
const PART_PLACES: u32 = 2;

/// `value` cut by the shares of a spread, valued once (R7): every part but
/// the last is rounded down, the last gets the residual, so the parts sum to
/// `value` exactly (P7). `None` when the shares carry nothing to cut by.
fn split_by_shares(value: Amount, shares: &[MonthShare]) -> Option<Vec<(String, Amount)>> {
    let total: Decimal = shares.iter().map(|share| share.amount.into_inner()).sum();
    let last = shares.len().checked_sub(1)?;
    if total.is_zero() {
        return None;
    }
    let mut given = Decimal::ZERO;
    let mut parts = Vec::with_capacity(shares.len());
    for (index, share) in shares.iter().enumerate() {
        let part = if index == last {
            value.into_inner() - given
        } else {
            (value.into_inner() * share.amount.into_inner() / total)
                .round_dp_with_strategy(PART_PLACES, RoundingStrategy::ToZero)
        };
        given += part;
        parts.push((share.month.clone(), Amount::new(part)));
    }
    Some(parts)
}

/// The month an operation without a spread counts in (P3, P4, Q3): the one
/// month of a period inside a month, otherwise the month of the money date.
fn whole_month(tx: &Transaction, money_month: &str) -> String {
    let Some(period) = &tx.meta().period else {
        return money_month.to_string();
    };
    let bounds = (
        NaiveDate::parse_from_str(&period.start, "%Y-%m-%d"),
        NaiveDate::parse_from_str(&period.end, "%Y-%m-%d"),
    );
    match bounds {
        (Ok(start), Ok(end)) if month_key(start) == month_key(end) => month_key(start),
        _ => money_month.to_string(),
    }
}

/// The flows of one operation under the period reading: one flow per month of
/// its spread, otherwise the one flow counted whole in its month.
fn period_flows_of(
    books: &Books<'_>,
    cal: &Calendar,
    currency: &str,
    tx: &Transaction,
) -> Vec<Flow> {
    let Some(flow) = payment_flow(books, cal, currency, tx) else {
        return Vec::new();
    };
    let parts = tx
        .meta()
        .spread
        .as_ref()
        .and_then(|spread| split_by_shares(flow.value, &spread.shares));
    match parts {
        Some(parts) => parts
            .into_iter()
            .map(|(month, value)| Flow {
                month,
                value,
                ..flow.clone()
            })
            .collect(),
        None => {
            let month = whole_month(tx, &flow.month);
            vec![Flow { month, ..flow }]
        }
    }
}

/// Whether the month `YYYY-MM` overlaps `[from, to)`.
fn month_overlaps(cal: &Calendar, month: &str, from: i64, to: i64) -> bool {
    let Some(first) = parse_month(month).map(first_of_month) else {
        return false;
    };
    let next = first
        .checked_add_months(Months::new(1))
        .unwrap_or(NaiveDate::MAX);
    cal.start_of(first) < to && cal.start_of(next) > from
}

/// The months a period or spread of `tx` touches, as `YYYY-MM` keys.
fn touched_months(tx: &Transaction) -> Vec<String> {
    let meta = tx.meta();
    let mut months: Vec<String> = meta
        .spread
        .iter()
        .flat_map(|spread| spread.shares.iter().map(|share| share.month.clone()))
        .collect();
    if let Some(period) = &meta.period {
        for bound in [&period.start, &period.end] {
            if let Ok(day) = NaiveDate::parse_from_str(bound, "%Y-%m-%d") {
                months.push(month_key(day));
            }
        }
    }
    months
}

/// Whether `tx` can have a flow in `[from, to)` under the period reading: its
/// money date is inside, or a month it touches overlaps the window. Anything
/// else is skipped before it is valued.
pub(crate) fn may_reach_window(cal: &Calendar, tx: &Transaction, from: i64, to: i64) -> bool {
    let secs = secs_or_now(&tx.meta().timestamp);
    (secs >= from && secs < to)
        || touched_months(tx)
            .iter()
            .any(|month| month_overlaps(cal, month, from, to))
}

/// Whether a month's flow of `tx` belongs to `[from, to)`: an operation
/// without a period by its money date (as `flows_between`); one with a period
/// also when the month it counts in overlaps the window.
fn in_window(cal: &Calendar, tx: &Transaction, flow: &Flow, from: i64, to: i64) -> bool {
    let secs = secs_or_now(&tx.meta().timestamp);
    let paid_inside = secs >= from && secs < to && flow.month == month_key(flow.date);
    if tx.meta().period.is_none() && tx.meta().spread.is_none() {
        return paid_inside;
    }
    paid_inside || month_overlaps(cal, &flow.month, from, to)
}

/// Classified flows of the window read by `basis`. `MoneyDate` is
/// `flows_between`; `Period` counts every operation in the month it is «for»
/// (R2): its spread by shares, a period inside one month whole there, anything
/// else whole in the month of the money date. An operation without a period
/// reads the same in both (R1, R8). The spending of shared events has no
/// period and counts by its date either way.
pub fn flows_for_basis(
    books: &Books<'_>,
    cal: &Calendar,
    currency: &str,
    from: i64,
    to: i64,
    basis: Basis,
) -> Vec<Flow> {
    if basis == Basis::MoneyDate {
        return flows_between(books, cal, currency, from, to);
    }
    let mut flows: Vec<Flow> = books
        .doc()
        .transactions
        .iter()
        .filter(|tx| may_reach_window(cal, tx, from, to))
        .flat_map(|tx| {
            period_flows_of(books, cal, currency, tx)
                .into_iter()
                .filter(|flow| in_window(cal, tx, flow, from, to))
        })
        .collect();
    flows.extend(extra_flows_from(
        books.doc(),
        books.shared_extras(),
        books.rate_book(),
        cal,
        currency,
        from,
        to,
    ));
    flows
}
