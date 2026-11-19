//! Scenarios of the agenda area: reminders, price alerts and the list of
//! what is coming up. Each decides in `planning::agenda` and ends in one
//! `vault.transact`; only this file reads the clock. The forecast of the
//! planned automations lives here because it asks the vault for balances and
//! occurrences of another area.

use std::collections::{HashMap, HashSet};

use crate::Amount;
use crate::calendar::{self, Calendar};
use crate::error::{Error, Result};
use crate::model::planning::{Automation, PriceAlert, Reminder, ReminderPay, ReminderRepeat};
use crate::model::store::StoreKind;
use crate::model::transaction::Transaction;
use crate::model::vault::now_iso;
use crate::planning::agenda::{self as rules, PriceAlertDraft, UpcomingItem, UpcomingKind, Window};
use crate::planning::automations::outflow;
use crate::pricing::history::secs_or_now;
use crate::service::UnlockedVault;

fn missing_reminder(id: &str) -> Error {
    Error::Missing(format!("reminder {id}"))
}

fn missing_alert(id: &str) -> Error {
    Error::Missing(format!("price alert {id}"))
}

/// Change the reminder with this id and hand back the changed copy.
fn change_reminder(
    vault: &mut UnlockedVault,
    id: &str,
    change: impl FnOnce(&mut Reminder) -> Result<()>,
) -> Result<Reminder> {
    vault.transact(|tx| {
        tx.collection::<Reminder>()
            .update(id, |reminder| {
                change(reminder)?;
                Ok(reminder.clone())
            })
            .map_err(|_| missing_reminder(id))?
    })
}

pub fn create_reminder(
    vault: &mut UnlockedVault,
    text: &str,
    due_at: &str,
    repeat: Option<ReminderRepeat>,
) -> Result<Reminder> {
    create_reminder_paying(vault, text, due_at, repeat, None)
}

/// A reminder with "Pay" in it.
pub fn create_reminder_paying(
    vault: &mut UnlockedVault,
    text: &str,
    due_at: &str,
    repeat: Option<ReminderRepeat>,
    pay: Option<ReminderPay>,
) -> Result<Reminder> {
    let reminder = rules::new_reminder(&vault.doc, text, due_at, repeat, pay)?;
    vault.transact(|tx| {
        tx.collection::<Reminder>().insert(reminder.clone())?;
        Ok(reminder)
    })
}

/// Edit what and when. An automatic reminder edited by hand keeps its
/// time from then on: the sync no longer moves it.
pub fn update_reminder(
    vault: &mut UnlockedVault,
    id: &str,
    text: &str,
    due_at: &str,
    repeat: Option<ReminderRepeat>,
) -> Result<Reminder> {
    rules::check_reminder(text, due_at)?;
    change_reminder(vault, id, |reminder| {
        rules::edit_reminder(reminder, text, due_at, repeat);
        Ok(())
    })
}

/// Edit what, when and what "Pay" fills in.
pub fn update_reminder_paying(
    vault: &mut UnlockedVault,
    id: &str,
    text: &str,
    due_at: &str,
    repeat: Option<ReminderRepeat>,
    pay: Option<ReminderPay>,
) -> Result<Reminder> {
    let pay = rules::clean_pay(&vault.doc, pay)?;
    if !vault.doc.reminders.iter().any(|reminder| reminder.id == id) {
        return Err(missing_reminder(id));
    }
    rules::check_reminder(text, due_at)?;
    change_reminder(vault, id, |reminder| {
        reminder.pay = pay;
        rules::edit_reminder(reminder, text, due_at, repeat);
        Ok(())
    })
}

/// Done. A repeating reminder is not closed but moves on to its next date
/// after now — «paid this month» leaves next month's in place.
pub fn set_reminder_done(vault: &mut UnlockedVault, id: &str, done: bool) -> Result<Reminder> {
    let now = calendar::now_secs();
    change_reminder(vault, id, |reminder| rules::mark_done(reminder, done, now))
}

/// Stop a repeating reminder: it is done for good.
pub fn finish_reminder_series(vault: &mut UnlockedVault, id: &str) -> Result<Reminder> {
    change_reminder(vault, id, |reminder| {
        rules::finish_series(reminder);
        Ok(())
    })
}

/// Put a reminder off: it is not due before `until`.
pub fn snooze_reminder(vault: &mut UnlockedVault, id: &str, until: &str) -> Result<Reminder> {
    rules::check_snooze(until)?;
    change_reminder(vault, id, |reminder| {
        reminder.snoozed_until = Some(until.to_string());
        Ok(())
    })
}

