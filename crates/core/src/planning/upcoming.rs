//! What is coming up: the planned automations with a running balance
//! forecast, rent, loans, reminders, debts and rejected runs. A read of the
//! books; it never transacts.

use std::collections::{HashMap, HashSet};

use crate::Amount;
use crate::calendar::{self, Calendar};
use crate::household::contacts::contact_balances_until;
use crate::model::planning::Automation;
use crate::model::store::StoreKind;
use crate::model::transaction::Transaction;
use crate::money::books::Books;
use crate::money::tree::find_store;
use crate::planning::agenda::{self as rules, UpcomingItem, UpcomingKind, Window};
use crate::planning::automation_plan::{automation_occurrences, automation_runs, automations};
use crate::planning::automations::{balance_at, outflow};
use crate::time::secs_or_now;

/// A forecast balance, seeded from the real one on first use.
fn forecast_balance(
    books: &Books<'_>,
    balances: &mut HashMap<(String, String), Amount>,
    store_id: &str,
    key: &str,
    now: i64,
) -> Amount {
    *balances
        .entry((store_id.to_string(), key.to_string()))
        .or_insert_with(|| balance_at(books.doc(), store_id, key, now))
}

/// The planned automations of the window, with a running balance forecast
/// per (store, asset), and the loans they pay.
fn automation_items(
    books: &Books<'_>,
    automations: &[Automation],
    cal: &Calendar,
    now: i64,
    horizon: i64,
) -> (Vec<UpcomingItem>, HashSet<String>) {
    let mut out = Vec::new();
    let mut planned: Vec<(i64, String, Transaction)> = Vec::new();
    let mut paid_loans: HashSet<String> = HashSet::new();
    for automation in automations.iter().filter(|item| item.active) {
        for (secs, tx) in automation_occurrences(automation, cal, now, horizon - 1) {
            if let Transaction::Transfer(inner) = &tx
                && let Some(target) = inner.to.store_id()
            {
                paid_loans.insert(target.to_string());
            }
            planned.push((secs, automation.id.clone(), tx));
        }
    }
    planned.sort_by_key(|(secs, _, _)| *secs);
    let mut balances: HashMap<(String, String), Amount> = HashMap::new();
    for (secs, automation_id, tx) in planned {
        let name = automations
            .iter()
            .find(|item| item.id == automation_id)
            .map(|item| item.name.clone())
            .unwrap_or_default();
        let estimated = tx.meta().origin.is_estimated();
        let mut item = UpcomingItem {
            kind: UpcomingKind::Automation,
            source_id: automation_id,
            at: calendar::iso(secs),
            date: rules::local_day(cal, secs),
            title: name,
            store_id: None,
            amount: None,
            asset: None,
            shortfall: None,
            estimated,
        };
        if let Some(flow) = outflow(&tx) {
            let fee_same = flow
                .fee
                .as_ref()
                .filter(|(fee_asset, _)| fee_asset.key() == flow.asset.key())
                .map_or(Amount::ZERO, |(_, value)| *value);
            let required = flow.amount + fee_same;
            let is_loan = find_store(books.doc(), &flow.store_id)
                .is_some_and(|store| store.kind == StoreKind::Liability);
            let available =
                forecast_balance(books, &mut balances, &flow.store_id, &flow.asset.key(), now);
            if !is_loan && available < required {
                item.shortfall = Some(required - available.max(Amount::ZERO));
            }
            item.store_id = Some(flow.store_id.clone());
            item.amount = Some(Amount::ZERO - flow.amount);
            item.asset = Some(flow.asset.clone());
        } else if let Some(leg) = tx.legs().into_iter().find(|leg| leg.amount.is_positive()) {
            item.store_id = Some(leg.store_id.clone());
            item.amount = Some(leg.amount);
            item.asset = Some(leg.asset.clone());
        }
        for leg in tx.legs() {
            let current =
                forecast_balance(books, &mut balances, &leg.store_id, &leg.asset.key(), now);
            balances.insert(
                (leg.store_id.clone(), leg.asset.key()),
                current + leg.amount,
            );
        }
        out.push(item);
    }
    (out, paid_loans)
}

/// What is coming up in the next `days` days, oldest first, plus rejected
/// automation runs still waiting for a decision.
pub fn upcoming(books: &Books<'_>, days: u32, now: i64, zone: &str) -> Vec<UpcomingItem> {
    let cal = Calendar::new(zone);
    let window = Window::new(&cal, days, now);
    let doc = books.doc();
    let automations = automations(books);
    let (mut out, paid_loans) = automation_items(books, &automations, &cal, now, window.horizon);
    out.extend(rules::housing_items(doc, &cal, &window));
    out.extend(rules::loan_items(doc, &paid_loans, &cal, &window));
    out.extend(rules::reminder_items(doc, &cal, &window));
    let balances = contact_balances_until(books, i64::MAX);
    out.extend(rules::debt_items(
        doc,
        |terms| {
            balances
                .iter()
                .find(|row| {
                    row.contact_id == terms.contact_id
                        && row.asset.symbol.eq_ignore_ascii_case(&terms.asset)
                })
                .map(|row| (row.receivable - row.payable, row.asset.clone()))
        },
        &cal,
        &window,
    ));
    out.extend(rules::run_items(
        doc,
        &automations,
        automation_runs(books, None),
        &cal,
    ));
    out.sort_by_key(|item| secs_or_now(&item.at));
    out
}
