//! Tests of the agenda area: the old inline tests (names and bodies unchanged).

use crate::Amount;
use crate::error::Error;
use crate::model::planning::{AlertCondition, PriceAlert};
use crate::model::planning::{Reminder, ReminderPay};
use crate::model::planning::{Schedule, TransactionDraft};
use crate::model::store::StoreKind;
use crate::model::transaction::Transaction;
use crate::model::vault::PricePoint;
use crate::planning::agenda::*;
use crate::service::AutomationDraft;
use crate::service::UnlockedVault;
use crate::service::test_support::*;
use crate::time::unix_secs;

fn secs(iso: &str) -> i64 {
    unix_secs(iso).unwrap()
}

#[test]
fn upcoming_flags_a_payment_the_balance_will_not_cover() {
    let (_dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    let mut seed = income(&card, "500", "seed");
    at(&mut seed, "2026-03-01T00:00:00Z");
    crate::usecases::money::transactions::add_transaction(&mut vault, seed).unwrap();
    let Transaction::Payment(mut rent) = payment(&card, "800", "rent") else {
        unreachable!()
    };
    rent.meta.categories = vec!["rent_mortgage".into()];
    crate::usecases::planning::automations::create_automation(
        &mut vault,
        AutomationDraft {
            time: None,
            asks: false,
            for_month_offset: None,
            name: "Rent".into(),
            active: true,
            preset: None,
            schedule: Schedule::Monthly { day: 15 },
            starts_at: "2026-01-01T00:00:00Z".into(),
            template: TransactionDraft::Payment(rent),
            amount_mode: Default::default(),
            workday_shift: Default::default(),
            property_id: None,
        },
    )
    .unwrap();
    let Transaction::Topup(salary) = income(&card, "1000", "salary") else {
        unreachable!()
    };
    crate::usecases::planning::automations::create_automation(
        &mut vault,
        AutomationDraft {
            time: None,
            asks: false,
            for_month_offset: None,
            name: "Salary".into(),
            active: true,
            preset: None,
            schedule: Schedule::Monthly { day: 20 },
            starts_at: "2026-01-01T00:00:00Z".into(),
            template: TransactionDraft::Topup(salary),
            amount_mode: Default::default(),
            workday_shift: Default::default(),
            property_id: None,
        },
    )
    .unwrap();
    let items = crate::planning::upcoming::upcoming(
        &vault.books(),
        45,
        secs("2026-03-10T00:00:00Z"),
        "UTC",
    );
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
    crate::usecases::planning::agenda::create_price_alert(
        &mut vault,
        PriceAlertDraft {
            asset: btc(),
            condition: AlertCondition::Above,
            price: amount("95000"),
            currency: "usd".into(),
            active: true,
            store_id: None,
            repeats: false,
        },
    )
    .unwrap();
    let now = secs("2026-03-02T00:00:00Z");
    assert_eq!(
        crate::usecases::planning::agenda::check_alerts(&mut vault, now)
            .unwrap()
            .len(),
        1
    );
    assert!(
        crate::usecases::planning::agenda::check_alerts(&mut vault, now)
            .unwrap()
            .is_empty()
    );
    assert!(crate::planning::agenda::alert_assets(&vault.doc).is_empty());
}

#[test]
fn a_reminder_keeps_what_pay_fills_in_and_drops_an_empty_button() {
    let (_dir, mut vault) = empty_vault();
    let card = store(&mut vault, "Card", StoreKind::Bank, None);
    let pay = |store_id: Option<&str>, category: &str| ReminderPay {
        store_id: store_id.map(str::to_string),
        category: Some(category.to_string()),
    };
    let reminder = crate::usecases::planning::agenda::create_reminder_paying(
        &mut vault,
        "ЖКХ",
        "2026-03-10T10:00:00Z",
        None,
        Some(pay(Some(&card), " utilities ")),
    )
    .unwrap();
    assert_eq!(reminder.pay, Some(pay(Some(&card), "utilities")));

    let edited = crate::usecases::planning::agenda::update_reminder_paying(
        &mut vault,
        &reminder.id,
        "ЖКХ",
        "2026-03-10T10:00:00Z",
        None,
        Some(pay(None, " ")),
    )
    .unwrap();
    assert_eq!(edited.pay, None);
    assert!(
        crate::usecases::planning::agenda::create_reminder_paying(
            &mut vault,
            "x",
            "2026-03-10T10:00:00Z",
            None,
            Some(pay(Some("nope"), "food"))
        )
        .is_err()
    );
}

#[test]
fn a_repeating_alert_rearms_after_the_price_crosses_back() {
    let (_dir, mut vault) = empty_vault();
    let price = |vault: &mut UnlockedVault, value: &str, ts: &str| {
        vault.doc.price_cache.push(PricePoint {
            asset_key: btc().key(),
            value_usd: amount(value),
            ts: ts.into(),
            source: "test".into(),
            change_24h_pct: None,
        })
    };
    price(&mut vault, "100000", "2026-03-01T00:00:00Z");
    crate::usecases::planning::agenda::create_price_alert(
        &mut vault,
        PriceAlertDraft {
            asset: btc(),
            condition: AlertCondition::Above,
            price: amount("95000"),
            currency: "usd".into(),
            active: true,
            store_id: None,
            repeats: true,
        },
    )
    .unwrap();
    assert_eq!(
        crate::usecases::planning::agenda::check_alerts(&mut vault, secs("2026-03-01T01:00:00Z"))
            .unwrap()
            .len(),
        1
    );
    assert!(
        crate::usecases::planning::agenda::check_alerts(&mut vault, secs("2026-03-01T02:00:00Z"))
            .unwrap()
            .is_empty()
    );
    assert_eq!(crate::planning::agenda::alert_assets(&vault.doc).len(), 1);
    price(&mut vault, "90000", "2026-03-02T00:00:00Z");
    assert!(
        crate::usecases::planning::agenda::check_alerts(&mut vault, secs("2026-03-02T01:00:00Z"))
            .unwrap()
            .is_empty()
    );
    assert!(vault.doc.price_alerts[0].triggered_at.is_none());
    price(&mut vault, "96000", "2026-03-03T00:00:00Z");
    assert_eq!(
        crate::usecases::planning::agenda::check_alerts(&mut vault, secs("2026-03-03T01:00:00Z"))
            .unwrap()
            .len(),
        1
    );
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
        repeats: false,
    };
    let alert = crate::usecases::planning::agenda::create_price_alert(&mut vault, draft(&exchange))
        .unwrap();
    assert_eq!(alert.store_id.as_deref(), Some(exchange.as_str()));
    assert!(matches!(
        crate::usecases::planning::agenda::create_price_alert(&mut vault, draft(&bank)),
        Err(Error::InvalidInput(_))
    ));
    assert!(matches!(
        crate::usecases::planning::agenda::create_price_alert(&mut vault, draft("nope")),
        Err(Error::Missing(_))
    ));
}

