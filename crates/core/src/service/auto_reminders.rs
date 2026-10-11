//! Automatic reminders (plan §15): made from what the user already entered —
//! a loan's payment day, the rent, the utilities pay day, a deposit's end —
//! and kept in step with it. A changed payment day moves the reminder, an
//! archived loan removes it, a payment made closes the month by itself.
//!
//! The user's word wins: a muted reminder is never created again, one whose
//! time was edited by hand is not moved, and «done» is never undone, because
//! only a changed schedule (its `source_key`) places a reminder anew.

use std::collections::{HashMap, HashSet};

use chrono::{Datelike, Duration, NaiveDate};

use crate::Amount;
use crate::calendar::{self, Calendar, clamp_day, first_of_month, next_month};
use crate::model::notify::NoticeCategory;
use crate::model::planning::{
    Reminder, ReminderRepeat, ReminderSource, ReminderSourceKind, RepeatUnit, UtilityKind,
};
use crate::model::store::{StoreKind, SubType};
use crate::model::transaction::{LegRole, Transaction};
use crate::pricing::history::secs_or_now;

use super::UnlockedVault;
use super::agenda::next_occurrence;

/// When a source's event happens.
#[derive(Debug, Clone, Copy, PartialEq)]
enum When {
    /// Every month on this day (a short month: its last day).
    Monthly(u8),
    /// Every year on this month and day.
    Yearly(u32, u32),
    /// Once.
    Once(NaiveDate),
}

/// One reminder the data asks for.
#[derive(Debug, Clone)]
struct Spec {
    source: ReminderSource,
    category: NoticeCategory,
    text: String,
    when: When,
}

/// The category a source's switches live under.
pub fn category_of(kind: ReminderSourceKind) -> NoticeCategory {
    use ReminderSourceKind::*;
    match kind {
        LoanPayment | CreditCardPayment => NoticeCategory::Loan,
        Rent => NoticeCategory::Rent,
        Utilities | MeterReadings => NoticeCategory::Utilities,
        DepositEnd | PdsContribution => NoticeCategory::Deposit,
        LoyaltyExpiry => NoticeCategory::Loyalty,
        Debt => NoticeCategory::Debt,
        GoalSaving | GoalDeadline => NoticeCategory::Goal,
        PropertyTax | InterestTax => NoticeCategory::Tax,
    }
}

/// Days the meters are read before the utilities are paid.
const METER_LEAD_DAYS: i64 = 3;

fn ymd(raw: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(raw.get(..10)?, "%Y-%m-%d").ok()
}

fn date_text(date: NaiveDate) -> String {
    date.format("%d.%m.%Y").to_string()
}

