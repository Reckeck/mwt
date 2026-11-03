//! Automatic reminders on two devices: one per source, whatever else shares
//! their day, with the user's words (done, muted, snoozed) kept by the later
//! edit. Old reminders (random id, schedule-only key) still match new ones.

use super::tests::{settle, two_devices};
use super::*;
use crate::model::planning::{Reminder, ReminderSource, ReminderSourceKind};
use crate::model::store::{LiabilityKind, LiabilityTerms, StoreKind};
use crate::pricing::history::secs_or_now;
use crate::service::StoreExtras;
use crate::service::test_support::amount;

const NOW: &str = "2026-03-10T08:00:00Z";

fn loan(vault: &mut UnlockedVault, name: &str) -> String {
    vault
        .create_store_with(
            name,
            StoreKind::Liability,
            "#c00",
            "",
            None,
            StoreExtras {
                liability: Some(LiabilityTerms {
                    kind: LiabilityKind::Mortgage,
                    rate: None,
                    limit: None,
                    due_day: Some(25),
                    monthly_payment: Some(amount("45000")),
                }),
                ..StoreExtras::default()
            },
        )
        .unwrap()
        .id
}

fn place(vault: &mut UnlockedVault) {
    vault
        .sync_auto_reminders(secs_or_now(NOW), "UTC", false)
        .unwrap();
}

/// A reminder as the build before derived ids made it: a random id and a key
/// of the schedule alone.
fn old_style(vault: &UnlockedVault, store_id: &str, id: &str) -> Reminder {
    let mut reminder = vault
        .doc
        .reminders
        .iter()
        .find(|known| {
            known
                .source
                .as_ref()
                .is_some_and(|source| source.id == store_id)
        })
        .expect("an automatic reminder")
        .clone();
    reminder.id = id.into();
    reminder.source_key = "m25|1|10:00".into();
    reminder
}

fn pause() {
    std::thread::sleep(std::time::Duration::from_millis(5));
}

fn of_loan<'a>(vault: &'a UnlockedVault, store_id: &str) -> Vec<&'a Reminder> {
    vault
        .doc
        .reminders
        .iter()
        .filter(|reminder| {
            reminder.source
                == Some(ReminderSource {
                    kind: ReminderSourceKind::LoanPayment,
                    id: store_id.into(),
                })
        })
        .collect()
}

#[test]
fn two_loans_on_the_same_day_keep_two_reminders() {
    let (_dir, mut left, mut right) = two_devices();
    let first = loan(&mut left, "Mortgage");
    let second = loan(&mut left, "Car loan");
    place(&mut left);
    assert_eq!(left.doc.reminders.len(), 2);
    settle(&mut left, &mut right);
    for vault in [&left, &right] {
        assert_eq!(
            of_loan(vault, &first).len(),
            1,
            "the first loan lost its reminder"
        );
        assert_eq!(
            of_loan(vault, &second).len(),
            1,
            "the second loan lost its reminder"
        );
    }
}

#[test]
fn new_automatic_reminders_get_the_same_id_on_every_device() {
    let (_dir, mut left, mut right) = two_devices();
    let id = loan(&mut left, "Mortgage");
    settle(&mut left, &mut right);
    place(&mut left);
    place(&mut right);
    assert_eq!(of_loan(&left, &id)[0].id, of_loan(&right, &id)[0].id);
    settle(&mut left, &mut right);
    assert_eq!(left.doc.reminders.len(), 1);
    assert_eq!(right.doc.reminders.len(), 1);
}

#[test]
fn dedupe_keeps_done_and_mute_of_a_reminder() {
    let (_dir, mut left, mut right) = two_devices();
    let id = loan(&mut left, "Mortgage");
    settle(&mut left, &mut right);
    place(&mut left);
    let copy = old_style(&left, &id, "random-right");
    left.doc.reminders[0].source_key = "m25|1|10:00".into();
    left.persist().unwrap();
    pause();
    // The other device made its own copy, then the user finished and muted it.
    right.doc.reminders.push(copy);
    right.persist().unwrap();
    right.sync_manifest(None).unwrap();
    pause();
    let last = right.doc.reminders.len() - 1;
    right.doc.reminders[last].done = true;
    right.doc.reminders[last].muted = true;
    right.doc.reminders[last].snoozed_until = Some("2026-03-20T10:00:00Z".into());
    right.persist().unwrap();
    settle(&mut left, &mut right);
    for vault in [&left, &right] {
        let kept = of_loan(vault, &id);
        assert_eq!(kept.len(), 1);
        assert!(
            kept[0].done && kept[0].muted,
            "the user word was lost: {:?}",
            kept
        );
        assert_eq!(
            kept[0].snoozed_until.as_deref(),
            Some("2026-03-20T10:00:00Z")
        );
    }
}

#[test]
fn an_old_reminder_is_not_duplicated_after_the_update() {
    let (_dir, mut left, mut right) = two_devices();
    let id = loan(&mut left, "Mortgage");
    place(&mut left);
    // The old build placed it: random id, schedule-only key, paid already.
    let mut old = old_style(&left, &id, "random-old");
    old.done = true;
    left.doc.reminders.clear();
    left.doc.reminders.push(old);
    left.persist().unwrap();
    settle(&mut left, &mut right);
    // The update: the schedule is unchanged, so nothing is placed anew.
    place(&mut left);
    place(&mut right);
    settle(&mut left, &mut right);
    for vault in [&left, &right] {
        let kept = of_loan(vault, &id);
        assert_eq!(kept.len(), 1, "duplicated: {kept:?}");
        assert!(kept[0].done, "done was undone by the new key");
    }
}
