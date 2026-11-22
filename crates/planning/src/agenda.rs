//! Reminders, price alerts and the list of what is coming up (plan §7.2,
//! §7.7): planned automations with a forecast of whether the money will be
//! there, rent and loan payments by their due day, reminders, and runs that
//! were rejected and wait for a decision. The rules read a document and take
//! the moment as an argument; the scenarios are in `usecases::planning::agenda`.

use std::collections::HashSet;

use chrono::{Duration, NaiveDate};

use crate::automations::outflow;
use mwt_base::Amount;
use mwt_base::calendar::{self, AUTOMATION_HOUR, Calendar, clamp_day, first_of_month, next_month};
use mwt_base::error::{Error, Result};
use mwt_base::time::{secs_or_now, unix_secs};
use mwt_model::DebtTerms;
use mwt_model::VaultDoc;
use mwt_model::{
    AlertCondition, Automation, AutomationOutcome, AutomationRun, PriceAlert, Reminder,
    ReminderPay, ReminderRepeat, UtilityKind,
};
use mwt_model::{AssetRef, StoreKind};

/// How many dates of one repeating reminder the agenda lists at most.
const MAX_REPEATS_LISTED: usize = 60;

/// When a reminder is due: its date, or the end of its snooze if later.
pub fn reminder_secs(reminder: &Reminder) -> i64 {
    let due = secs_or_now(&reminder.due_at);
    reminder
        .snoozed_until
        .as_deref()
        .map_or(due, |until| due.max(secs_or_now(until)))
}

/// Whether a reminder is waiting for the user at `now`.
pub(crate) fn reminder_is_due(reminder: &Reminder, now: i64) -> bool {
    !reminder.done && !reminder.muted && reminder_secs(reminder) <= now
}

/// The first date of a repeat after both `due_at` and `now`, RFC3339.
pub fn next_occurrence(repeat: ReminderRepeat, due_at: &str, now: i64) -> Option<String> {
    let anchor = chrono::DateTime::parse_from_rfc3339(due_at)
        .ok()?
        .with_timezone(&chrono::Utc);
    // Counted from the due date each time, so the 31st stays the 31st; a
    // long-forgotten daily reminder is far behind: step until past now.
    (1..100_000)
        .map_while(|count| repeat.nth(anchor, count))
        .find(|at| at.timestamp() > now)
        .map(|at| at.format("%Y-%m-%dT%H:%M:%SZ").to_string())
}

/// What an alert is: everything the user edits.
#[derive(Debug, Clone, PartialEq)]
pub struct PriceAlertDraft {
    pub asset: AssetRef,
    pub condition: AlertCondition,
    pub price: Amount,
    pub currency: String,
    pub active: bool,
    /// The exchange whose terminal sets the alert.
    pub store_id: Option<String>,
    /// Fires on every crossing instead of once.
    pub repeats: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum UpcomingKind {
    Automation,
    Housing,
    LoanPayment,
    Reminder,
    /// A rejected automation run waiting for "Retry / Skip".
    RejectedRun,
    /// An operation an automation booked with an approximate amount, waiting
    /// for the real one; `source_id` is the operation's id.
    UnconfirmedRun,
    /// A debt with a contact falls due (overdue ones stay listed).
    Debt,
}

/// One thing on the agenda.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct UpcomingItem {
    pub kind: UpcomingKind,
    /// Automation, utility item, store, reminder or run id.
    pub source_id: String,
    /// RFC3339 moment.
    pub at: String,
    /// Local date, `YYYY-MM-DD`.
    pub date: String,
    /// Automation or item name, store name, reminder text.
    pub title: String,
    pub store_id: Option<String>,
    /// Signed: money in is positive, money out negative.
    pub amount: Option<Amount>,
    pub asset: Option<AssetRef>,
    /// For money going out: how much will be missing by then, at the
    /// balance forecast from today's balance and everything planned before.
    pub shortfall: Option<Amount>,
    /// The amount is approximate: an automation whose amount differs each time.
    #[serde(default)]
    pub estimated: bool,
}