#[test]
fn reminders_come_due_and_can_be_done() {
    let (_dir, mut vault) = empty_vault();
    let reminder = crate::usecases::planning::agenda::create_reminder(
        &mut vault,
        "Pay the nanny",
        "2026-03-05T09:00:00Z",
        None,
    )
    .unwrap();
    assert!(
        crate::planning::agenda::due_reminders(&vault.doc, secs("2026-03-04T00:00:00Z")).is_empty()
    );
    assert_eq!(
        crate::planning::agenda::due_reminders(&vault.doc, secs("2026-03-05T10:00:00Z")).len(),
        1
    );
    crate::usecases::planning::agenda::set_reminder_done(&mut vault, &reminder.id, true).unwrap();
    assert!(
        crate::planning::agenda::due_reminders(&vault.doc, secs("2026-03-05T10:00:00Z")).is_empty()
    );
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
    let reminder = crate::usecases::planning::agenda::create_reminder(
        &mut vault,
        "Rent",
        "2020-01-31T07:30:00Z",
        Some(monthly),
    )
    .unwrap();
    let moved =
        crate::usecases::planning::agenda::set_reminder_done(&mut vault, &reminder.id, true)
            .unwrap();
    assert!(!moved.done, "a repeating reminder stays open");
    assert!(secs(&moved.due_at) > secs("2026-01-01T00:00:00Z"));
    assert_eq!(&moved.due_at[11..], "07:30:00Z", "the time stays");

    let finished =
        crate::usecases::planning::agenda::finish_reminder_series(&mut vault, &reminder.id)
            .unwrap();
    assert!(finished.done && finished.repeat.is_none());
}

#[test]
fn a_snoozed_reminder_waits_until_its_snooze_ends() {
    let (_dir, mut vault) = empty_vault();
    let reminder = crate::usecases::planning::agenda::create_reminder(
        &mut vault,
        "Call",
        "2026-03-05T09:00:00Z",
        None,
    )
    .unwrap();
    crate::usecases::planning::agenda::snooze_reminder(
        &mut vault,
        &reminder.id,
        "2026-03-05T12:00:00Z",
    )
    .unwrap();
    assert!(
        crate::planning::agenda::due_reminders(&vault.doc, secs("2026-03-05T10:00:00Z")).is_empty()
    );
    assert_eq!(
        crate::planning::agenda::due_reminders(&vault.doc, secs("2026-03-05T12:30:00Z")).len(),
        1
    );
    assert!(
        crate::usecases::planning::agenda::snooze_reminder(&mut vault, &reminder.id, "soon")
            .is_err()
    );
}

