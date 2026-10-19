//! Reminders, price alerts and the list of what is coming up (plan §7.2,
//! §7.7): planned automations with a forecast of whether the money will be
//! there, rent and loan payments by their due day, reminders, and runs that
//! were rejected and wait for a decision.

use std::collections::{HashMap, HashSet};

use chrono::Duration;

use crate::Amount;
use crate::calendar::{self, AUTOMATION_HOUR, Calendar, clamp_day, first_of_month, next_month};
use crate::error::{Error, Result};
use crate::model::planning::{
    AlertCondition, AutomationOutcome, PriceAlert, Reminder, ReminderRepeat, UtilityKind,
};
use crate::model::store::{AssetRef, StoreKind};
use crate::model::transaction::Transaction;
use crate::model::vault::now_iso;
use crate::pricing::history::{secs_or_now, unix_secs};

use super::UnlockedVault;
use super::automations::{draft_to_transaction, outflow, planned_dates};

/// How many dates of one repeating reminder the agenda lists at most.
const MAX_REPEATS_LISTED: usize = 60;

/// When a reminder is due: its date, or the end of its snooze if later.
pub(crate) fn reminder_secs(reminder: &Reminder) -> i64 {
    let due = secs_or_now(&reminder.due_at);
    reminder
        .snoozed_until
        .as_deref()
        .map_or(due, |until| due.max(secs_or_now(until)))
}

/// The first date of a repeat after both `due_at` and `now`, RFC3339.
pub(crate) fn next_occurrence(repeat: ReminderRepeat, due_at: &str, now: i64) -> Option<String> {
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum UpcomingKind {
    Automation,
    Housing,
    LoanPayment,
    Reminder,
    /// A rejected automation run waiting for «Повторить / Пропустить».
    RejectedRun,
    /// A debt with a contact falls due (overdue ones stay listed).
    Debt,
}

/// One thing on the agenda.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
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
}

impl UnlockedVault {
    // -- reminders ----------------------------------------------------------

    pub fn reminders(&self) -> Vec<Reminder> {
        let mut out = self.doc.reminders.clone();
        out.sort_by(|left, right| left.due_at.cmp(&right.due_at));
        out
    }

    fn check_reminder(text: &str, due_at: &str) -> Result<()> {
        if text.trim().is_empty() {
            return Err(Error::InvalidInput("write what to remind about".into()));
        }
        if unix_secs(due_at).is_none() {
            return Err(Error::InvalidInput("choose when to remind".into()));
        }
        Ok(())
    }

    pub fn create_reminder(
        &mut self,
        text: &str,
        due_at: &str,
        repeat: Option<ReminderRepeat>,
    ) -> Result<Reminder> {
        Self::check_reminder(text, due_at)?;
        let reminder = Reminder {
            repeat: repeat.map(Self::clean_repeat),
            ..Reminder::new(text, due_at)
        };
        self.doc.reminders.push(reminder.clone());
        self.persist()?;
        Ok(reminder)
    }

    /// Edit what and when. An automatic reminder edited by hand keeps its
    /// time from then on: the sync no longer moves it.
    pub fn update_reminder(
        &mut self,
        id: &str,
        text: &str,
        due_at: &str,
        repeat: Option<ReminderRepeat>,
    ) -> Result<Reminder> {
        Self::check_reminder(text, due_at)?;
        let reminder = self.find_reminder_mut(id)?;
        reminder.text = text.trim().to_string();
        reminder.due_at = due_at.to_string();
        reminder.repeat = repeat.map(Self::clean_repeat);
        reminder.snoozed_until = None;
        if reminder.source.is_some() {
            reminder.user_edited = true;
        }
        let out = reminder.clone();
        self.persist()?;
        Ok(out)
    }

    fn clean_repeat(repeat: ReminderRepeat) -> ReminderRepeat {
        ReminderRepeat {
            every: repeat.every.clamp(1, 999),
            ..repeat
        }
    }