/// The reminders, the soonest first.
pub fn reminders(doc: &VaultDoc) -> Vec<Reminder> {
    let mut out = doc.reminders.clone();
    out.sort_by(|left, right| left.due_at.cmp(&right.due_at));
    out
}

pub fn check_reminder(text: &str, due_at: &str) -> Result<()> {
    if text.trim().is_empty() {
        return Err(Error::InvalidInput("write what to remind about".into()));
    }
    if unix_secs(due_at).is_none() {
        return Err(Error::InvalidInput("choose when to remind".into()));
    }
    Ok(())
}

/// An empty button is no button; a wallet must exist.
pub fn clean_pay(doc: &VaultDoc, pay: Option<ReminderPay>) -> Result<Option<ReminderPay>> {
    let Some(pay) = pay else { return Ok(None) };
    let pick = |value: Option<String>| {
        value
            .map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty())
    };
    let pay = ReminderPay {
        store_id: pick(pay.store_id),
        category: pick(pay.category),
    };
    if let Some(store_id) = &pay.store_id
        && !doc.stores.iter().any(|store| store.id == *store_id)
    {
        return Err(Error::NotFound(format!("store {store_id}")));
    }
    Ok((pay.store_id.is_some() || pay.category.is_some()).then_some(pay))
}

fn clean_repeat(repeat: ReminderRepeat) -> ReminderRepeat {
    ReminderRepeat {
        every: repeat.every.clamp(1, 999),
        ..repeat
    }
}

/// A new reminder, checked: its repeat in range and its "Pay" button real.
pub fn new_reminder(
    doc: &VaultDoc,
    text: &str,
    due_at: &str,
    repeat: Option<ReminderRepeat>,
    pay: Option<ReminderPay>,
) -> Result<Reminder> {
    check_reminder(text, due_at)?;
    Ok(Reminder {
        repeat: repeat.map(clean_repeat),
        pay: clean_pay(doc, pay)?,
        ..Reminder::new(text, due_at)
    })
}

/// Edit what and when. An automatic reminder edited by hand keeps its
/// time from then on: the sync no longer moves it.
pub fn edit_reminder(
    reminder: &mut Reminder,
    text: &str,
    due_at: &str,
    repeat: Option<ReminderRepeat>,
) {
    reminder.text = text.trim().to_string();
    reminder.due_at = due_at.to_string();
    reminder.repeat = repeat.map(clean_repeat);
    reminder.snoozed_until = None;
    if reminder.source.is_some() {
        reminder.user_edited = true;
    }
}

/// Done. A repeating reminder is not closed but moves on to its next date
/// after `now` — «paid this month» leaves next month's in place.
pub fn mark_done(reminder: &mut Reminder, done: bool, now: i64) -> Result<()> {
    let next = match (done, reminder.repeat) {
        (true, Some(repeat)) => Some(
            next_occurrence(repeat, &reminder.due_at, now)
                .ok_or_else(|| Error::InvalidInput("the reminder has no next date".into()))?,
        ),
        _ => None,
    };
    reminder.snoozed_until = None;
    match next {
        Some(due_at) => {
            reminder.due_at = due_at;
            reminder.done = false;
        }
        None => reminder.done = done,
    }
    Ok(())
}

/// Stop a repeating reminder: it is done for good.
pub fn finish_series(reminder: &mut Reminder) {
    reminder.repeat = None;
    reminder.done = true;
    reminder.snoozed_until = None;
    if reminder.source.is_some() {
        // An automatic one must not come back with the next sync.
        reminder.muted = true;
    }
}

/// The moment a snooze ends must be a real one.
pub fn check_snooze(until: &str) -> Result<()> {
    if unix_secs(until).is_none() {
        return Err(Error::InvalidInput("choose when to remind again".into()));
    }
    Ok(())
}

/// Open reminders whose time has come (a snoozed one when its snooze ends).
pub fn due_reminders(doc: &VaultDoc, now: i64) -> Vec<Reminder> {
    reminders(doc)
        .into_iter()
        .filter(|reminder| reminder_is_due(reminder, now))
        .collect()
}