// -- pins of the methods no old test named, and the pure rules ------------------

#[test]
fn update_reminder_rewrites_the_reminder_and_a_missing_one_is_refused() {
    let (_dir, mut vault) = empty_vault();
    let made = crate::usecases::planning::agenda::create_reminder(
        &mut vault,
        "Call",
        "2026-03-05T09:00:00Z",
        None,
    )
    .unwrap();
    let edited = crate::usecases::planning::agenda::update_reminder(
        &mut vault,
        &made.id,
        " Call the bank ",
        "2026-03-06T09:00:00Z",
        None,
    )
    .unwrap();
    assert_eq!(edited.text, "Call the bank");
    assert_eq!(
        crate::planning::agenda::reminders(&vault.doc)[0].due_at,
        "2026-03-06T09:00:00Z"
    );
    assert!(matches!(
        crate::usecases::planning::agenda::update_reminder(
            &mut vault,
            "nope",
            "x",
            "2026-03-06T09:00:00Z",
            None
        ),
        Err(Error::Missing(_))
    ));
}

#[test]
fn delete_price_alert_removes_it_and_a_missing_one_is_refused() {
    let (_dir, mut vault) = empty_vault();
    let alert = crate::usecases::planning::agenda::create_price_alert(
        &mut vault,
        PriceAlertDraft {
            asset: btc(),
            condition: AlertCondition::Above,
            price: amount("95000"),
            currency: "usd".into(),
            active: true,
            store_id: None,
            repeats: false,
        },
    )
    .unwrap();
    crate::usecases::planning::agenda::delete_price_alert(&mut vault, &alert.id).unwrap();
    assert!(crate::planning::agenda::price_alerts(&vault.doc).is_empty());
    assert!(matches!(
        crate::usecases::planning::agenda::delete_price_alert(&mut vault, &alert.id),
        Err(Error::Missing(_))
    ));
}

#[test]
fn a_repeating_reminder_moves_on_when_done_by_the_pure_rule() {
    let mut reminder = Reminder::new("Pay", "2026-03-05T09:00:00Z");
    reminder.repeat = Some(crate::model::planning::ReminderRepeat {
        every: 1,
        unit: crate::model::planning::RepeatUnit::Month,
    });
    mark_done(&mut reminder, true, secs("2026-03-05T10:00:00Z")).unwrap();
    assert_eq!(reminder.due_at, "2026-04-05T09:00:00Z");
    assert!(!reminder.done);
    let mut once = Reminder::new("Once", "2026-03-05T09:00:00Z");
    mark_done(&mut once, true, secs("2026-03-05T10:00:00Z")).unwrap();
    assert!(once.done);
}

#[test]
fn a_snoozed_reminder_keeps_its_snooze_moment() {
    let (_dir, mut vault) = empty_vault();
    let made = crate::usecases::planning::agenda::create_reminder(
        &mut vault,
        "Rent",
        "2026-03-05T09:00:00Z",
        None,
    )
    .unwrap();
    let snoozed = crate::usecases::planning::agenda::snooze_reminder(
        &mut vault,
        &made.id,
        "2026-03-07T09:00:00Z",
    )
    .unwrap();
    assert_eq!(
        snoozed.snoozed_until.as_deref(),
        Some("2026-03-07T09:00:00Z")
    );
}

#[test]
fn fire_alerts_fires_once_and_rearms_a_repeating_one_on_a_bare_list() {
    let alert = |repeats| PriceAlert {
        id: "a".into(),
        asset: btc(),
        condition: AlertCondition::Above,
        price: amount("95000"),
        currency: "usd".into(),
        active: true,
        triggered_at: None,
        triggered_for: None,
        store_id: None,
        repeats,
    };
    let at = |price: &'static str| move |_: &PriceAlert| Some(amount(price));
    let mut once = vec![alert(false)];
    let (fired, changed) = fire_alerts(&mut once, at("100000"), "t1");
    assert_eq!((fired.len(), changed), (1, true));
    let (fired, changed) = fire_alerts(&mut once, at("100000"), "t2");
    assert_eq!((fired.len(), changed), (0, false));
    let mut again = vec![alert(true)];
    fire_alerts(&mut again, at("100000"), "t1");
    let (fired, changed) = fire_alerts(&mut again, at("90000"), "t2");
    assert_eq!((fired.len(), changed), (0, true), "rearmed, nothing fired");
    assert!(!again[0].is_triggered());
    let (fired, _) = fire_alerts(&mut again, at("100000"), "t3");
    assert_eq!(fired.len(), 1);
    let (fired, changed) = fire_alerts(&mut again, |_| None, "t4");
    assert_eq!((fired.len(), changed), (0, false));
    assert!(alert_assets(&crate::model::vault::VaultDoc::default()).is_empty());
}