    /// Done. A repeating reminder is not closed but moves on to its next date
    /// after now — «paid this month» leaves next month's in place.
    pub fn set_reminder_done(&mut self, id: &str, done: bool) -> Result<Reminder> {
        let now = calendar::now_secs();
        let reminder = self.find_reminder_mut(id)?;
        reminder.snoozed_until = None;
        match (done, reminder.repeat) {
            (true, Some(repeat)) => {
                reminder.due_at = next_occurrence(repeat, &reminder.due_at, now)
                    .ok_or_else(|| Error::InvalidInput("the reminder has no next date".into()))?;
                reminder.done = false;
            }
            _ => reminder.done = done,
        }
        let out = reminder.clone();
        self.persist()?;
        Ok(out)
    }

    /// Stop a repeating reminder: it is done for good.
    pub fn finish_reminder_series(&mut self, id: &str) -> Result<Reminder> {
        let reminder = self.find_reminder_mut(id)?;
        reminder.repeat = None;
        reminder.done = true;
        reminder.snoozed_until = None;
        if reminder.source.is_some() {
            // An automatic one must not come back with the next sync.
            reminder.muted = true;
        }
        let out = reminder.clone();
        self.persist()?;
        Ok(out)
    }

    /// Put a reminder off: it is not due before `until`.
    pub fn snooze_reminder(&mut self, id: &str, until: &str) -> Result<Reminder> {
        if unix_secs(until).is_none() {
            return Err(Error::InvalidInput("choose when to remind again".into()));
        }
        let reminder = self.find_reminder_mut(id)?;
        reminder.snoozed_until = Some(until.to_string());
        let out = reminder.clone();
        self.persist()?;
        Ok(out)
    }

    /// Delete a reminder. An automatic one is muted rather than removed: its
    /// source would make it again with the next sync, and a muted one stays
    /// gone (and hidden) until the source itself is gone.
    pub fn delete_reminder(&mut self, id: &str) -> Result<()> {
        if self.find_reminder_mut(id)?.source.is_some() {
            return self.finish_reminder_series(id).map(|_| ());
        }
        let before = self.doc.reminders.len();
        self.doc.reminders.retain(|reminder| reminder.id != id);
        if self.doc.reminders.len() == before {
            return Err(Error::Missing(format!("reminder {id}")));
        }
        self.persist()
    }

    fn find_reminder_mut(&mut self, id: &str) -> Result<&mut Reminder> {
        self.doc
            .reminders
            .iter_mut()
            .find(|reminder| reminder.id == id)
            .ok_or_else(|| Error::Missing(format!("reminder {id}")))
    }

    /// Open reminders whose time has come (a snoozed one when its snooze ends).
    pub fn due_reminders(&self, now: i64) -> Vec<Reminder> {
        self.reminders()
            .into_iter()
            .filter(|reminder| !reminder.done && !reminder.muted && reminder_secs(reminder) <= now)
            .collect()
    }

    // -- price alerts ---------------------------------------------------------

    pub fn price_alerts(&self) -> Vec<PriceAlert> {
        self.doc.price_alerts.clone()
    }

