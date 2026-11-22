//! "Mine" (redesign Auto-1 · 1): everything that fires on its own in one
//! list — schemes, repeats, reminders and price alerts — with when each fires
//! next and what it will do. Before, that took five screens. The rows that
//! come from the document are built here; the rows that read workflow graphs
//! are in `usecases::planning::my_automations` (`planning` may not use
//! `workflow`).

use chrono::Duration;

use mwt_base::Amount;
use mwt_base::calendar;
use mwt_base::error::{Error, Result};
use mwt_base::time::unix_secs;
use mwt_model::AssetRef;
use mwt_model::Transaction;
use mwt_model::VaultDoc;
use mwt_model::{AlertCondition, Automation, PriceAlert, Reminder};

/// How far ahead "Next time" looks: a yearly schedule still has a date.
const LOOKAHEAD_DAYS: i64 = 400;

/// The moment "Next time" looks up to, from `now`.
pub fn lookahead_until(now: i64) -> i64 {
    now + Duration::days(LOOKAHEAD_DAYS).num_seconds()
}

/// What kind of thing fires.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum MineKind {
    /// A workflow the user drew.
    Scheme,
    /// An automation: an operation on a schedule.
    Repeat,
    Reminder,
    Alert,
}

/// One row of "Mine".
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct MineItem {
    pub kind: MineKind,
    /// The workflow, automation, reminder or alert id.
    pub id: String,
    pub name: String,
    /// Switched on: a reminder that is not muted, an alert that has not fired.
    pub active: bool,
    /// When it fires next, RFC3339; `None` for an alert (it waits for the
    /// price) and for a scheme without a schedule.
    pub next_at: Option<String>,
    /// The operation it makes: signed as it moves the money (an income
    /// positive, a payment negative, a transfer by what arrives).
    pub amount: Option<Amount>,
    pub asset: Option<AssetRef>,
    /// The wallet the money leaves.
    pub from_store: Option<String>,
    /// The wallet the money comes to.
    pub to_store: Option<String>,
    /// It asks "yes" before it does anything.
    pub asks: bool,
    /// It only notifies: a reminder, an alert, a scheme without operations.
    pub notifies: bool,
    /// For an alert: above or below `amount` of `asset`, in `currency`.
    pub condition: Option<AlertCondition>,
    pub currency: Option<String>,
    /// The scheme's own description.
    pub description: String,
}

impl MineItem {
    pub fn new(kind: MineKind, id: &str, name: &str, active: bool) -> Self {
        Self {
            kind,
            id: id.to_string(),
            name: name.to_string(),
            active,
            next_at: None,
            amount: None,
            asset: None,
            from_store: None,
            to_store: None,
            asks: false,
            notifies: false,
            condition: None,
            currency: None,
            description: String::new(),
        }
    }

    /// The money of an operation: where it leaves, where it comes, how much.
    pub fn with_operation(mut self, tx: &Transaction) -> Self {
        let legs = tx.legs();
        let out = legs.iter().find(|leg| leg.amount.is_negative());
        let into = legs.iter().find(|leg| leg.amount.is_positive());
        self.from_store = out.map(|leg| leg.store_id.clone());
        self.to_store = into.map(|leg| leg.store_id.clone());
        if let Some(leg) = into.or(out) {
            self.amount = Some(leg.amount);
            self.asset = Some(leg.asset.clone());
        }
        self
    }
}

/// The row of a repeat: its next operation, when it has one.
pub fn repeat_item(automation: &Automation, next: Option<(i64, Transaction)>) -> MineItem {
    let mut item = MineItem::new(
        MineKind::Repeat,
        &automation.id,
        &automation.name,
        automation.active,
    );
    if let Some((moment, tx)) = next {
        item.next_at = Some(calendar::iso(moment));
        item = item.with_operation(&tx);
    }
    item
}

/// The rows of the reminders that are not done.
pub fn reminder_items(doc: &VaultDoc) -> Vec<MineItem> {
    doc.reminders
        .iter()
        .filter(|reminder| !reminder.done)
        .map(|reminder| {
            let mut item = MineItem::new(
                MineKind::Reminder,
                &reminder.id,
                &reminder.text,
                !reminder.muted,
            );
            item.next_at = Some(
                reminder
                    .snoozed_until
                    .clone()
                    .unwrap_or_else(|| reminder.due_at.clone()),
            );
            item.notifies = true;
            item
        })
        .collect()
}

/// The rows of the price alerts.
pub fn alert_items(doc: &VaultDoc) -> Vec<MineItem> {
    doc.price_alerts
        .iter()
        .map(|alert| {
            let name = alert.asset.symbol.to_uppercase();
            let mut item = MineItem::new(
                MineKind::Alert,
                &alert.id,
                &name,
                alert.active && (alert.repeats || !alert.is_triggered()),
            );
            item.amount = Some(alert.price);
            item.asset = Some(alert.asset.clone());
            item.condition = Some(alert.condition);
            item.currency = Some(alert.currency.clone());
            item.notifies = true;
            item
        })
        .collect()
}

/// The soonest first; alerts and schemes without a date after them, by name.
pub fn sort_items(items: &mut [MineItem]) {
    let when = |item: &MineItem| item.next_at.as_deref().and_then(unix_secs);
    // `None` after every date, then by name.
    items.sort_by_cached_key(|item| (when(item).is_none(), when(item), item.name.to_lowercase()));
}

/// Mute a reminder or switch it back on.
pub fn set_reminder_active(reminders: &mut [Reminder], id: &str, active: bool) -> Result<()> {
    let reminder = reminders
        .iter_mut()
        .find(|reminder| reminder.id == id)
        .ok_or_else(|| Error::Missing(format!("reminder {id}")))?;
    reminder.muted = !active;
    Ok(())
}

/// Switch an alert; one switched back on waits for the price again.
pub fn set_alert_active(alerts: &mut [PriceAlert], id: &str, active: bool) -> Result<()> {
    let alert = alerts
        .iter_mut()
        .find(|alert| alert.id == id)
        .ok_or_else(|| Error::Missing(format!("price alert {id}")))?;
    alert.active = active;
    if active {
        alert.triggered_at = None;
        alert.triggered_for = None;
    }
    Ok(())
}