impl UnlockedVault {
    /// Everything the data asks to be reminded of, in the user's language.
    fn auto_specs(&self, today: NaiveDate, ru: bool) -> Vec<Spec> {
        use ReminderSourceKind::*;
        let mut out = Vec::new();
        let mut push = |kind: ReminderSourceKind, id: &str, text: String, when: When| {
            out.push(Spec {
                source: ReminderSource {
                    kind,
                    id: id.to_string(),
                },
                category: category_of(kind),
                text,
                when,
            });
        };
        let live = || self.doc.stores.iter().filter(|store| !store.archived);

        // Loans and credit cards: the mandatory payment day.
        for store in live() {
            let Some(terms) = &store.liability else {
                continue;
            };
            let Some(day) = terms.due_day.filter(|day| (1..=31).contains(day)) else {
                continue;
            };
            let card = store.sub_type == Some(SubType::CreditCard);
            if !card && store.kind != StoreKind::Liability {
                continue;
            }
            let amount = terms
                .monthly_payment
                .map(|value| format!(": {value}"))
                .unwrap_or_default();
            let text = match (card, ru) {
                (true, true) => format!("Погасить кредитку «{}» до {day}-го{amount}", store.name),
                (true, false) => {
                    format!("Pay off the card «{}» by the {day}th{amount}", store.name)
                }
                (false, true) => format!("Платёж по кредиту «{}», {day}-го{amount}", store.name),
                (false, false) => format!("Loan payment «{}» on the {day}th{amount}", store.name),
            };
            let kind = if card { CreditCardPayment } else { LoanPayment };
            push(kind, &store.id, text, When::Monthly(day));
        }

        // Rent and the utilities of each property.
        for item in self.doc.utility_items.iter().filter(|item| item.active) {
            if let UtilityKind::Housing {
                amount, due_day, ..
            } = &item.kind
                && (1..=31).contains(due_day)
            {
                let code = item.currency.to_uppercase();
                let text = if ru {
                    format!("Аренда «{}»: {amount} {code}, {due_day}-го", item.name)
                } else {
                    format!("Rent «{}»: {amount} {code} on the {due_day}th", item.name)
                };
                push(Rent, &item.id, text, When::Monthly(*due_day));
            }
        }
        for property in live().filter(|store| store.kind == StoreKind::Property) {
            let Some(day) = property
                .property
                .as_ref()
                .and_then(|info| info.utilities_pay_day)
                .filter(|day| (1..=31).contains(day))
            else {
                continue;
            };
            let items: Vec<_> = self
                .doc
                .utility_items
                .iter()
                .filter(|item| item.active && item.property_id == property.id)
                .collect();
            if items.is_empty() {
                continue;
            }
            let text = if ru {
                format!("Оплатить ЖКХ: {}, {day}-го", property.name)
            } else {
                format!("Pay the utilities of {} on the {day}th", property.name)
            };
            push(Utilities, &property.id, text, When::Monthly(day));
            if items
                .iter()
                .any(|item| matches!(item.kind, UtilityKind::Metered { .. }))
            {
                let read_day = (i64::from(day) - METER_LEAD_DAYS).max(1) as u8;
                let text = if ru {
                    format!("Передать показания счётчиков: {}", property.name)
                } else {
                    format!("Send the meter readings of {}", property.name)
                };
                push(MeterReadings, &property.id, text, When::Monthly(read_day));
            }
        }

        // Deposits: the end of a term deposit; the yearly ПДС contribution.
        for store in live() {
            let Some(terms) = &store.deposit else {
                continue;
            };
            if let (Some(opened), Some(months)) =
                (terms.opened_at.as_deref().and_then(ymd), terms.term_months)
                && let Some(end) = opened.checked_add_months(chrono::Months::new(months))
                && end >= today
            {
                let text = if ru {
                    format!(
                        "Вклад «{}» заканчивается {}: продлить или забрать",
                        store.name,
                        date_text(end)
                    )
                } else {
                    format!(
                        "Deposit «{}» ends on {}: renew or withdraw",
                        store.name,
                        date_text(end)
                    )
                };
                push(DepositEnd, &store.id, text, When::Once(end));
            }
            if terms.pds.is_some() {
                let text = if ru {
                    format!(
                        "Внесите взнос по ПДС «{}» до конца года — для софинансирования",
                        store.name
                    )
                } else {
                    format!(
                        "Pay into «{}» before the year ends for the state co-financing",
                        store.name
                    )
                };
                push(PdsContribution, &store.id, text, When::Yearly(12, 1));
            }
        }

        // Loyalty points that burn.
        for store in live() {
            let Some(info) = &store.loyalty else { continue };
            let Some(date) = info
                .expires_at
                .as_deref()
                .and_then(ymd)
                .filter(|date| *date >= today)
            else {
                continue;
            };
            let points = info
                .expiring_points
                .map(|value| value.to_string())
                .unwrap_or_default();
            let text = if ru {
                format!(
                    "Сгорят {points} баллов «{}» {}",
                    store.name,
                    date_text(date)
                )
            } else {
                format!(
                    "{points} points of «{}» burn on {}",
                    store.name,
                    date_text(date)
                )
            };
            push(LoyaltyExpiry, &store.id, text, When::Once(date));
        }

        // Debts with a due date and something still open.
        let balances = self.contact_balances();
        for terms in &self.doc.debt_terms {
            let Some(date) = terms.due_at.as_deref().and_then(ymd) else {
                continue;
            };
            let net = balances
                .iter()
                .filter(|row| {
                    row.contact_id == terms.contact_id
                        && row.asset.symbol.eq_ignore_ascii_case(&terms.asset)
                })
                .fold(Amount::ZERO, |acc, row| acc + row.receivable - row.payable);
            if net.is_zero() {
                continue;
            }
            let Some(contact) = self
                .doc
                .contacts
                .iter()
                .find(|contact| contact.id == terms.contact_id)
            else {
                continue;
            };
            let asset = terms.asset.to_uppercase();
            let abs = net.abs();
            let text = match (net.is_positive(), ru) {
                (true, true) => format!(
                    "{} должен вернуть {abs} {asset} до {}",
                    contact.name,
                    date_text(date)
                ),
                (true, false) => format!(
                    "{} owes you {abs} {asset} by {}",
                    contact.name,
                    date_text(date)
                ),
                (false, true) => format!(
                    "Вернуть {} {abs} {asset} до {}",
                    contact.name,
                    date_text(date)
                ),
                (false, false) => format!(
                    "Pay back {} {abs} {asset} by {}",
                    contact.name,
                    date_text(date)
                ),
            };
            push(Debt, &terms.id, text, When::Once(date));
        }

        // Goals: the monthly saving and the deadline.
        for goal in self.doc.goals.iter().filter(|goal| goal.active) {
            if let Some(amount) = goal.monthly_commitment.filter(|value| value.is_positive()) {
                let day = ymd(&goal.starts_at).map_or(1, |date| date.day() as u8);
                let code = goal.currency.to_uppercase();
                let text = if ru {
                    format!("Отложить {amount} {code} на «{}»", goal.name)
                } else {
                    format!("Put {amount} {code} aside for «{}»", goal.name)
                };
                push(GoalSaving, &goal.id, text, When::Monthly(day));
            }
            if let Some(date) = goal
                .deadline
                .as_deref()
                .and_then(ymd)
                .filter(|date| *date >= today)
            {
                let text = if ru {
                    format!("Дедлайн цели «{}» — {}", goal.name, date_text(date))
                } else {
                    format!("The deadline of «{}» is {}", goal.name, date_text(date))
                };
                push(GoalDeadline, &goal.id, text, When::Once(date));
            }
        }

        // Taxes, when there is a reason: property, interest on deposits.
        if live().any(|store| store.kind == StoreKind::Property && store.parent_id.is_none()) {
            let text = if ru {
                "Налог на имущество — заплатить до 1 декабря".to_string()
            } else {
                "Property tax is due by December 1".to_string()
            };
            push(PropertyTax, "property-tax", text, When::Yearly(12, 1));
        }
        if live().any(|store| store.deposit.is_some()) {
            let text = if ru {
                "НДФЛ с процентов по вкладам — заплатить до 1 декабря".to_string()
            } else {
                "Tax on deposit interest is due by December 1".to_string()
            };
            push(InterestTax, "interest-tax", text, When::Yearly(12, 1));
        }
        out
    }