// -- price alerts ---------------------------------------------------------

/// The alerts as the list shows them: a firing recorded for another
/// threshold is hidden (`triggered_at` cleared) in the answer only; the
/// stored record keeps it.
pub fn price_alerts(doc: &VaultDoc) -> Vec<PriceAlert> {
    doc.price_alerts
        .iter()
        .map(|alert| {
            let mut shown = alert.clone();
            if !alert.is_triggered() {
                shown.triggered_at = None;
            }
            shown
        })
        .collect()
}

pub fn check_alert(doc: &VaultDoc, draft: &PriceAlertDraft) -> Result<()> {
    if draft.asset.symbol.trim().is_empty() {
        return Err(Error::InvalidInput("choose an asset".into()));
    }
    if !draft.price.is_positive() {
        return Err(Error::InvalidInput("the price must be positive".into()));
    }
    if draft.currency.trim().is_empty() {
        return Err(Error::InvalidInput("choose a currency".into()));
    }
    if let Some(store_id) = &draft.store_id {
        let store = doc
            .stores
            .iter()
            .find(|store| store.id == *store_id)
            .ok_or_else(|| Error::Missing(format!("store {store_id}")))?;
        if store.kind != StoreKind::Exchange || store.parent_id.is_some() {
            return Err(Error::InvalidInput(
                "an alert belongs to an exchange".into(),
            ));
        }
    }
    Ok(())
}

/// A new armed alert of a checked draft, with the id given.
pub fn new_alert(draft: PriceAlertDraft, id: String) -> PriceAlert {
    PriceAlert {
        id,
        asset: draft.asset,
        condition: draft.condition,
        price: draft.price,
        currency: draft.currency.trim().to_lowercase(),
        active: draft.active,
        triggered_at: None,
        triggered_for: None,
        store_id: draft.store_id,
        repeats: draft.repeats,
    }
}

/// An edited alert is armed again.
pub fn rearm_with(alert: &mut PriceAlert, draft: PriceAlertDraft) {
    alert.asset = draft.asset;
    alert.condition = draft.condition;
    alert.price = draft.price;
    alert.currency = draft.currency.trim().to_lowercase();
    alert.active = draft.active;
    alert.triggered_at = None;
    alert.triggered_for = None;
    alert.store_id = draft.store_id;
    alert.repeats = draft.repeats;
}

/// Assets of armed alerts — what the scheduler refreshes prices for.
pub fn alert_assets(doc: &VaultDoc) -> Vec<AssetRef> {
    let mut seen = HashSet::new();
    doc.price_alerts
        .iter()
        .filter(|alert| alert.active && (!alert.is_triggered() || alert.repeats))
        .map(|alert| alert.asset.clone())
        .filter(|asset| seen.insert(asset.key()))
        .collect()
}

/// Fire every armed alert whose condition holds at the price `price_of`
/// gives, stamped `fired_at`. A one-off alert stays triggered until edited;
/// a repeating one is armed again once the price is back on the other side.
/// Returns the alerts that fired and whether anything changed at all.
pub fn fire_alerts(
    alerts: &mut [PriceAlert],
    price_of: impl Fn(&PriceAlert) -> Option<Amount>,
    fired_at: &str,
) -> (Vec<PriceAlert>, bool) {
    let mut fired = Vec::new();
    let mut rearmed = false;
    for alert in alerts
        .iter_mut()
        .filter(|alert| alert.active && (!alert.is_triggered() || alert.repeats))
    {
        let Some(price) = price_of(alert) else {
            continue;
        };
        let hit = match alert.condition {
            AlertCondition::Above => price >= alert.price,
            AlertCondition::Below => price <= alert.price,
        };
        if alert.is_triggered() {
            if !hit {
                alert.triggered_at = None;
                alert.triggered_for = None;
                rearmed = true;
            }
        } else if hit {
            alert.triggered_at = Some(fired_at.to_string());
            alert.triggered_for = Some(alert.fingerprint());
            fired.push(alert.clone());
        }
    }
    let changed = !fired.is_empty() || rearmed;
    (fired, changed)
}

