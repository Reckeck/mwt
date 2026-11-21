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
use crate::model::contact::DebtTerms;
use crate::model::notify::NoticeCategory;
use crate::model::planning::{
    Reminder, ReminderRepeat, ReminderSource, ReminderSourceKind, RepeatUnit, UtilityKind,
};
use crate::model::store::{PropertyCategory, StoreKind, SubType};
use crate::model::transaction::{LegRole, Transaction};
use crate::time::secs_or_now;

use crate::model::vault::VaultDoc;
use crate::planning::agenda::next_occurrence;

/// When a source's event happens.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum When {
    /// Every month on this day (a short month: its last day).
    Monthly(u8),
    /// Every year on this month and day.
    Yearly(u32, u32),
    /// Once.
    Once(NaiveDate),
}

/// One reminder the data asks for.
#[derive(Debug, Clone)]
pub(crate) struct Spec {
    pub(crate) source: ReminderSource,
    pub(crate) category: NoticeCategory,
    pub(crate) text: String,
    pub(crate) when: When,
}

/// The text of the automatic reminders in the reader's language and in the
/// other one, by source. The stored text is in the language of the device that
/// wrote it, so a text equal to either rendering is still the generated one.
pub(crate) struct Renderings {
    mine: HashMap<ReminderSource, String>,
    other: HashMap<ReminderSource, String>,
}

impl Renderings {
    /// The reader's rendering, when the stored text is one of the two.
    pub(crate) fn shown(&self, reminder: &Reminder) -> Option<&str> {
        let source = reminder.source.as_ref()?;
        let mine = self.mine.get(source)?;
        let generated = *mine == reminder.text || self.other.get(source) == Some(&reminder.text);
        generated.then_some(mine.as_str())
    }

    /// Whether the stored text is the other language's rendering of the same data.
    pub(crate) fn is_other(&self, source: &ReminderSource, text: &str) -> bool {
        self.other.get(source).is_some_and(|other| other == text)
    }
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
        VehicleInsurance | VehicleService => NoticeCategory::Vehicle,
    }
}

/// The part of a source that never changes with its schedule: its kind and
/// the id of the entity (loan, property, deposit...) behind it.
pub(crate) fn source_identity(source: &ReminderSource) -> String {
    let kind = serde_json::to_value(source.kind)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_default();
    format!("{kind}/{}", source.id)
}

/// The id an automatic reminder gets: the same on every device, so two
/// devices that place it meet in one record. Reminders made before this had
/// random ids; sync dedupe matches them by [`source_identity`].
pub(crate) fn auto_reminder_id(source: &ReminderSource) -> String {
    format!("auto-{}", source_identity(source).replace('/', "-"))
}

/// What a reminder's `source_key` is for a source and a schedule: the source
/// identity in front, so two loans due at the same time never share a key.
/// Before it, the key was the schedule alone (it has no `/`).
pub(crate) fn keyed(source: &ReminderSource, schedule: &str) -> String {
    format!("{}/{schedule}", source_identity(source))
}

/// Days the meters are read before the utilities are paid.
pub(crate) const METER_LEAD_DAYS: i64 = 3;

pub(crate) fn ymd(raw: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(raw.get(..10)?, "%Y-%m-%d").ok()
}

pub(crate) fn date_text(date: NaiveDate) -> String {
    date.format("%d.%m.%Y").to_string()
}