    /// Create, move and remove the automatic reminders so they match the data.
    /// Returns whether anything changed (and was written).
    pub fn sync_auto_reminders(&mut self, now: i64, zone: &str, ru: bool) -> crate::Result<bool> {
        let cal = Calendar::new(zone);
        let today = cal.date_of(now);
        let settings = self.doc.settings.notifications.clone();
        let (hour, minute) = settings
            .default_time
            .split_once(':')
            .and_then(|(hour, minute)| {
                Some((hour.parse::<u32>().ok()?, minute.parse::<u32>().ok()?))
            })
            .filter(|(hour, minute)| *hour < 24 && *minute < 60)
            .unwrap_or((10, 0));

        let specs: Vec<Spec> = self
            .auto_specs(today, ru)
            .into_iter()
            .filter(|spec| settings.category(spec.category).auto_reminders)
            .collect();
        let wanted: HashSet<ReminderSource> =
            specs.iter().map(|spec| spec.source.clone()).collect();
        let mut changed = false;

        // Gone or switched off: removed, unless muted on purpose.
        let before = self.doc.reminders.len();
        self.doc
            .reminders
            .retain(|reminder| match &reminder.source {
                Some(source) => wanted.contains(source) || reminder.muted,
                None => true,
            });
        changed |= self.doc.reminders.len() != before;

        let index: HashMap<ReminderSource, usize> = self
            .doc
            .reminders
            .iter()
            .enumerate()
            .filter_map(|(at, reminder)| reminder.source.clone().map(|source| (source, at)))
            .collect();

        for spec in specs {
            let lead = i64::from(settings.category(spec.category).lead_days);
            let key = format!(
                "{}|{lead}|{hour:02}:{minute:02}",
                match spec.when {
                    When::Monthly(day) => format!("m{day}"),
                    When::Yearly(month, day) => format!("y{month:02}-{day:02}"),
                    When::Once(date) => format!("d{date}"),
                }
            );
            let place = || -> Option<(String, Option<ReminderRepeat>)> {
                let (event, repeat) = match spec.when {
                    When::Monthly(day) => {
                        let this = clamp_day(today, day);
                        let event = if this - Duration::days(lead) >= today {
                            this
                        } else {
                            clamp_day(next_month(first_of_month(today)), day)
                        };
                        (
                            event,
                            Some(ReminderRepeat {
                                every: 1,
                                unit: RepeatUnit::Month,
                            }),
                        )
                    }
                    When::Yearly(month, day) => {
                        let this = NaiveDate::from_ymd_opt(today.year(), month, day)?;
                        let event = if this - Duration::days(lead) >= today {
                            this
                        } else {
                            NaiveDate::from_ymd_opt(today.year() + 1, month, day)?
                        };
                        (
                            event,
                            Some(ReminderRepeat {
                                every: 1,
                                unit: RepeatUnit::Year,
                            }),
                        )
                    }
                    When::Once(date) => (date, None),
                };
                let due = (event - Duration::days(lead)).max(today.min(event));
                Some((calendar::iso(cal.at_local_hm(due, hour, minute)), repeat))
            };
            match index.get(&spec.source).copied() {
                Some(at) => {
                    let reminder = &mut self.doc.reminders[at];
                    if reminder.muted {
                        continue;
                    }
                    if reminder.text != spec.text {
                        reminder.text = spec.text.clone();
                        changed = true;
                    }
                    if !reminder.user_edited && reminder.source_key != key {
                        if let Some((due_at, repeat)) = place() {
                            reminder.due_at = due_at;
                            reminder.repeat = repeat;
                            reminder.done = false;
                            reminder.snoozed_until = None;
                        }
                        reminder.source_key = key;
                        changed = true;
                    }
                }
                None => {
                    let Some((due_at, repeat)) = place() else {
                        continue;
                    };
                    let mut reminder = Reminder::new(&spec.text, &due_at);
                    reminder.repeat = repeat;
                    reminder.source = Some(spec.source.clone());
                    reminder.source_key = key;
                    self.doc.reminders.push(reminder);
                    changed = true;
                }
            }
        }
        changed |= self.close_paid_reminders(now, &cal);
        if changed {
            self.persist()?;
        }
        Ok(changed)
    }

