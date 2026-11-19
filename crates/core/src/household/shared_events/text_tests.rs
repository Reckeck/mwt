use super::testing::*;
use super::text::*;
use crate::model::shared_event::SharedEvent;

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