/// Everything the data asks to be reminded of, in the user's language.
pub(crate) fn auto_specs(
    doc: &VaultDoc,
    open_debt: impl Fn(&DebtTerms) -> Amount,
    today: NaiveDate,
    ru: bool,
) -> Vec<Spec> {
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
    let live = || doc.stores.iter().filter(|store| !store.archived);

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
    for item in doc.utility_items.iter().filter(|item| item.active) {
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
        let items: Vec<_> = doc
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
    for terms in &doc.debt_terms {
        let Some(date) = terms.due_at.as_deref().and_then(ymd) else {
            continue;
        };
        let net = open_debt(terms);
        if net.is_zero() {
            continue;
        }
        let Some(contact) = doc
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
    for goal in doc.goals.iter().filter(|goal| goal.active) {
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

    // Cars: the insurance ends, the service is due by date or by mileage.
    for store in
        live().filter(|store| store.kind == StoreKind::Property && store.parent_id.is_none())
    {
        let Some(car) = store
            .property
            .as_ref()
            .filter(|info| info.category == PropertyCategory::Car)
            .and_then(|info| info.car.as_ref())
        else {
            continue;
        };
        for (policy, until) in [("osago", &car.osago_until), ("kasko", &car.kasko_until)] {
            let Some(date) = until.as_deref().and_then(ymd).filter(|date| *date >= today) else {
                continue;
            };
            let name = match (policy, ru) {
                ("osago", true) => "ОСАГО",
                ("osago", false) => "Compulsory insurance",
                (_, true) => "КАСКО",
                (_, false) => "Comprehensive insurance",
            };
            let text = if ru {
                format!("{name} «{}» заканчивается {}", store.name, date_text(date))
            } else {
                format!("{name} of «{}» ends on {}", store.name, date_text(date))
            };
            push(
                VehicleInsurance,
                &format!("{}:{policy}", store.id),
                text,
                When::Once(date),
            );
        }
        // Whichever comes first: the date, or the mileage close to the service.
        let by_date = car.service_due_at.as_deref().and_then(ymd);
        // Pinned to the day the mileage was read, so the reminder does not move daily.
        let read_on = car.mileage_at.as_deref().and_then(ymd).unwrap_or(today);
        let due = if car.service_due_by_km() {
            Some(by_date.map_or(read_on, |date| date.min(read_on)))
        } else {
            by_date
        };
        if let Some(date) = due {
            let text = if ru {
                format!("ТО «{}» — {}", store.name, date_text(date))
            } else {
                format!("Service of «{}» is due on {}", store.name, date_text(date))
            };
            push(VehicleService, &store.id, text, When::Once(date));
        }
    }

    // Taxes, when there is a reason: each object by its type, interest on deposits.
    for store in
        live().filter(|store| store.kind == StoreKind::Property && store.parent_id.is_none())
    {
        let category = store
            .property
            .as_ref()
            .map(|info| info.category)
            .unwrap_or_default();
        let text = match (category, ru) {
            (PropertyCategory::Car, true) => {
                format!("Транспортный налог: {} — до 1 декабря", store.name)
            }
            (PropertyCategory::Car, false) => {
                format!("Vehicle tax for {} is due by December 1", store.name)
            }
            (PropertyCategory::Land, true) => {
                format!("Земельный налог: {} — до 1 декабря", store.name)
            }
            (PropertyCategory::Land, false) => {
                format!("Land tax for {} is due by December 1", store.name)
            }
            (PropertyCategory::Other, _) => continue,
            (_, true) => format!("Налог на имущество: {} — до 1 декабря", store.name),
            (_, false) => format!("Property tax for {} is due by December 1", store.name),
        };
        push(PropertyTax, &store.id, text, When::Yearly(12, 1));
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

/// Both renderings of every automatic reminder's text, by source.
pub(crate) fn auto_renderings(
    doc: &VaultDoc,
    open_debt: impl Fn(&DebtTerms) -> Amount,
    today: NaiveDate,
    ru: bool,
) -> Renderings {
    let by_source = |ru: bool| -> HashMap<ReminderSource, String> {
        auto_specs(doc, &open_debt, today, ru)
            .into_iter()
            .map(|spec| (spec.source, spec.text))
            .collect()
    };
    Renderings {
        mine: by_source(ru),
        other: by_source(!ru),
    }
}

/// The reminders as a reader in `ru` or English sees them. The stored
/// text of an automatic reminder is in the language of whichever device
/// wrote it; when it is one of the two renderings of its source it is
/// shown in the reader's language, otherwise (the user typed it, or the
/// data moved on) as it is. Nothing is written.
pub(crate) fn localized(mut reminders: Vec<Reminder>, renderings: &Renderings) -> Vec<Reminder> {
    for reminder in &mut reminders {
        if let Some(text) = renderings.shown(reminder) {
            reminder.text = text.to_string();
        }
    }
    reminders
}

/// The reminders as they must be after a sync at `now`, so they match the
/// data: the automatic ones created, moved and removed. `None` when nothing
/// changes.
pub(crate) fn synced_reminders(
    doc: &VaultDoc,
    open_debt: impl Fn(&DebtTerms) -> Amount,
    now: i64,
    zone: &str,
    ru: bool,
) -> Option<Vec<Reminder>> {
    let cal = Calendar::new(zone);
    let today = cal.date_of(now);
    let settings = doc.settings.notifications.clone();
    let (hour, minute) = settings
        .default_time
        .split_once(':')
        .and_then(|(hour, minute)| Some((hour.parse::<u32>().ok()?, minute.parse::<u32>().ok()?)))
        .filter(|(hour, minute)| *hour < 24 && *minute < 60)
        .unwrap_or((10, 0));

    let renderings = auto_renderings(doc, &open_debt, today, ru);
    let specs: Vec<Spec> = auto_specs(doc, &open_debt, today, ru)
        .into_iter()
        .filter(|spec| settings.category(spec.category).auto_reminders)
        .collect();
    let wanted: HashSet<ReminderSource> = specs.iter().map(|spec| spec.source.clone()).collect();
    let mut changed = false;

    // Gone or switched off: removed, unless muted on purpose.
    let mut reminders = doc.reminders.clone();
    let before = reminders.len();
    reminders.retain(|reminder| match &reminder.source {
        Some(source) => wanted.contains(source) || reminder.muted,
        None => true,
    });
    changed |= reminders.len() != before;

    let index: HashMap<ReminderSource, usize> = reminders
        .iter()
        .enumerate()
        .filter_map(|(at, reminder)| reminder.source.clone().map(|source| (source, at)))
        .collect();

    for spec in specs {
        let lead = i64::from(settings.category(spec.category).lead_days);
        let schedule = format!(
            "{}|{lead}|{hour:02}:{minute:02}",
            match spec.when {
                When::Monthly(day) => format!("m{day}"),
                When::Yearly(month, day) => format!("y{month:02}-{day:02}"),
                When::Once(date) => format!("d{date}"),
            }
        );
        let key = keyed(&spec.source, &schedule);
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
                let reminder = &mut reminders[at];
                if reminder.muted {
                    continue;
                }
                // The other language's text of the same data is not stale:
                // rewriting it would make two devices trade it forever.
                if reminder.text != spec.text && !renderings.is_other(&spec.source, &reminder.text)
                {
                    reminder.text = spec.text.clone();
                    changed = true;
                }
                // A key of the old form (the schedule alone) that still
                // names this schedule is the same placement: only its
                // form is brought up to date, «done» and the rest stay.
                if reminder.source_key == schedule {
                    reminder.source_key = key.clone();
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
                reminder.id = auto_reminder_id(&spec.source);
                reminder.repeat = repeat;
                reminder.source = Some(spec.source.clone());
                reminder.source_key = key;
                reminders.push(reminder);
                changed = true;
            }
        }
    }
    changed |= close_paid_reminders(&mut reminders, doc, now, &cal);
    changed.then_some(reminders)
}

/// A monthly reminder whose payment is already booked for its month moves
/// on to the next month by itself.
fn close_paid_reminders(
    reminders: &mut [Reminder],
    doc: &VaultDoc,
    now: i64,
    cal: &Calendar,
) -> bool {
    use ReminderSourceKind::*;
    let lead_of = |kind| {
        i64::from(
            doc.settings
                .notifications
                .category(category_of(kind))
                .lead_days,
        )
    };
    let mut moved = Vec::new();
    for (at, reminder) in reminders.iter().enumerate() {
        let (Some(source), Some(repeat)) = (&reminder.source, reminder.repeat) else {
            continue;
        };
        if reminder.done || reminder.muted || secs_or_now(&reminder.due_at) > now + 86_400 * 31 {
            continue;
        }
        let lead = if source.kind == MeterReadings {
            0
        } else {
            lead_of(source.kind)
        };
        let event = cal.date_of(secs_or_now(&reminder.due_at)) + Duration::days(lead);
        let (start, end) = cal.month_bounds(event);
        if month_is_paid(doc, source, start, end)
            && let Some(next) =
                next_occurrence(repeat, &reminder.due_at, secs_or_now(&reminder.due_at))
        {
            moved.push((at, next));
        }
    }
    let changed = !moved.is_empty();
    for (at, next) in moved {
        reminders[at].due_at = next;
        reminders[at].snoozed_until = None;
    }
    changed
}

/// Whether what a monthly reminder asks for is already done in `[start, end)`.
fn month_is_paid(doc: &VaultDoc, source: &ReminderSource, start: i64, end: i64) -> bool {
    use ReminderSourceKind::*;
    let in_month = |tx: &Transaction| {
        let secs = secs_or_now(&tx.meta().timestamp);
        secs >= start && secs < end
    };
    let property_items = |property: &str| -> Vec<String> {
        doc.utility_items
            .iter()
            .filter(|item| item.property_id == property)
            .map(|item| item.id.clone())
            .collect()
    };
    match source.kind {
        // Money moved onto the loan or the card.
        LoanPayment | CreditCardPayment => doc.transactions.iter().any(|tx| {
            in_month(tx)
                && tx.legs().iter().any(|leg| {
                    leg.store_id == source.id
                        && leg.role == LegRole::Principal
                        && leg.amount.is_positive()
                })
        }),
        Rent => doc.transactions.iter().any(|tx| {
            in_month(tx) && tx.meta().utility_item_id.as_deref() == Some(source.id.as_str())
        }),
        Utilities => {
            let items = property_items(&source.id);
            doc.transactions.iter().any(|tx| {
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
            doc.meter_readings
                .iter()
                .any(|reading| items.contains(&reading.item_id) && reading.period == month)
        }
        _ => false,
    }
}

/// The UTC date of a moment, for a month key (the bounds start at local midnight).
fn cal_date(secs: i64) -> NaiveDate {
    chrono::DateTime::from_timestamp(secs + 12 * 3600, 0)
        .map(|moment| moment.date_naive())
        .unwrap_or_default()
}