    fn check_alert(&self, draft: &PriceAlertDraft) -> Result<()> {
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
            let store = self
                .find_store(store_id)
                .ok_or_else(|| Error::Missing(format!("store {store_id}")))?;
            if store.kind != StoreKind::Exchange || store.parent_id.is_some() {
                return Err(Error::InvalidInput(
                    "an alert belongs to an exchange".into(),
                ));
            }
        }
        Ok(())
    }

    pub fn create_price_alert(&mut self, draft: PriceAlertDraft) -> Result<PriceAlert> {
        self.check_alert(&draft)?;
        let alert = PriceAlert {
            id: uuid::Uuid::new_v4().to_string(),
            asset: draft.asset,
            condition: draft.condition,
            price: draft.price,
            currency: draft.currency.trim().to_lowercase(),
            active: draft.active,
            triggered_at: None,
            store_id: draft.store_id,
        };
        self.doc.price_alerts.push(alert.clone());
        self.persist()?;
        Ok(alert)
    }

    /// Edit an alert; an edited alert is armed again.
    pub fn update_price_alert(&mut self, id: &str, draft: PriceAlertDraft) -> Result<PriceAlert> {
        self.check_alert(&draft)?;
        let alert = self
            .doc
            .price_alerts
            .iter_mut()
            .find(|alert| alert.id == id)
            .ok_or_else(|| Error::Missing(format!("price alert {id}")))?;
        alert.asset = draft.asset;
        alert.condition = draft.condition;
        alert.price = draft.price;
        alert.currency = draft.currency.trim().to_lowercase();
        alert.active = draft.active;
        alert.triggered_at = None;
        alert.store_id = draft.store_id;
        let out = alert.clone();
        self.persist()?;
        Ok(out)
    }

    pub fn delete_price_alert(&mut self, id: &str) -> Result<()> {
        let before = self.doc.price_alerts.len();
        self.doc.price_alerts.retain(|alert| alert.id != id);
        if self.doc.price_alerts.len() == before {
            return Err(Error::Missing(format!("price alert {id}")));
        }
        self.persist()
    }

    /// Assets of armed alerts — what the scheduler refreshes prices for.
    pub fn alert_assets(&self) -> Vec<AssetRef> {
        let mut seen = HashSet::new();
        self.doc
            .price_alerts
            .iter()
            .filter(|alert| alert.active && alert.triggered_at.is_none())
            .map(|alert| alert.asset.clone())
            .filter(|asset| seen.insert(asset.key()))
            .collect()
    }

    /// Fire every armed alert whose condition holds at the cached price.
    /// An alert fires once: it stays triggered until edited.
    pub fn check_alerts(&mut self, now: i64) -> Result<Vec<PriceAlert>> {
        let book = self.rate_book();
        let mut fired = Vec::new();
        for alert in self
            .doc
            .price_alerts
            .iter_mut()
            .filter(|alert| alert.active && alert.triggered_at.is_none())
        {
            let Some(usd) = book.price_at(&alert.asset.key(), now) else {
                continue;
            };
            let Some(price) = book.convert(usd, "usd", &alert.currency, now) else {
                continue;
            };
            let hit = match alert.condition {
                AlertCondition::Above => price >= alert.price,
                AlertCondition::Below => price <= alert.price,
            };
            if hit {
                alert.triggered_at = Some(now_iso());
                fired.push(alert.clone());
            }
        }
        if !fired.is_empty() {
            self.persist()?;
        }
        Ok(fired)
    }

    // -- upcoming -------------------------------------------------------------

    /// A forecast balance, seeded from the real one on first use.
    fn forecast_balance(
        &self,
        balances: &mut HashMap<(String, String), Amount>,
        store_id: &str,
        key: &str,
        now: i64,
    ) -> Amount {
        *balances
            .entry((store_id.to_string(), key.to_string()))
            .or_insert_with(|| self.balance_at(store_id, key, now))
    }

    /// What is coming up in the next `days` days, oldest first, plus rejected
    /// automation runs still waiting for a decision.
    pub fn upcoming(&self, days: u32, now: i64, zone: &str) -> Vec<UpcomingItem> {
        let cal = Calendar::new(zone);
        let today = cal.date_of(now);
        let last = today + Duration::days(i64::from(days));
        let horizon = cal.start_of(last + Duration::days(1));
        let mut out = Vec::new();

        // Automations, with a running balance forecast per (store, asset).
        let mut planned: Vec<(i64, String, Transaction)> = Vec::new();
        let mut paid_loans: HashSet<String> = HashSet::new();
        for automation in self.doc.automations.iter().filter(|item| item.active) {
            let Some(start) = cal.date_of_iso(&automation.starts_at) else {
                continue;
            };
            let after = Some(today - Duration::days(1));
            for date in planned_dates(&automation.schedule, start, after, last) {
                let secs = cal.at_local(date, AUTOMATION_HOUR);
                if secs <= now {
                    continue;
                }
                let iso = calendar::iso(secs);
                let tx = draft_to_transaction(&automation.template, &automation.id, &iso, &iso);
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
            let name = self
                .doc
                .automations
                .iter()
                .find(|item| item.id == automation_id)
                .map(|item| item.name.clone())
                .unwrap_or_default();
            let mut item = UpcomingItem {
                kind: UpcomingKind::Automation,
                source_id: automation_id,
                at: calendar::iso(secs),
                date: cal.date_of(secs).format("%Y-%m-%d").to_string(),
                title: name,
                store_id: None,
                amount: None,
                asset: None,
                shortfall: None,
            };
            if let Some(flow) = outflow(&tx) {
                let fee_same = flow
                    .fee
                    .as_ref()
                    .filter(|(fee_asset, _)| fee_asset.key() == flow.asset.key())
                    .map_or(Amount::ZERO, |(_, value)| *value);
                let required = flow.amount + fee_same;
                let is_loan = self
                    .find_store(&flow.store_id)
                    .is_some_and(|store| store.kind == StoreKind::Liability);
                let available =
                    self.forecast_balance(&mut balances, &flow.store_id, &flow.asset.key(), now);
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
                    self.forecast_balance(&mut balances, &leg.store_id, &leg.asset.key(), now);
                balances.insert(
                    (leg.store_id.clone(), leg.asset.key()),
                    current + leg.amount,
                );
            }
            out.push(item);
        }

        // Rent and loan payments by their due day.
        let due_dates = |due_day: u8| {
            let mut dates = Vec::new();
            let mut month = first_of_month(today);
            while month <= last {
                let date = clamp_day(month, due_day);
                if date >= today && date <= last {
                    dates.push(date);
                }
                month = next_month(month);
            }
            dates
        };
        for item in self.doc.utility_items.iter().filter(|item| item.active) {
            if let UtilityKind::Housing {
                amount,
                due_day,
                automation_id: None,
                ..
            } = &item.kind
            {
                for date in due_dates(*due_day) {
                    let secs = cal.at_local(date, AUTOMATION_HOUR);
                    out.push(UpcomingItem {
                        kind: UpcomingKind::Housing,
                        source_id: item.id.clone(),
                        at: calendar::iso(secs),
                        date: date.format("%Y-%m-%d").to_string(),
                        title: item.name.clone(),
                        store_id: item.store_id.clone(),
                        amount: Some(Amount::ZERO - *amount),
                        asset: Some(AssetRef::fiat(
                            &item.currency,
                            &item.currency.to_uppercase(),
                        )),
                        shortfall: None,
                    });
                }
            }
        }
        for store in &self.doc.stores {
            if store.kind != StoreKind::Liability
                || store.archived
                || paid_loans.contains(&store.id)
            {
                continue;
            }
            let Some(terms) = store.liability.as_ref() else {
                continue;
            };
            let (Some(payment), Some(due_day)) = (terms.monthly_payment, terms.due_day) else {
                continue;
            };
            for date in due_dates(due_day) {
                let secs = cal.at_local(date, AUTOMATION_HOUR);
                out.push(UpcomingItem {
                    kind: UpcomingKind::LoanPayment,
                    source_id: store.id.clone(),
                    at: calendar::iso(secs),
                    date: date.format("%Y-%m-%d").to_string(),
                    title: store.name.clone(),
                    store_id: Some(store.id.clone()),
                    amount: Some(Amount::ZERO - payment),
                    asset: None,
                    shortfall: None,
                });
            }
        }

        // Reminders: overdue ones too, and every repeat within the horizon.
        for reminder in self
            .doc
            .reminders
            .iter()
            .filter(|item| !item.done && !item.muted)
        {
            let mut at = calendar::iso(reminder_secs(reminder));
            for _ in 0..MAX_REPEATS_LISTED {
                let secs = secs_or_now(&at);
                if secs >= horizon {
                    break;
                }
                out.push(UpcomingItem {
                    kind: UpcomingKind::Reminder,
                    source_id: reminder.id.clone(),
                    at: at.clone(),
                    date: cal.date_of(secs).format("%Y-%m-%d").to_string(),
                    title: reminder.text.clone(),
                    store_id: None,
                    amount: None,
                    asset: None,
                    shortfall: None,
                });
                let Some(next) = reminder
                    .repeat
                    .and_then(|repeat| next_occurrence(repeat, &at, secs))
                else {
                    break;
                };
                at = next;
            }
        }

        // Debts with a due date and something still open; overdue ones too.
        let balances = self.contact_balances();
        for terms in &self.doc.debt_terms {
            let Some(due) = terms.due_at.as_deref() else {
                continue;
            };
            let Ok(date) = chrono::NaiveDate::parse_from_str(due, "%Y-%m-%d") else {
                continue;
            };
            let open = balances.iter().find(|row| {
                row.contact_id == terms.contact_id
                    && row.asset.symbol.eq_ignore_ascii_case(&terms.asset)
            });
            let Some(row) = open else { continue };
            let net = row.receivable - row.payable;
            if net.is_zero() {
                continue;
            }
            let secs = cal.at_local(date, AUTOMATION_HOUR);
            if secs >= horizon {
                continue;
            }
            let name = self
                .doc
                .contacts
                .iter()
                .find(|contact| contact.id == terms.contact_id)
                .map(|contact| contact.name.clone())
                .unwrap_or_default();
            out.push(UpcomingItem {
                kind: UpcomingKind::Debt,
                source_id: terms.contact_id.clone(),
                at: calendar::iso(secs),
                date: due.to_string(),
                title: name,
                store_id: None,
                amount: Some(net),
                asset: Some(row.asset.clone()),
                shortfall: None,
            });
        }

        // Rejected runs wait for «Повторить / Пропустить».
        for run in &self.doc.automation_runs {
            let AutomationOutcome::Rejected { .. } = &run.outcome else {
                continue;
            };
            let name = self
                .doc
                .automations
                .iter()
                .find(|item| item.id == run.automation_id)
                .map(|item| item.name.clone())
                .unwrap_or_default();
            let secs = secs_or_now(&run.planned_at);
            out.push(UpcomingItem {
                kind: UpcomingKind::RejectedRun,
                source_id: run.id.clone(),
                at: run.planned_at.clone(),
                date: cal.date_of(secs).format("%Y-%m-%d").to_string(),
                title: name,
                store_id: None,
                amount: None,
                asset: None,
                shortfall: None,
            });
        }
        out.sort_by_key(|item| secs_or_now(&item.at));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::planning::{Schedule, TransactionDraft};
    use crate::model::vault::PricePoint;
    use crate::service::AutomationDraft;
    use crate::service::test_support::*;

    fn secs(iso: &str) -> i64 {
        unix_secs(iso).unwrap()
    }

    #[test]
    fn upcoming_flags_a_payment_the_balance_will_not_cover() {
        let (_dir, mut vault) = empty_vault();
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        let mut seed = income(&card, "500", "seed");
        at(&mut seed, "2026-03-01T00:00:00Z");
        vault.add_transaction(seed).unwrap();
        let Transaction::Payment(mut rent) = payment(&card, "800", "rent") else {
            unreachable!()
        };
        rent.meta.categories = vec!["rent_mortgage".into()];
        vault
            .create_automation(AutomationDraft {
                name: "Rent".into(),
                active: true,
                preset: None,
                schedule: Schedule::Monthly { day: 15 },
                starts_at: "2026-01-01T00:00:00Z".into(),
                template: TransactionDraft::Payment(rent),
            })
            .unwrap();
        let Transaction::Topup(salary) = income(&card, "1000", "salary") else {
            unreachable!()
        };
        vault
            .create_automation(AutomationDraft {
                name: "Salary".into(),
                active: true,
                preset: None,
                schedule: Schedule::Monthly { day: 20 },
                starts_at: "2026-01-01T00:00:00Z".into(),
                template: TransactionDraft::Topup(salary),
            })
            .unwrap();
        let items = vault.upcoming(45, secs("2026-03-10T00:00:00Z"), "UTC");
        let titles: Vec<(&str, Option<Amount>)> = items
            .iter()
            .map(|item| (item.title.as_str(), item.shortfall))
            .collect();
        // Mar 15 rent: 500 of 800. Mar 20 salary. Apr 15 rent: 700 of 800.
        assert_eq!(
            titles,
            vec![
                ("Rent", Some(amount("300"))),
                ("Salary", None),
                ("Rent", Some(amount("100"))),
                ("Salary", None),
            ]
        );
    }

    #[test]
    fn an_alert_fires_once() {
        let (_dir, mut vault) = empty_vault();
        vault.doc.price_cache.push(PricePoint {
            asset_key: btc().key(),
            value_usd: amount("100000"),
            ts: "2026-03-01T00:00:00Z".into(),
            source: "test".into(),
            change_24h_pct: None,
        });
        vault
            .create_price_alert(PriceAlertDraft {
                asset: btc(),
                condition: AlertCondition::Above,
                price: amount("95000"),
                currency: "usd".into(),
                active: true,
                store_id: None,
            })
            .unwrap();
        let now = secs("2026-03-02T00:00:00Z");
        assert_eq!(vault.check_alerts(now).unwrap().len(), 1);
        assert!(vault.check_alerts(now).unwrap().is_empty());
        assert!(vault.alert_assets().is_empty());
    }

    #[test]
    fn an_alert_belongs_to_an_exchange_only() {
        let (_dir, mut vault) = empty_vault();
        let exchange = store(&mut vault, "Bybit", StoreKind::Exchange, None);
        let bank = store(&mut vault, "Bank", StoreKind::Bank, None);
        let draft = |store_id: &str| PriceAlertDraft {
            asset: btc(),
            condition: AlertCondition::Above,
            price: amount("95000"),
            currency: "usd".into(),
            active: true,
            store_id: Some(store_id.to_string()),
        };
        let alert = vault.create_price_alert(draft(&exchange)).unwrap();
        assert_eq!(alert.store_id.as_deref(), Some(exchange.as_str()));
        assert!(matches!(
            vault.create_price_alert(draft(&bank)),
            Err(Error::InvalidInput(_))
        ));
        assert!(matches!(
            vault.create_price_alert(draft("nope")),
            Err(Error::Missing(_))
        ));
    }

    #[test]
    fn reminders_come_due_and_can_be_done() {
        let (_dir, mut vault) = empty_vault();
        let reminder = vault
            .create_reminder("Pay the nanny", "2026-03-05T09:00:00Z", None)
            .unwrap();
        assert!(vault.due_reminders(secs("2026-03-04T00:00:00Z")).is_empty());
        assert_eq!(vault.due_reminders(secs("2026-03-05T10:00:00Z")).len(), 1);
        vault.set_reminder_done(&reminder.id, true).unwrap();
        assert!(vault.due_reminders(secs("2026-03-05T10:00:00Z")).is_empty());
    }

    #[test]
    fn a_monthly_reminder_moves_on_when_done_and_keeps_its_day() {
        use crate::model::planning::{ReminderRepeat, RepeatUnit};
        let monthly = ReminderRepeat {
            every: 1,
            unit: RepeatUnit::Month,
        };
        // Long overdue: done jumps past now, not just one month on.
        let next = next_occurrence(
            monthly,
            "2020-01-31T07:30:00Z",
            secs("2020-04-15T00:00:00Z"),
        );
        assert_eq!(next.as_deref(), Some("2020-04-30T07:30:00Z"));

        let (_dir, mut vault) = empty_vault();
        let reminder = vault
            .create_reminder("Rent", "2020-01-31T07:30:00Z", Some(monthly))
            .unwrap();
        let moved = vault.set_reminder_done(&reminder.id, true).unwrap();
        assert!(!moved.done, "a repeating reminder stays open");
        assert!(secs(&moved.due_at) > secs("2026-01-01T00:00:00Z"));
        assert_eq!(&moved.due_at[11..], "07:30:00Z", "the time stays");

        let finished = vault.finish_reminder_series(&reminder.id).unwrap();
        assert!(finished.done && finished.repeat.is_none());
    }

    #[test]
    fn a_snoozed_reminder_waits_until_its_snooze_ends() {
        let (_dir, mut vault) = empty_vault();
        let reminder = vault
            .create_reminder("Call", "2026-03-05T09:00:00Z", None)
            .unwrap();
        vault
            .snooze_reminder(&reminder.id, "2026-03-05T12:00:00Z")
            .unwrap();
        assert!(vault.due_reminders(secs("2026-03-05T10:00:00Z")).is_empty());
        assert_eq!(vault.due_reminders(secs("2026-03-05T12:30:00Z")).len(), 1);
        assert!(vault.snooze_reminder(&reminder.id, "soon").is_err());
    }
}