/// Delete a reminder. An automatic one is muted rather than removed: its
/// source would make it again with the next sync, and a muted one stays
/// gone (and hidden) until the source itself is gone.
pub fn delete_reminder(vault: &mut UnlockedVault, id: &str) -> Result<()> {
    let reminder = vault
        .doc
        .reminders
        .iter()
        .find(|reminder| reminder.id == id)
        .ok_or_else(|| missing_reminder(id))?;
    if reminder.source.is_some() {
        return finish_reminder_series(vault, id).map(|_| ());
    }
    vault.transact(|tx| {
        tx.collection::<Reminder>()
            .remove(id)
            .map(|_| ())
            .map_err(|_| missing_reminder(id))
    })
}

pub fn create_price_alert(vault: &mut UnlockedVault, draft: PriceAlertDraft) -> Result<PriceAlert> {
    rules::check_alert(&vault.doc, &draft)?;
    let alert = rules::new_alert(draft, uuid::Uuid::new_v4().to_string());
    vault.transact(|tx| {
        tx.collection::<PriceAlert>().insert(alert.clone())?;
        Ok(alert)
    })
}

/// Edit an alert; an edited alert is armed again.
pub fn update_price_alert(
    vault: &mut UnlockedVault,
    id: &str,
    draft: PriceAlertDraft,
) -> Result<PriceAlert> {
    rules::check_alert(&vault.doc, &draft)?;
    vault.transact(|tx| {
        tx.collection::<PriceAlert>()
            .update(id, |alert| {
                rules::rearm_with(alert, draft);
                alert.clone()
            })
            .map_err(|_| missing_alert(id))
    })
}

pub fn delete_price_alert(vault: &mut UnlockedVault, id: &str) -> Result<()> {
    vault.transact(|tx| {
        tx.collection::<PriceAlert>()
            .remove(id)
            .map(|_| ())
            .map_err(|_| missing_alert(id))
    })
}

/// Fire every armed alert whose condition holds at the cached price.
/// A one-off alert stays triggered until edited; a repeating one is
/// armed again once the price is back on the other side.
pub fn check_alerts(vault: &mut UnlockedVault, now: i64) -> Result<Vec<PriceAlert>> {
    let book = vault.rate_book();
    let price_of = |alert: &PriceAlert| {
        let usd = book.price_at(&alert.asset.key(), now)?;
        book.convert(usd, "usd", &alert.currency, now)
    };
    let mut alerts = vault.doc.price_alerts.clone();
    let (fired, changed) = rules::fire_alerts(&mut alerts, price_of, &now_iso());
    if changed {
        vault.transact(|tx| {
            tx.doc_mut().price_alerts = alerts;
            Ok(())
        })?;
    }
    Ok(fired)
}

/// A forecast balance, seeded from the real one on first use.
fn forecast_balance(
    vault: &UnlockedVault,
    balances: &mut HashMap<(String, String), Amount>,
    store_id: &str,
    key: &str,
    now: i64,
) -> Amount {
    *balances
        .entry((store_id.to_string(), key.to_string()))
        .or_insert_with(|| vault.balance_at(store_id, key, now))
}

/// The planned automations of the window, with a running balance forecast
/// per (store, asset), and the loans they pay.
fn automation_items(
    vault: &UnlockedVault,
    automations: &[Automation],
    cal: &Calendar,
    now: i64,
    horizon: i64,
) -> (Vec<UpcomingItem>, HashSet<String>) {
    let mut out = Vec::new();
    let mut planned: Vec<(i64, String, Transaction)> = Vec::new();
    let mut paid_loans: HashSet<String> = HashSet::new();
    for automation in automations.iter().filter(|item| item.active) {
        for (secs, tx) in vault.automation_occurrences(automation, cal, now, horizon - 1) {
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
            let is_loan = vault
                .find_store(&flow.store_id)
                .is_some_and(|store| store.kind == StoreKind::Liability);
            let available =
                forecast_balance(vault, &mut balances, &flow.store_id, &flow.asset.key(), now);
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
                forecast_balance(vault, &mut balances, &leg.store_id, &leg.asset.key(), now);
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
pub fn upcoming(vault: &UnlockedVault, days: u32, now: i64, zone: &str) -> Vec<UpcomingItem> {
    let cal = Calendar::new(zone);
    let window = Window::new(&cal, days, now);
    let automations = vault.automations();
    let (mut out, paid_loans) = automation_items(vault, &automations, &cal, now, window.horizon);
    out.extend(rules::housing_items(&vault.doc, &cal, &window));
    out.extend(rules::loan_items(&vault.doc, &paid_loans, &cal, &window));
    out.extend(rules::reminder_items(&vault.doc, &cal, &window));
    let balances = vault.contact_balances();
    out.extend(rules::debt_items(
        &vault.doc,
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
        &vault.doc,
        &automations,
        vault.automation_runs(None),
        &cal,
    ));
    out.sort_by_key(|item| secs_or_now(&item.at));
    out
}