    /// A monthly reminder whose payment is already booked for its month moves
    /// on to the next month by itself.
    fn close_paid_reminders(&mut self, now: i64, cal: &Calendar) -> bool {
        use ReminderSourceKind::*;
        let lead_of = |vault: &Self, kind| {
            i64::from(
                vault
                    .doc
                    .settings
                    .notifications
                    .category(category_of(kind))
                    .lead_days,
            )
        };
        let mut moved = Vec::new();
        for (at, reminder) in self.doc.reminders.iter().enumerate() {
            let (Some(source), Some(repeat)) = (&reminder.source, reminder.repeat) else {
                continue;
            };
            if reminder.done || reminder.muted || secs_or_now(&reminder.due_at) > now + 86_400 * 31
            {
                continue;
            }
            let lead = if source.kind == MeterReadings {
                0
            } else {
                lead_of(self, source.kind)
            };
            let event = cal.date_of(secs_or_now(&reminder.due_at)) + Duration::days(lead);
            let (start, end) = cal.month_bounds(event);
            if self.month_is_paid(source, start, end)
                && let Some(next) =
                    next_occurrence(repeat, &reminder.due_at, secs_or_now(&reminder.due_at))
            {
                moved.push((at, next));
            }
        }
        let changed = !moved.is_empty();
        for (at, next) in moved {
            self.doc.reminders[at].due_at = next;
            self.doc.reminders[at].snoozed_until = None;
        }
        changed
    }

    /// Whether what a monthly reminder asks for is already done in `[start, end)`.
    fn month_is_paid(&self, source: &ReminderSource, start: i64, end: i64) -> bool {
        use ReminderSourceKind::*;
        let in_month = |tx: &Transaction| {
            let secs = secs_or_now(&tx.meta().timestamp);
            secs >= start && secs < end
        };
        let property_items = |property: &str| -> Vec<String> {
            self.doc
                .utility_items
                .iter()
                .filter(|item| item.property_id == property)
                .map(|item| item.id.clone())
                .collect()
        };
        match source.kind {
            // Money moved onto the loan or the card.
            LoanPayment | CreditCardPayment => self.doc.transactions.iter().any(|tx| {
                in_month(tx)
                    && tx.legs().iter().any(|leg| {
                        leg.store_id == source.id
                            && leg.role == LegRole::Principal
                            && leg.amount.is_positive()
                    })
            }),
            Rent => self.doc.transactions.iter().any(|tx| {
                in_month(tx) && tx.meta().utility_item_id.as_deref() == Some(source.id.as_str())
            }),
            Utilities => {
                let items = property_items(&source.id);
                self.doc.transactions.iter().any(|tx| {
                    in_month(tx)
                        && tx
                            .meta()
                            .utility_item_id
                            .as_ref()
                            .is_some_and(|id| items.contains(id))
                })
            }
            MeterReadings => {
                let items = property_items(&source.id);
                let month = calendar::month_key(cal_date(start));
                self.doc
                    .meter_readings
                    .iter()
                    .any(|reading| items.contains(&reading.item_id) && reading.period == month)
            }
            _ => false,
        }
    }
}

