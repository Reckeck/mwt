//! "Share the settlement": the text to send to the chat. Purchases, shares,
//! who pays whom through the organizer, and the organizer's requisites when
//! they are a contact (or a guest who left some).

use chrono::{Datelike, NaiveDate};
use rust_decimal::Decimal;

use mwt_base::Amount;
use mwt_base::error::Result;
use mwt_model::ContactAccount;
use mwt_model::VaultDoc;
use mwt_model::{ME, SharedEvent};

use super::find_event;
use super::split::{self, Split};

const NBSP: char = '\u{a0}';

const MONTHS_RU: [&str; 12] = [
    "янв", "фев", "мар", "апр", "мая", "июн", "июл", "авг", "сен", "окт", "ноя", "дек",
];
const MONTHS_EN: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// What the text says, in one language.
pub struct Words {
    months: [&'static str; 12],
    me_subject: &'static str,
    me_object: &'static str,
    purchases: &'static str,
    spent: &'static str,
    shares: &'static str,
    who_pays: &'static str,
    settled: &'static str,
    nothing_left: &'static str,
    requisites: &'static str,
    phone: &'static str,
}

pub fn words(english: bool) -> Words {
    if english {
        Words {
            months: MONTHS_EN,
            me_subject: "Me",
            me_object: "me",
            purchases: "Purchases",
            spent: "Spent",
            shares: "Shares",
            who_pays: "Who pays whom",
            settled: "All settled up",
            nothing_left: "Nothing to pay yet",
            requisites: "Pay to",
            phone: "phone",
        }
    } else {
        Words {
            months: MONTHS_RU,
            me_subject: "Я",
            me_object: "мне",
            purchases: "Покупки",
            spent: "Потрачено",
            shares: "Доли",
            who_pays: "Кто кому",
            settled: "Все рассчитались",
            nothing_left: "Платить пока нечего",
            requisites: "Реквизиты",
            phone: "тел.",
        }
    }
}

/// Russian "5 people", "2 people", "1 person" (the form after "to"), `5 people`.
pub fn people_text(count: usize, english: bool) -> String {
    if english {
        return format!("{count} {}", if count == 1 { "person" } else { "people" });
    }
    let last = count % 10;
    let tens = count % 100;
    // Russian "for 1 person", "for 2 people", "for 5 people"
    let word =
        if (last == 1 && tens != 11) || ((2..=4).contains(&last) && !(12..=14).contains(&tens)) {
            "человека"
        } else {
            "человек"
        };
    format!("{count} {word}")
}

/// `7 100` / `1 735,5`: groups of three, a decimal comma in Russian.
pub fn money(value: Amount, currency: &str, english: bool) -> String {
    let rounded = split::round_places(value, 2).0.normalize();
    let text = rounded.abs().to_string();
    let (whole, fraction) = text.split_once('.').unwrap_or((text.as_str(), ""));
    let mut grouped = String::new();
    for (index, digit) in whole.chars().enumerate() {
        if index > 0 && (whole.len() - index) % 3 == 0 {
            grouped.push(NBSP);
        }
        grouped.push(digit);
    }
    if !fraction.is_empty() {
        grouped.push(if english { '.' } else { ',' });
        grouped.push_str(fraction);
    }
    let sign = if rounded < Decimal::ZERO { "-" } else { "" };
    let symbol = match currency.to_lowercase().as_str() {
        "rub" => "₽".to_string(),
        "usd" => "$".to_string(),
        "eur" => "€".to_string(),
        other => other.to_uppercase(),
    };
    format!("{sign}{grouped}{NBSP}{symbol}")
}

fn day_text(day: NaiveDate, words: &Words, with_year: bool) -> String {
    let month = words.months[day.month0() as usize];
    if with_year {
        format!("{} {month} {}", day.day(), day.year())
    } else {
        format!("{} {month}", day.day())
    }
}

/// Russian "2 Nov", "2-4 Nov", "30 Oct - 2 Nov"; another year than `year` adds it.
pub fn range_text(event: &SharedEvent, words: &Words, year: i32) -> Option<String> {
    let parse = |raw: &str| NaiveDate::parse_from_str(raw, "%Y-%m-%d").ok();
    let start = event.starts_on.as_deref().and_then(parse)?;
    let end = event.ends_on.as_deref().and_then(parse);
    let with_year = start.year() != year || end.is_some_and(|last| last.year() != year);
    Some(match end {
        None => day_text(start, words, with_year),
        Some(last) if last.month() == start.month() && last.year() == start.year() => {
            let month = words.months[start.month0() as usize];
            if with_year {
                format!("{}–{} {month} {}", start.day(), last.day(), start.year())
            } else {
                format!("{}–{} {month}", start.day(), last.day())
            }
        }
        Some(last) => format!(
            "{} – {}",
            day_text(start, words, with_year),
            day_text(last, words, with_year)
        ),
    })
}

fn account_text(account: &ContactAccount) -> String {
    if account.label.trim().is_empty() {
        account.value.clone()
    } else {
        format!("{}: {}", account.label, account.value)
    }
}

pub fn organizer_requisites(doc: &VaultDoc, event: &SharedEvent, words: &Words) -> Option<String> {
    let organizer = event
        .participants
        .iter()
        .find(|person| person.id == event.organizer)?;
    let mut parts: Vec<String> = Vec::new();
    if let Some(contact) = organizer
        .contact_id
        .as_deref()
        .and_then(|id| doc.contacts.iter().find(|contact| contact.id == id))
    {
        parts.extend(
            contact
                .accounts
                .iter()
                .filter(|account| account.active)
                .map(account_text),
        );
    } else if let Some(details) = &organizer.details {
        if !details.phone.trim().is_empty() {
            parts.push(format!("{} {}", words.phone, details.phone));
        }
        if !details.messenger.trim().is_empty() {
            parts.push(details.messenger.clone());
        }
        parts.extend(details.requisites.iter().map(account_text));
    }
    (!parts.is_empty()).then(|| {
        format!(
            "{} {}: {}",
            words.requisites,
            organizer.name,
            parts.join(", ")
        )
    })
}

/// The settlement as text for a chat, in `ru` or `en`; `year` is the current
/// one (dates of another year say it).
pub fn share_text(doc: &VaultDoc, event_id: &str, locale: &str, year: i32) -> Result<String> {
    let event = find_event(doc, event_id)?;
    let computed = split::compute(event);
    Ok(compose_share_text(doc, event, &computed, locale, year))
}

pub fn compose_share_text(
    doc: &VaultDoc,
    event: &SharedEvent,
    computed: &Split,
    locale: &str,
    year: i32,
) -> String {
    let english = locale.trim().eq_ignore_ascii_case("en");
    let words = words(english);
    let name_of = |id: &str, object: bool| -> String {
        if id == ME {
            return (if object {
                words.me_object
            } else {
                words.me_subject
            })
            .to_string();
        }
        event
            .participants
            .iter()
            .find(|person| person.id == id)
            .map(|person| person.name.clone())
            .unwrap_or_default()
    };
    let cash = |value: Amount| money(value, &event.currency, english);
    let mut lines: Vec<String> = Vec::new();
    lines.push(match range_text(event, &words, year) {
        Some(dates) => format!("{} · {dates}", event.title),
        None => event.title.clone(),
    });

    let mut bought: Vec<String> = Vec::new();
    for item in event.items.iter().filter(|item| !item.cancelled) {
        for purchase in &item.purchases {
            if let Some(line) = computed
                .purchases
                .iter()
                .find(|line| line.purchase_id == purchase.id)
            {
                bought.push(format!(
                    "• {} — {} ({})",
                    item.title,
                    cash(line.event_amount),
                    name_of(&line.buyer, false)
                ));
            }
        }
    }
    if !bought.is_empty() {
        lines.push(format!("{}:", words.purchases));
        lines.extend(bought);
    }

    let preposition = if english { "for" } else { "на" };
    lines.push(format!(
        "{} {} {preposition} {}",
        words.spent,
        cash(computed.spent),
        people_text(computed.people.len(), english)
    ));
    let shares: Vec<String> = computed
        .people
        .iter()
        .map(|row| format!("{} {}", name_of(&row.id, false), cash(row.share)))
        .collect();
    lines.push(format!("{}: {}", words.shares, shares.join(" · ")));

    let plan = split::transfer_plan(event, computed);
    if plan.is_empty() {
        let any = event.items.iter().any(|item| !item.purchases.is_empty());
        lines.push(
            if any {
                words.settled
            } else {
                words.nothing_left
            }
            .to_string(),
        );
    } else {
        lines.push(format!("{}:", words.who_pays));
        for line in &plan {
            lines.push(format!(
                "• {} → {}: {}",
                name_of(&line.from, false),
                name_of(&line.to, true),
                cash(line.amount)
            ));
        }
    }
    if let Some(requisites) = organizer_requisites(doc, event, &words) {
        lines.push(requisites);
    }
    lines.join("\n")
}