// -- upcoming -------------------------------------------------------------

/// The window the agenda lists: from today to `last`, the moment after it
/// being `horizon`.
pub struct Window {
    pub today: NaiveDate,
    pub last: NaiveDate,
    pub horizon: i64,
}

impl Window {
    pub fn new(cal: &Calendar, days: u32, now: i64) -> Self {
        let today = cal.date_of(now);
        let last = today + Duration::days(i64::from(days));
        let horizon = cal.start_of(last + Duration::days(1));
        Self {
            today,
            last,
            horizon,
        }
    }

    /// The dates of a monthly due day inside the window.
    fn due_dates(&self, due_day: u8) -> Vec<NaiveDate> {
        let mut dates = Vec::new();
        let mut month = first_of_month(self.today);
        while month <= self.last {
            let date = clamp_day(month, due_day);
            if date >= self.today && date <= self.last {
                dates.push(date);
            }
            month = next_month(month);
        }
        dates
    }
}

fn day_text(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

fn bare_item(
    kind: UpcomingKind,
    source_id: String,
    at: String,
    date: String,
    title: String,
) -> UpcomingItem {
    UpcomingItem {
        kind,
        source_id,
        at,
        date,
        title,
        store_id: None,
        amount: None,
        asset: None,
        shortfall: None,
        estimated: false,
    }
}

/// Rent by its due day, unless an automation pays it.
pub fn housing_items(doc: &VaultDoc, cal: &Calendar, window: &Window) -> Vec<UpcomingItem> {
    let mut out = Vec::new();
    for item in doc.utility_items.iter().filter(|item| item.active) {
        if let UtilityKind::Housing {
            amount,
            due_day,
            automation_id: None,
            ..
        } = &item.kind
        {
            for date in window.due_dates(*due_day) {
                let secs = cal.at_local(date, AUTOMATION_HOUR);
                out.push(UpcomingItem {
                    store_id: item.store_id.clone(),
                    amount: Some(Amount::ZERO - *amount),
                    asset: Some(AssetRef::fiat(
                        &item.currency,
                        &item.currency.to_uppercase(),
                    )),
                    ..bare_item(
                        UpcomingKind::Housing,
                        item.id.clone(),
                        calendar::iso(secs),
                        day_text(date),
                        item.name.clone(),
                    )
                });
            }
        }
    }
    out
}

/// Loan payments by their due day, except the loans an automation pays.
pub fn loan_items(
    doc: &VaultDoc,
    paid_loans: &HashSet<String>,
    cal: &Calendar,
    window: &Window,
) -> Vec<UpcomingItem> {
    let mut out = Vec::new();
    for store in &doc.stores {
        if store.kind != StoreKind::Liability || store.archived || paid_loans.contains(&store.id) {
            continue;
        }
        let Some(terms) = store.liability.as_ref() else {
            continue;
        };
        let (Some(payment), Some(due_day)) = (terms.monthly_payment, terms.due_day) else {
            continue;
        };
        for date in window.due_dates(due_day) {
            let secs = cal.at_local(date, AUTOMATION_HOUR);
            out.push(UpcomingItem {
                store_id: Some(store.id.clone()),
                amount: Some(Amount::ZERO - payment),
                ..bare_item(
                    UpcomingKind::LoanPayment,
                    store.id.clone(),
                    calendar::iso(secs),
                    day_text(date),
                    store.name.clone(),
                )
            });
        }
    }
    out
}

/// Reminders: overdue ones too, and every repeat within the horizon.
pub fn reminder_items(doc: &VaultDoc, cal: &Calendar, window: &Window) -> Vec<UpcomingItem> {
    let mut out = Vec::new();
    for reminder in doc
        .reminders
        .iter()
        .filter(|item| !item.done && !item.muted)
    {
        let mut at = calendar::iso(reminder_secs(reminder));
        for _ in 0..MAX_REPEATS_LISTED {
            let secs = secs_or_now(&at);
            if secs >= window.horizon {
                break;
            }
            out.push(bare_item(
                UpcomingKind::Reminder,
                reminder.id.clone(),
                at.clone(),
                day_text(cal.date_of(secs)),
                reminder.text.clone(),
            ));
            let Some(next) = reminder
                .repeat
                .and_then(|repeat| next_occurrence(repeat, &at, secs))
            else {
                break;
            };
            at = next;
        }
    }
    out
}

/// Debts with a due date and something still open; overdue ones too.
/// `open_debt` gives the net open amount of a debt's contact and its asset.
pub fn debt_items(
    doc: &VaultDoc,
    open_debt: impl Fn(&DebtTerms) -> Option<(Amount, AssetRef)>,
    cal: &Calendar,
    window: &Window,
) -> Vec<UpcomingItem> {
    let mut out = Vec::new();
    for terms in &doc.debt_terms {
        let Some(due) = terms.due_at.as_deref() else {
            continue;
        };
        let Ok(date) = NaiveDate::parse_from_str(due, "%Y-%m-%d") else {
            continue;
        };
        let Some((net, asset)) = open_debt(terms) else {
            continue;
        };
        if net.is_zero() {
            continue;
        }
        let secs = cal.at_local(date, AUTOMATION_HOUR);
        if secs >= window.horizon {
            continue;
        }
        let name = doc
            .contacts
            .iter()
            .find(|contact| contact.id == terms.contact_id)
            .map(|contact| contact.name.clone())
            .unwrap_or_default();
        out.push(UpcomingItem {
            amount: Some(net),
            asset: Some(asset),
            ..bare_item(
                UpcomingKind::Debt,
                terms.contact_id.clone(),
                calendar::iso(secs),
                due.to_string(),
                name,
            )
        });
    }
    out
}

/// Rejected runs wait for "Retry / Skip"; operations booked approximately
/// wait for their real amount.
pub fn run_items(
    doc: &VaultDoc,
    automations: &[Automation],
    runs: Vec<AutomationRun>,
    cal: &Calendar,
) -> Vec<UpcomingItem> {
    let mut out = Vec::new();
    for run in runs {
        let name = automations
            .iter()
            .find(|item| item.id == run.automation_id)
            .map(|item| item.name.clone())
            .unwrap_or_default();
        let secs = secs_or_now(&run.planned_at);
        let date = day_text(cal.date_of(secs));
        match &run.outcome {
            AutomationOutcome::Rejected { .. } => out.push(bare_item(
                UpcomingKind::RejectedRun,
                run.id.clone(),
                run.planned_at.clone(),
                date,
                name,
            )),
            AutomationOutcome::Created { tx_id } if run.estimated => {
                let Some(tx) = doc.transactions.iter().find(|tx| &tx.meta().id == tx_id) else {
                    continue;
                };
                let leg = tx.legs().into_iter().find(|leg| leg.amount.is_positive());
                let flow = outflow(tx);
                out.push(UpcomingItem {
                    store_id: flow
                        .as_ref()
                        .map(|flow| flow.store_id.clone())
                        .or_else(|| leg.as_ref().map(|leg| leg.store_id.clone())),
                    amount: flow
                        .as_ref()
                        .map(|flow| Amount::ZERO - flow.amount)
                        .or_else(|| leg.as_ref().map(|leg| leg.amount)),
                    asset: flow
                        .map(|flow| flow.asset)
                        .or_else(|| leg.map(|leg| leg.asset)),
                    estimated: true,
                    ..bare_item(
                        UpcomingKind::UnconfirmedRun,
                        tx_id.clone(),
                        run.planned_at.clone(),
                        date,
                        name,
                    )
                });
            }
            _ => {}
        }
    }
    out
}

/// The day of a moment as `YYYY-MM-DD` in the calendar's zone.
pub fn local_day(cal: &Calendar, secs: i64) -> String {
    day_text(cal.date_of(secs))
}