/// The UTC date of a moment, for a month key (the bounds start at local midnight).
fn cal_date(secs: i64) -> NaiveDate {
    chrono::DateTime::from_timestamp(secs + 12 * 3600, 0)
        .map(|moment| moment.date_naive())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use super::*;
    use crate::model::store::{LiabilityKind, LiabilityTerms, PropertyInfo};
    use crate::model::transaction::AccountRef;
    use crate::service::{StoreExtras, UtilityDraft};

    fn loan(vault: &mut UnlockedVault, due_day: u8) -> String {
        vault
            .create_store_with(
                "Ипотека",
                StoreKind::Liability,
                "#c00",
                "",
                None,
                StoreExtras {
                    liability: Some(LiabilityTerms {
                        kind: LiabilityKind::Mortgage,
                        rate: None,
                        limit: None,
                        due_day: Some(due_day),
                        monthly_payment: Some(amount("45000")),
                    }),
                    ..StoreExtras::default()
                },
            )
            .unwrap()
            .id
    }

    fn auto(vault: &UnlockedVault, kind: ReminderSourceKind) -> Vec<Reminder> {
        vault
            .doc
            .reminders
            .iter()
            .filter(|reminder| {
                reminder
                    .source
                    .as_ref()
                    .is_some_and(|source| source.kind == kind)
            })
            .cloned()
            .collect()
    }

    #[test]
    fn a_loan_gets_a_monthly_reminder_ahead_of_its_payment_day() {
        let (_dir, mut vault) = empty_vault();
        let id = loan(&mut vault, 25);
        let now = secs_or_now("2026-03-10T08:00:00Z");
        assert!(vault.sync_auto_reminders(now, "UTC", true).unwrap());
        let reminders = auto(&vault, ReminderSourceKind::LoanPayment);
        assert_eq!(reminders.len(), 1);
        // One day ahead (the loan category's default), at 10:00.
        assert_eq!(reminders[0].due_at, "2026-03-24T10:00:00Z");
        assert_eq!(
            reminders[0].repeat.map(|repeat| repeat.unit),
            Some(RepeatUnit::Month)
        );
        assert!(reminders[0].text.contains("Ипотека") && reminders[0].text.contains("45000"));
        assert_eq!(reminders[0].source.as_ref().unwrap().id, id);

        // Nothing to do the second time.
        assert!(!vault.sync_auto_reminders(now, "UTC", true).unwrap());
    }

    #[test]
    fn a_changed_payment_day_moves_it_but_done_is_not_undone() {
        let (_dir, mut vault) = empty_vault();
        let id = loan(&mut vault, 25);
        let now = secs_or_now("2026-03-10T08:00:00Z");
        vault.sync_auto_reminders(now, "UTC", true).unwrap();
        let reminder = auto(&vault, ReminderSourceKind::LoanPayment)[0].clone();

        // Done: it moves to April, and the next sync keeps it there.
        vault.set_reminder_done(&reminder.id, true).unwrap();
        vault.sync_auto_reminders(now, "UTC", true).unwrap();
        let after = auto(&vault, ReminderSourceKind::LoanPayment)[0].clone();
        assert_ne!(after.due_at, "2026-03-24T10:00:00Z");

        // A new payment day: placed again.
        let store = vault
            .doc
            .stores
            .iter_mut()
            .find(|store| store.id == id)
            .unwrap();
        store.liability.as_mut().unwrap().due_day = Some(15);
        vault.sync_auto_reminders(now, "UTC", true).unwrap();
        assert_eq!(
            auto(&vault, ReminderSourceKind::LoanPayment)[0].due_at,
            "2026-03-14T10:00:00Z"
        );
    }

    #[test]
    fn an_archived_loan_loses_its_reminder_and_a_muted_one_never_returns() {
        let (_dir, mut vault) = empty_vault();
        let id = loan(&mut vault, 25);
        let now = secs_or_now("2026-03-10T08:00:00Z");
        vault.sync_auto_reminders(now, "UTC", false).unwrap();
        let reminder = auto(&vault, ReminderSourceKind::LoanPayment)[0].clone();
        vault.finish_reminder_series(&reminder.id).unwrap();
        vault.sync_auto_reminders(now, "UTC", false).unwrap();
        let kept = auto(&vault, ReminderSourceKind::LoanPayment);
        assert_eq!(kept.len(), 1);
        assert!(
            kept[0].muted && kept[0].done,
            "a finished one stays finished"
        );

        vault.archive_store(&id, true).unwrap();
        vault.sync_auto_reminders(now, "UTC", false).unwrap();
        assert!(auto(&vault, ReminderSourceKind::LoanPayment).len() <= 1);
        // Unmuted ones go with their source.
        vault
            .doc
            .reminders
            .iter_mut()
            .for_each(|reminder| reminder.muted = false);
        vault.sync_auto_reminders(now, "UTC", false).unwrap();
        assert!(auto(&vault, ReminderSourceKind::LoanPayment).is_empty());
    }

    #[test]
    fn a_payment_made_closes_the_month() {
        let (_dir, mut vault) = empty_vault();
        let id = loan(&mut vault, 25);
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        let mut salary = income(&card, "1000", "salary");
        at(&mut salary, "2026-03-01T10:00:00Z");
        vault.add_transaction(salary).unwrap();
        let now = secs_or_now("2026-03-20T08:00:00Z");
        vault.sync_auto_reminders(now, "UTC", true).unwrap();
        assert_eq!(
            auto(&vault, ReminderSourceKind::LoanPayment)[0].due_at,
            "2026-03-24T10:00:00Z"
        );

        let mut paid =
            transfer_between(AccountRef::Store(card), AccountRef::Store(id), usd(), "450");
        at(&mut paid, "2026-03-20T09:00:00Z");
        vault.add_transaction(paid).unwrap();
        vault.sync_auto_reminders(now, "UTC", true).unwrap();
        assert_eq!(
            auto(&vault, ReminderSourceKind::LoanPayment)[0].due_at,
            "2026-04-24T10:00:00Z"
        );
    }

    #[test]
    fn a_switched_off_category_makes_no_reminders_and_taxes_need_a_reason() {
        let (_dir, mut vault) = empty_vault();
        loan(&mut vault, 25);
        let now = secs_or_now("2026-03-10T08:00:00Z");
        vault.sync_auto_reminders(now, "UTC", true).unwrap();
        assert!(
            auto(&vault, ReminderSourceKind::PropertyTax).is_empty(),
            "no property, no tax"
        );

        vault
            .doc
            .settings
            .notifications
            .categories
            .push(crate::model::notify::NoticeCategoryCfg {
                category: NoticeCategory::Loan,
                enabled: true,
                lead_days: 1,
                auto_reminders: false,
            });
        vault.sync_auto_reminders(now, "UTC", true).unwrap();
        assert!(auto(&vault, ReminderSourceKind::LoanPayment).is_empty());

        let flat = vault
            .create_store_with(
                "Квартира",
                StoreKind::Property,
                "#555",
                "",
                None,
                StoreExtras {
                    property: Some(PropertyInfo {
                        utilities_pay_day: Some(10),
                        ..PropertyInfo::default()
                    }),
                    ..StoreExtras::default()
                },
            )
            .unwrap();
        vault
            .create_utility_item(UtilityDraft {
                name: "Свет".into(),
                active: true,
                kind: UtilityKind::Metered {
                    unit: "kWh".into(),
                    tariff: amount("5"),
                },
                currency: "rub".into(),
                property_id: flat.id.clone(),
                store_id: None,
                category: "utilities".into(),
            })
            .unwrap();
        vault.sync_auto_reminders(now, "UTC", true).unwrap();
        assert_eq!(auto(&vault, ReminderSourceKind::PropertyTax).len(), 1);
        assert_eq!(auto(&vault, ReminderSourceKind::Utilities).len(), 1);
        assert_eq!(auto(&vault, ReminderSourceKind::MeterReadings).len(), 1);
    }
}
