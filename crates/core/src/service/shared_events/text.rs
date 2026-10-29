//! "Share the settlement": the text to send to the chat. Purchases, shares,
//! who pays whom through the organizer, and the organizer's requisites when
//! they are a contact (or a guest who left some).

use chrono::{Datelike, NaiveDate};
use rust_decimal::Decimal;

use crate::Amount;
use crate::error::Result;
use crate::model::contact::ContactAccount;
use crate::model::shared_event::{ME, SharedEvent};
use crate::service::UnlockedVault;

use super::split::{self, Split};

const NBSP: char = '\u{a0}';

const MONTHS_RU: [&str; 12] = [
    "янв", "фев", "мар", "апр", "мая", "июн", "июл", "авг", "сен", "окт", "ноя", "дек",
];
const MONTHS_EN: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// What the text says, in one language.
struct Words {
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

fn words(english: bool) -> Words {
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
fn people_text(count: usize, english: bool) -> String {
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
fn money(value: Amount, currency: &str, english: bool) -> String {
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
fn range_text(event: &SharedEvent, words: &Words, year: i32) -> Option<String> {
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

impl UnlockedVault {
    fn organizer_requisites(&self, event: &SharedEvent, words: &Words) -> Option<String> {
        let organizer = event
            .participants
            .iter()
            .find(|person| person.id == event.organizer)?;
        let mut parts: Vec<String> = Vec::new();
        if let Some(contact) = organizer
            .contact_id
            .as_deref()
            .and_then(|id| self.doc.contacts.iter().find(|contact| contact.id == id))
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

    /// The settlement as text for a chat, in `ru` or `en`.
    pub fn shared_event_share_text(&self, event_id: &str, locale: &str) -> Result<String> {
        let event = self.find_shared_event(event_id)?;
        let computed = split::compute(event);
        Ok(self.compose_share_text(event, &computed, locale))
    }

    fn compose_share_text(&self, event: &SharedEvent, computed: &Split, locale: &str) -> String {
        let english = locale.trim().eq_ignore_ascii_case("en");
        let words = words(english);
        let year = chrono::Utc::now().year();
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
        if let Some(requisites) = self.organizer_requisites(event, &words) {
            lines.push(requisites);
        }
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::*;

    #[test]
    fn the_skewers_text_has_purchases_shares_and_who_pays_whom() {
        let scenario = skewers_scenario();
        let text = scenario
            .vault
            .shared_event_share_text(&scenario.event, "ru")
            .unwrap();
        assert!(text.starts_with("Шашлыки · 2–4 ноя"), "{text}");
        assert!(text.contains("• Мясо — 4\u{a0}800\u{a0}₽ (Я)"), "{text}");
        assert!(
            text.contains("Потрачено 7\u{a0}100\u{a0}₽ на 5 человек"),
            "{text}"
        );
        assert!(
            text.contains(
                "Доли: Я 1\u{a0}060\u{a0}₽ · Петя 1\u{a0}735\u{a0}₽ · Вася 2\u{a0}185\u{a0}₽ · Аня 1\u{a0}060\u{a0}₽ · Оля 1\u{a0}060\u{a0}₽"
            ),
            "{text}"
        );
        assert!(text.contains("• Аня → мне: 1\u{a0}060\u{a0}₽"), "{text}");
        assert!(text.contains("• Вася → мне: 1\u{a0}685\u{a0}₽"), "{text}");
        // I organize and have no contact of my own: no requisites line
        assert!(!text.contains("Реквизиты"), "{text}");
    }

    #[test]
    fn english_text_and_the_organizers_requisites_from_their_contact() {
        let mut scenario = gift_scenario();
        let anya = scenario.contact("anya");
        let draft = crate::service::ContactDraft {
            name: "Аня".into(),
            accounts: vec![crate::model::contact::ContactAccount {
                label: "Карта".into(),
                kind: crate::model::contact::DetailKind::Card,
                value: "5536 9137 •• 4417".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        scenario.vault.update_contact(&anya, draft).unwrap();
        let ru = scenario
            .vault
            .shared_event_share_text(&scenario.event, "ru")
            .unwrap();
        assert!(
            ru.contains("Реквизиты Аня: Карта: 5536 9137 •• 4417"),
            "{ru}"
        );
        assert!(ru.contains("• Я → Аня: 1\u{a0}850\u{a0}₽"), "{ru}");
        let en = scenario
            .vault
            .shared_event_share_text(&scenario.event, "en")
            .unwrap();
        assert!(en.contains("Spent 7\u{a0}400\u{a0}₽ for 4 people"), "{en}");
        assert!(en.contains("• Me → Аня: 1\u{a0}850\u{a0}₽"), "{en}");
        assert!(en.contains("Pay to Аня"), "{en}");
    }

    #[test]
    fn a_guest_organizer_without_details_has_no_requisites_line() {
        let mut scenario = skewers_scenario();
        let id = scenario.event.clone();
        scenario.vault.find_shared_event_mut(&id).unwrap().organizer = "olya".into();
        let text = scenario.vault.shared_event_share_text(&id, "ru").unwrap();
        assert!(!text.contains("Реквизиты"), "{text}");
    }

    #[test]
    fn dates_read_naturally() {
        let words_ru = words(false);
        let mut event = SharedEvent {
            starts_on: Some("2026-10-30".into()),
            ends_on: Some("2026-11-02".into()),
            ..SharedEvent::default()
        };
        assert_eq!(
            range_text(&event, &words_ru, 2026).unwrap(),
            "30 окт – 2 ноя"
        );
        event.starts_on = Some("2026-11-02".into());
        event.ends_on = None;
        assert_eq!(range_text(&event, &words_ru, 2026).unwrap(), "2 ноя");
        assert_eq!(range_text(&event, &words_ru, 2027).unwrap(), "2 ноя 2026");
        event.starts_on = None;
        assert!(range_text(&event, &words_ru, 2026).is_none());
    }

    #[test]
    fn money_groups_thousands_and_keeps_cents() {
        assert_eq!(money(amount("7100"), "rub", false), "7\u{a0}100\u{a0}₽");
        assert_eq!(
            money(amount("1234567.5"), "usd", true),
            "1\u{a0}234\u{a0}567.5\u{a0}$"
        );
        assert_eq!(money(amount("-60"), "rub", false), "-60\u{a0}₽");
        assert_eq!(money(amount("12.5"), "rub", false), "12,5\u{a0}₽");
        assert_eq!(people_text(5, false), "5 человек");
        assert_eq!(people_text(2, false), "2 человека");
        assert_eq!(people_text(11, false), "11 человек");
    }
}
