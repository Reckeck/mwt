//! Money flows of the budget: every operation of a period classified once
//! (`classify_flow`) and valued in one currency at the rate of its day, plus
//! the spending of shared events that has no payment of mine behind it. The
//! queries take the `Books` of one request, so a request builds its rates
//! once.

use std::collections::HashMap;

use chrono::NaiveDate;

use crate::Amount;
use crate::calendar::Calendar;
use crate::household::shared_events::analytics::{category_group, extras_between, value_in};
use crate::kernel::books::Books;
use crate::model::VaultDoc;
use crate::model::transaction::{Leg, LegRole, Transaction};
use crate::money::rates::RateBook;
use crate::money::shared_ratios::SharedExtra;
use crate::money::worth::{FlowClass, classify_flow, spending};
use crate::time::secs_or_now;

#[derive(Debug, Clone)]
pub(crate) struct Flow {
    pub date: NaiveDate,
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
pub(crate) fn leg_value_in(
    book: &RateBook,
    leg: &Leg,
    currency: &str,
    secs: i64,
) -> Option<Amount> {
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

/// The flows of my own operations; a payment made for a shared event
/// counts for my part of it only.
fn payment_flows(
    books: &Books<'_>,
    cal: &Calendar,
    currency: &str,
    from: i64,
    to: i64,
) -> Vec<Flow> {
    let book = books.rate_book();
    let shared: &HashMap<String, Amount> = books.shared_ratios();
    books
        .doc()
        .transactions
        .iter()
        .filter_map(|tx| {
            let secs = secs_or_now(&tx.meta().timestamp);
            if secs < from || secs >= to {
                return None;
            }
            let class = classify(books, tx);
            if class == FlowClass::Ignored {
                return None;
            }
            let (mut value, missing) = flow_value(book, tx, class, currency);
            if let Some(ratio) = shared.get(&tx.meta().id) {
                value = value * *ratio;
            }
            Some(Flow {
                date: cal.date_of(secs),
                class,
                value,
                categories: tx.meta().categories.clone(),
                stores: tx.legs().into_iter().map(|leg| leg.store_id).collect(),
                tx_id: tx.meta().id.clone(),
                missing,
            })
        })
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
pub(crate) fn flows_between(
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
