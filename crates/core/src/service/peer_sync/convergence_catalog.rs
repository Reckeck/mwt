//! Stage M of the sync fixes: the edit kinds that reproduce the catalog bugs
//! (keys and links after a merge) and the invariants checked once the devices
//! have converged. Every violation belongs to a catalog item; an item still
//! open is tolerated only while it is listed in [`KNOWN_OPEN`]. A fix slice
//! removes its item from the list, so the same test then guards the fix.
//!
//! `random_edits_with_no_item_tolerated_stay_green` runs the harness with
//! nothing tolerated and reports the broken invariants per item.

use super::convergence_tests::{Edit, Ledger, Rng, event_ids, guest, pick};
use super::*;
use crate::ipc_contract::{
    AttachPaymentInput, SharedEventDraft, SharedItemDraft, SharedPurchaseDraft,
};
use crate::model::planning::AssetClass;
use crate::model::shared_event::{ItemSplit, ME, PriceMode};
use crate::model::store::{AssetRef, MAX_STORE_DEPTH, StoreKind, StoreScope};
use crate::model::transaction::Transaction;
use crate::pricing::history::unix_secs;
use crate::service::StoreExtras;
use crate::service::allocation::AssetClassTarget;
use crate::service::contacts::DebtTermsDraft;
use crate::service::goals::GoalDraft;
use crate::service::test_support::{amount, income_of, payment, usd};
use crate::service::utilities::UtilityDraft;
use crate::service::valuation::ValuationInput;
use std::collections::{BTreeMap, BTreeSet};

/// One item of the stage M catalog (`trade-app-sync-bugs.md`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Item {
    /// 25: a cycle, a missing parent or a parent that is not a wallet.
    AccountTree25,
    /// 28: a live record names a store, category or contact that is gone.
    DeletedReferenced28,
    /// 103: two debt terms for one contact and asset.
    DebtTerms103,
    /// 104: the shares do not add up to 100, or a link names a missing class.
    AssetClasses104,
    /// 105: two readings for one meter and day, two valuations for one asset and day.
    Readings105,
    /// 114: more than one root Cash wallet.
    CashWallets114,
    /// 115: a live record names a participant that was removed.
    DeletedParticipant115,
    /// 116: the event currency does not match the purchases in it.
    EventCurrency116,
    /// 122: an envelope of a deleted goal, or two for one goal and wallet.
    GoalEnvelopes122,
    /// 123: two contacts for one guest, a payment linked to two events.
    GuestsAndLinks123,
    /// 29: a staged import names an exec id that is booked or discarded.
    StagedImports29,
    /// A purchase added to an item next to another device's one is lost
    /// (fixed by protocol 3: every purchase has its own stamp).
    ParallelPurchases,
    /// 121: one category switch is lost when another device saves the whole
    /// notification object, and the per-device switches travel.
    NoticeSettings121,
    /// 33: an automatic reminder text is rewritten back and forth between
    /// devices that use different languages.
    ReminderText33,
    /// 26: a price stamped in the base currency of one device is read as the
    /// base currency of another.
    PriceBase26,
    /// 100: the flat inflation rates differ between devices that have met.
    FlatInflation100,
    // Left for the slice that adds the field it needs: 27 (`WorkflowCap27`,
    // slice 6, needs `Workflow.max_amount_currency`).
}

/// The items that still reproduce today. Each fix slice removes its own.
pub(super) const KNOWN_OPEN: &[Item] = &[];

#[derive(Clone, Debug)]
pub(super) struct Violation {
    pub item: Item,
    pub text: String,
}

fn found(out: &mut Vec<Violation>, item: Item, text: String) {
    out.push(Violation { item, text });
}

/// A stand-in for a non-financial asset a property holds (valuations need one).
fn flat_asset() -> AssetRef {
    AssetRef {
        symbol: "FLAT".into(),
        name: "Flat".into(),
        coingecko_id: None,
        chain: None,
        contract: None,
        quote: "FLAT".into(),
    }
}

/// The purchase of the flat; the random edits may delete it, so a valuation
/// buys it again when the device holds none.
fn flat_topup(property: &str) -> Transaction {
    let mut flat = income_of(property, flat_asset(), "1", "1", "flat");
    if let Transaction::Topup(topup) = &mut flat {
        topup.details.unit_price = Some(amount("100000"));
    }
    flat
}

/// A property with a flat in it and a metered utility, made on device 0 and
/// pulled by the others, so valuations and readings have something to hang on.
pub(super) fn add_catalog_stores(devices: &mut [UnlockedVault], ledger: &mut Ledger) {
    let property = devices[0]
        .create_store_with(
            "conv flat",
            StoreKind::Property,
            "#555",
            "",
            None,
            StoreExtras {
                property: Some(crate::model::store::PropertyInfo {
                    category: crate::model::store::PropertyCategory::Flat,
                    ..Default::default()
                }),
                ..StoreExtras::default()
            },
        )
        .unwrap()
        .id;
    devices[0].add_transaction(flat_topup(&property)).unwrap();
    ledger.thing_key = flat_asset().key();
    ledger.utility_id = devices[0]
        .create_utility_item(UtilityDraft {
            name: "Electricity".into(),
            active: true,
            kind: crate::model::planning::UtilityKind::Metered {
                unit: "kWh".into(),
                tariff: amount("5"),
                formula: None,
                zones: Vec::new(),
            },
            currency: "usd".into(),
            property_id: property.clone(),
            store_id: None,
            category: "utilities".into(),
        })
        .unwrap()
        .id;
    ledger.property_id = property;
    for name in ["anchor A", "anchor B"] {
        let contact = devices[0]
            .create_contact(crate::service::contacts::ContactDraft {
                name: name.into(),
                kind: crate::model::contact::ContactKind::Person,
                note: String::new(),
                accounts: Vec::new(),
                group: String::new(),
            })
            .unwrap();
        ledger.anchors.push(contact.id);
    }
    for to in 1..devices.len() {
        super::convergence_tests::pull(devices, 0, to);
    }
}

/// One of the first `limit` of the sorted ids. A small pool makes two
/// devices that edit independently reach for the same record now and then.
fn pick_early(rng: &mut Rng, mut ids: Vec<String>, limit: usize) -> Option<String> {
    ids.sort();
    ids.truncate(limit);
    pick(rng, &ids).cloned()
}

fn pick_event(
    vault: &UnlockedVault,
    rng: &mut Rng,
) -> Option<crate::model::shared_event::SharedEvent> {
    let id = pick_early(rng, event_ids(vault), 2)?;
    vault
        .doc
        .shared_events
        .iter()
        .find(|event| event.id == id)
        .cloned()
}

fn purchase_ids(vault: &UnlockedVault, event_id: &str) -> BTreeSet<(String, String)> {
    vault
        .doc
        .shared_events
        .iter()
        .filter(|event| event.id == event_id)
        .flat_map(|event| &event.items)
        .flat_map(|item| {
            item.purchases
                .iter()
                .map(|purchase| (item.id.clone(), purchase.id.clone()))
        })
        .collect()
}

/// The edits of the catalog. A refusal of the core is a normal outcome of a
/// random edit, as in the other appliers.
pub(super) fn apply_catalog_edit(
    kind: Edit,
    vault: &mut UnlockedVault,
    rng: &mut Rng,
    ledger: &mut Ledger,
) {
    let number = ledger.fresh();
    match kind {
        Edit::SetDebtTerms => {
            let mut ids: Vec<String> = ledger
                .anchors
                .iter()
                .filter(|id| vault.doc.contacts.iter().any(|contact| &contact.id == *id))
                .cloned()
                .collect();
            if ids.is_empty() {
                ids = vault
                    .doc
                    .contacts
                    .iter()
                    .map(|contact| contact.id.clone())
                    .collect();
            }
            let Some(contact_id) = pick(rng, &ids).cloned() else {
                return;
            };
            let clear = rng.below(5) == 0;
            let _ = vault.set_debt_terms(DebtTermsDraft {
                contact_id,
                asset: ["USD", "EUR"][rng.below(2)].into(),
                due_at: (!clear).then(|| format!("2026-0{}-1{}", 4 + rng.below(5), rng.below(9))),
                note: if clear {
                    String::new()
                } else {
                    format!("terms {number}")
                },
            });
        }
        Edit::AddMeterReading => {
            let day = 1 + rng.below(3);
            // Roughly rising with the day, so most readings pass the monotonic rule.
            let value = amount(&format!("{}", day * 100 + rng.below(50)));
            let _ = vault.add_meter_reading(
                &ledger.utility_id,
                value,
                "2026-03",
                Some(u8::try_from(day).unwrap_or(1)),
                None,
            );
        }
        Edit::AddValuation => {
            let held = vault.doc.holdings.iter().any(|row| {
                row.store_id == ledger.property_id && row.asset.key() == ledger.thing_key
            });
            if !held {
                let _ = vault.add_transaction(flat_topup(&ledger.property_id));
            }
            let _ = vault.add_valuation(ValuationInput {
                store_id: ledger.property_id.clone(),
                asset_key: ledger.thing_key.clone(),
                date: format!("2026-03-0{}", 1 + rng.below(3)),
                unit_price: amount(&format!("{}", 1000 + rng.below(500))),
                currency: "usd".into(),
            });
        }
        Edit::MoveAccount => {
            let banks: Vec<String> = vault
                .doc
                .stores
                .iter()
                .filter(|candidate| candidate.name.starts_with("bank") && !candidate.archived)
                .map(|candidate| candidate.id.clone())
                .collect();
            let subs: Vec<String> = vault
                .doc
                .stores
                .iter()
                .filter(|candidate| candidate.name.starts_with("sv"))
                .map(|candidate| candidate.id.clone())
                .collect();
            if rng.below(5) == 0 {
                // An empty wallet goes while another device moves an account into it.
                let empty: Vec<String> = banks
                    .iter()
                    .filter(|bank| vault.children_of(bank).is_empty())
                    .cloned()
                    .collect();
                if let Some(bank) = pick_early(rng, empty, 2)
                    && vault.delete_store(&bank).is_ok()
                {
                    ledger.deleted.insert(format!("stores:{bank}"));
                }
                return;
            }
            let (Some(sub), Some(target)) = (pick(rng, &subs).cloned(), pick_early(rng, banks, 2))
            else {
                return;
            };
            if vault.move_store(&sub, Some(&target)).is_ok() {
                ledger.touched_after.insert(format!("stores:{sub}"));
            }
        }
        Edit::CreateGoal => {
            if vault.doc.goals.len() >= 4 {
                return;
            }
            let store_id = vault.doc.stores[0].id.clone();
            let _ = vault.create_goal(GoalDraft {
                name: format!("goal {number}"),
                active: true,
                target_amount: amount("1000"),
                currency: "usd".into(),
                starts_at: "2026-01-01T00:00:00Z".into(),
                deadline: Some("2026-12-31T00:00:00Z".into()),
                priority: 3,
                monthly_commitment: None,
                scope: StoreScope {
                    store_ids: vec![store_id],
                    include_children: true,
                },
                scenario_id: None,
                expected_return: None,
            });
        }
        Edit::DeleteGoal => {
            let ids: Vec<String> = vault.doc.goals.iter().map(|goal| goal.id.clone()).collect();
            let Some(id) = pick(rng, &ids).cloned() else {
                return;
            };
            if vault
                .delete_goal(&id, chrono::Utc::now().timestamp())
                .is_ok()
            {
                ledger.deleted.insert(format!("goals:{id}"));
            }
        }
        Edit::FundGoal => {
            let ids: Vec<String> = vault.doc.goals.iter().map(|goal| goal.id.clone()).collect();
            let Some(id) = pick(rng, &ids).cloned() else {
                return;
            };
            let store_id = vault.doc.stores[0].id.clone();
            if vault
                .fund_goal(&id, &store_id, amount(&format!("{}", 1 + rng.below(20))))
                .is_ok()
            {
                // Funding a goal whose envelope was deleted elsewhere is a new
                // funding, not a resurrection: mark the envelopes it filled.
                for envelope in vault.doc.envelopes.iter().filter(|envelope| {
                    envelope.goal_id.as_deref() == Some(id.as_str())
                        && envelope.store_id == store_id
                }) {
                    ledger
                        .touched_after
                        .insert(format!("envelopes:{}", envelope.id));
                }
            }
        }
        Edit::CreatePayment => {
            let store_id = vault.doc.stores[0].id.clone();
            let _ = vault.add_transaction(payment(
                &store_id,
                &format!("{}", 1 + rng.below(5)),
                "conv pay",
            ));
        }
        Edit::MakeGuestContact => {
            let Some(event) = pick_event(vault, rng) else {
                return;
            };
            let guests: Vec<String> = event
                .participants
                .iter()
                .filter(|person| person.contact_id.is_none())
                .map(|person| person.id.clone())
                .collect();
            if let Some(person) = pick_early(rng, guests, 2).as_ref()
                && vault
                    .make_shared_participant_contact(&event.id, person)
                    .is_ok()
            {
                // An edit of the person after another device removed them wins.
                ledger
                    .touched_after
                    .insert(format!("{}/participant/{person}", event.id));
            }
        }
        Edit::AttachPayment => {
            let Some(event) = pick_event(vault, rng) else {
                return;
            };
            let payments: Vec<String> = vault
                .doc
                .transactions
                .iter()
                .filter(|tx| matches!(tx, Transaction::Payment(_)))
                .map(|tx| tx.meta().id.clone())
                .collect();
            let Some(tx_id) = pick_early(rng, payments, 2) else {
                return;
            };
            let before = purchase_ids(vault, &event.id);
            let attached = vault.attach_payment_to_shared_event(AttachPaymentInput {
                event_id: event.id.clone(),
                tx_id,
                new_item_title: Some(format!("attached {number}")),
                ..Default::default()
            });
            if attached.is_ok() {
                remember_purchases(vault, &event.id, &before, ledger);
            }
        }
        Edit::ChangeEventCurrency => {
            // Only an event nothing is bought in yet may change its currency.
            let bare: Vec<String> = vault
                .doc
                .shared_events
                .iter()
                .filter(|event| event.items.iter().all(|item| item.purchases.is_empty()))
                .map(|event| event.id.clone())
                .collect();
            let Some(event) = pick_early(rng, bare, 3).and_then(|id| {
                vault
                    .doc
                    .shared_events
                    .iter()
                    .find(|event| event.id == id)
                    .cloned()
            }) else {
                return;
            };
            let _ = vault.update_shared_event(
                &event.id,
                SharedEventDraft {
                    title: event.title.clone(),
                    note: event.note.clone(),
                    starts_on: event.starts_on.clone(),
                    ends_on: event.ends_on.clone(),
                    currency: ["usd", "eur"][rng.below(2)].into(),
                    rounding_step: Some(event.rounding_step),
                    category_id: event.category_id.clone(),
                    organizer: event.organizer.clone(),
                    i_participate: event.i_participate,
                    participants: Vec::new(),
                },
            );
        }
        Edit::FireAlert => fire_alert(vault, number),
        Edit::ChangeAlertPrice => edit_alert(vault, rng),
        Edit::ConfirmImport => confirm_import(vault, rng, ledger),
        Edit::SetAssetClasses => set_asset_classes(vault, rng),
        Edit::UndoLastBatch => undo_last_batch(vault, ledger),
        Edit::SharedPurchaseToItem => {
            let Some(event) = pick_event(vault, rng) else {
                return;
            };
            let (Some(item), Some(person)) = (
                pick(rng, &event.items).cloned(),
                pick(rng, &event.participants).cloned(),
            ) else {
                return;
            };
            let before = purchase_ids(vault, &event.id);
            let added = vault.add_shared_purchase(
                &event.id,
                SharedPurchaseDraft {
                    item_id: Some(item.id),
                    buyer: person.id,
                    qty: amount("1"),
                    amount: amount(&format!("{}", 10 + rng.below(90))),
                    currency: event.currency.clone(),
                    price_mode: PriceMode::Total,
                    ..Default::default()
                },
            );
            if added.is_ok() {
                remember_purchases(vault, &event.id, &before, ledger);
            }
        }
        Edit::SetNoticeCategory => set_notice_category(vault, ledger, number),
        Edit::ToggleSystemNotices => toggle_system_notices(vault, rng, ledger),
        Edit::OpenReminderList => {
            let _ = vault.sync_auto_reminders(REMINDER_NOW, "UTC", device_is_ru(vault));
        }
        Edit::TxWithBasePrice => tx_with_base_price(vault, ledger, number),
        Edit::SetFlatInflation => set_flat_inflation(vault, rng),
        _ => {}
    }
}

/// A fixed "now" so the automatic reminders of every run are the same.
const REMINDER_NOW: i64 = 1_772_323_200; // 2026-03-01T00:00:00Z

/// Rubles per dollar on every device (`seed_rub_rate`).
const RUB_PER_USD: i64 = 100;

/// The language of a device follows its base currency: the device with the
/// ruble base is the Russian one (`run` gives device 1 that base).
fn device_is_ru(vault: &UnlockedVault) -> bool {
    vault.base_currency() == "rub"
}

/// A rate for the second base currency, so a price can be read in either.
pub(super) fn seed_rub_rate(vault: &mut UnlockedVault) {
    vault.apply_fx(&[crate::model::vault::FxPoint {
        currency: "rub".into(),
        per_usd: amount(&RUB_PER_USD.to_string()),
        ts: "2020-01-01T00:00:00Z".into(),
        source: "conv".into(),
    }]);
    vault.persist().unwrap();
}

/// 121: the form sends the whole object; only one category changes in it.
const NOTICE_CATEGORIES: [crate::model::notify::NoticeCategory; 6] = {
    use crate::model::notify::NoticeCategory::{Debt, Goal, Loan, Reminder, Rent, Utilities};
    [Reminder, Loan, Rent, Utilities, Debt, Goal]
};

fn notice_leads(vault: &UnlockedVault) -> Vec<u16> {
    NOTICE_CATEGORIES
        .iter()
        .map(|category| {
            vault
                .doc
                .settings
                .notifications
                .category(*category)
                .lead_days
        })
        .collect()
}

fn set_notice_category(vault: &mut UnlockedVault, ledger: &mut Ledger, number: usize) {
    let category = NOTICE_CATEGORIES[number % NOTICE_CATEGORIES.len()];
    let mut config = vault.doc.settings.notifications.clone();
    let mut entry = config.category(category);
    // Far from every default, and different for every edit.
    entry.lead_days = u16::try_from(31 + number % 50).unwrap_or(31);
    match config
        .categories
        .iter_mut()
        .find(|cfg| cfg.category == category)
    {
        Some(slot) => *slot = entry.clone(),
        None => config.categories.push(entry.clone()),
    }
    let saved = vault.update_settings(crate::ipc_contract::UpdateSettingsInput {
        notifications: Some(config),
        ..Default::default()
    });
    if saved.is_ok() {
        ledger.notice_writes.push((category, entry.lead_days));
    }
}

/// 121: the two switches belong to the device that sets them.
fn toggle_system_notices(vault: &mut UnlockedVault, rng: &mut Rng, ledger: &mut Ledger) {
    let mut config = vault.doc.settings.notifications.clone();
    config.system = rng.below(2) == 0;
    config.hide_details_when_locked = rng.below(2) == 0;
    let flags = (config.system, config.hide_details_when_locked);
    let saved = vault.update_settings(crate::ipc_contract::UpdateSettingsInput {
        notifications: Some(config),
        ..Default::default()
    });
    if saved.is_ok() {
        ledger.device_flags.insert(vault.path.clone(), flags);
    }
}

/// 100: a flat rate of one currency; the form sends the whole table, so the
/// other currencies go back as they were on this device.
fn set_flat_inflation(vault: &mut UnlockedVault, rng: &mut Rng) {
    let currency = ["rub", "usd", "eur", "thb", "gbp"][rng.below(5)];
    let mut table = vault.doc.settings.inflation_manual.clone();
    table.insert(
        currency.to_string(),
        amount(&format!("{}.5", 2 + rng.below(30))),
    );
    let _ = vault.update_settings(crate::ipc_contract::UpdateSettingsInput {
        inflation_manual: Some(table),
        ..Default::default()
    });
}

/// 100: once the devices have met they compute with one table of flat rates.
pub(super) fn check_flat_inflation_agrees(devices: &[UnlockedVault]) -> Vec<Violation> {
    let mut out = Vec::new();
    let first = devices[0].flat_inflation();
    for (index, device) in devices.iter().enumerate().skip(1) {
        let table = device.flat_inflation();
        if table != first {
            found(
                &mut out,
                Item::FlatInflation100,
                format!("device {index} has the flat rates {table:?}, device 0 has {first:?}"),
            );
        }
    }
    out
}

/// 26: an income whose price in the base currency is written in the base of
/// the device that entered it.
fn tx_with_base_price(vault: &mut UnlockedVault, ledger: &mut Ledger, number: usize) {
    let store_id = vault.doc.stores[0].id.clone();
    let usd_price = 2 + number % 5;
    let in_base = if vault.base_currency() == "rub" {
        usd_price as i64 * RUB_PER_USD
    } else {
        usd_price as i64
    };
    let mut tx = income_of(&store_id, usd(), "10", &usd_price.to_string(), "based");
    if let Transaction::Topup(topup) = &mut tx {
        topup.price.price_base = Some(amount(&in_base.to_string()));
        topup.price.price_base_currency = Some(vault.base_currency().to_string());
        topup.meta.timestamp = "2026-02-01T00:00:00Z".into();
    }
    if let Ok(saved) = vault.add_transaction(tx) {
        ledger
            .base_priced
            .push((saved.meta().id.clone(), amount(&usd_price.to_string())));
    }
}

/// 33: every device opens the reminder list in its own language, twice, with
/// an exchange after each opening. The second opening must find nothing to
/// rewrite and the second exchange nothing to send.
pub(super) fn reminder_text_ping_pong(devices: &mut [UnlockedVault]) -> Vec<Violation> {
    let mut out = Vec::new();
    for cycle in 0..2 {
        let mut rewrote = false;
        for device in devices.iter_mut() {
            let ru = device_is_ru(device);
            rewrote |= device.sync_auto_reminders(REMINDER_NOW, "UTC", ru).unwrap();
        }
        let mut sent = false;
        for _ in 0..3 {
            for from in 0..devices.len() {
                for to in 0..devices.len() {
                    if from != to {
                        sent |= super::convergence_tests::pull(devices, from, to);
                    }
                }
            }
        }
        if cycle == 1 && (rewrote || sent) {
            found(
                &mut out,
                Item::ReminderText33,
                format!("the second opening of the reminder list rewrote={rewrote} sent={sent}"),
            );
        }
    }
    out
}

fn remember_purchases(
    vault: &UnlockedVault,
    event_id: &str,
    before: &BTreeSet<(String, String)>,
    ledger: &mut Ledger,
) {
    for (item, purchase) in purchase_ids(vault, event_id).difference(before) {
        ledger
            .purchases
            .push((event_id.to_string(), item.clone(), purchase.clone()));
    }
}

/// Baskets drawn from three fixed ids, so two devices editing at once edit the
/// same records; shares add up to 100 on the device that saves them.
fn alert_draft(price: usize) -> crate::service::PriceAlertDraft {
    crate::service::PriceAlertDraft {
        asset: crate::service::test_support::btc(),
        condition: crate::model::planning::AlertCondition::Above,
        price: amount(&price.to_string()),
        currency: "usd".into(),
        active: true,
        store_id: None,
        repeats: false,
    }
}

/// A new price lands in this device's cache and the armed alerts are checked.
/// Prices sit on a small pool of hours, so the cache stays far below its caps
/// (thinning a bigger one is a different test's business). A price depends
/// only on its hour: two devices never write one cache record differently.
fn fire_alert(vault: &mut UnlockedVault, number: usize) {
    const HOURS: usize = 20;
    let base = unix_secs("2026-03-01T00:00:00Z").unwrap();
    let hour = i64::try_from(number % HOURS).unwrap_or(0);
    let stamp = chrono::DateTime::from_timestamp(base + hour * 3600, 0)
        .unwrap()
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    vault.apply_prices(&[crate::model::vault::PricePoint {
        asset_key: crate::service::test_support::btc().key(),
        value_usd: amount(&(90_000 + hour * 2_000).to_string()),
        ts: stamp,
        source: "conv".into(),
        change_24h_pct: None,
    }]);
    vault.check_alerts(base + 3600 * HOURS as i64 + 60).unwrap();
}

/// Queues a fill from a small pool of exec ids on the exchange wallet and
/// then, more often than not, books or discards it. Two devices reach for the
/// same id, so one decides while the other has not seen it (29).
fn confirm_import(vault: &mut UnlockedVault, rng: &mut Rng, ledger: &Ledger) {
    let exec_id = format!("imp-{}", rng.below(4));
    let fill = crate::exchange::Execution {
        exec_id: exec_id.clone(),
        symbol: "BTCUSDT".into(),
        side: "buy".into(),
        order_type: "Market".into(),
        exec_price: "65000".into(),
        exec_qty: "0.5".into(),
        exec_value: "32500".into(),
        exec_fee: "0.5".into(),
        exec_time: "1700000000000".into(),
        is_maker: false,
        closed_size: None,
        category: String::new(),
    };
    let Some(pending) =
        crate::model::vault::PendingImport::from_execution(&ledger.exchange_id, "spot", &fill)
    else {
        return;
    };
    let _ = vault.stage_imports(vec![pending]);
    let ids = [exec_id];
    let _ = match rng.below(6) {
        0..=2 => vault.confirm_pending_imports(&ids),
        3 => vault.discard_pending_imports(&ids),
        _ => Ok(0),
    };
}

/// Creates the one alert the harness plays with, then moves its threshold.
fn edit_alert(vault: &mut UnlockedVault, rng: &mut Rng) {
    let price = 90_000 + rng.below(40_000);
    let id = vault
        .doc
        .price_alerts
        .iter()
        .map(|alert| alert.id.clone())
        .min();
    match id {
        Some(id) => {
            vault.update_price_alert(&id, alert_draft(price)).unwrap();
        }
        None => {
            vault.create_price_alert(alert_draft(price)).unwrap();
        }
    }
}

/// An alert counts as fired on every device or on none. (A firing recorded
/// for an older threshold stays in `triggered_at`: that is the normal merged
/// state, `is_triggered` is what must agree.)
pub(super) fn check_alerts_agree(devices: &[UnlockedVault], context: &str) {
    for other in &devices[1..] {
        for alert in &devices[0].doc.price_alerts {
            let twin = other
                .doc
                .price_alerts
                .iter()
                .find(|found| found.id == alert.id);
            assert!(
                twin.is_some_and(|twin| twin.is_triggered() == alert.is_triggered()),
                "{context}: alert {} counts as fired on one device only",
                alert.id
            );
        }
    }
}

fn set_asset_classes(vault: &mut UnlockedVault, rng: &mut Rng) {
    let ids = ["cls-a", "cls-b", "cls-c"];
    let count = 1 + rng.below(ids.len());
    let first = rng.below(ids.len());
    let chosen: Vec<&str> = (0..count)
        .map(|offset| ids[(first + offset) % ids.len()])
        .collect();
    let mut classes = Vec::new();
    let mut left = 100usize;
    for (index, id) in chosen.iter().enumerate() {
        let share = if index + 1 == chosen.len() {
            left
        } else {
            let part = 10 * (1 + rng.below(left / 10 - (chosen.len() - index - 1)));
            left -= part;
            part
        };
        classes.push(AssetClass {
            id: (*id).into(),
            name: format!("class {id}"),
            target_pct: amount(&format!("{share}")),
            cap: false,
        });
    }
    match rng.below(6) {
        // Back to the preset.
        0 => {
            let _ = vault.set_asset_classes(Vec::new());
        }
        // Put an asset or a wallet in one of the baskets.
        1 => {
            let target = if rng.below(2) == 0 {
                AssetClassTarget::Asset {
                    asset_key: usd().key(),
                }
            } else {
                AssetClassTarget::Store {
                    store_id: vault.doc.stores[0].id.clone(),
                }
            };
            let class = ids[rng.below(ids.len())].to_string();
            let _ = vault.assign_asset_class(target, Some(class));
        }
        _ => {
            let _ = vault.set_asset_classes(classes);
        }
    }
}

fn purchase_present(
    doc: &crate::model::vault::VaultDoc,
    event_id: &str,
    item_id: &str,
    purchase_id: &str,
) -> bool {
    doc.shared_events
        .iter()
        .find(|event| event.id == event_id)
        .and_then(|event| event.items.iter().find(|item| item.id == item_id))
        .is_some_and(|item| {
            item.purchases
                .iter()
                .any(|purchase| purchase.id == purchase_id)
        })
}

/// Undo the newest batch this device applied. Undo brings records back, so
/// everything the batch touched is exempt from the "deleted stays deleted" check.
fn undo_last_batch(vault: &mut UnlockedVault, ledger: &mut Ledger) {
    let Some(batch) = vault
        .doc
        .sync
        .batches
        .iter()
        .rev()
        .find(|batch| batch.undone_at.is_none() && !batch.entries.is_empty())
        .cloned()
    else {
        return;
    };
    let before: Vec<bool> = ledger
        .purchases
        .iter()
        .map(|(event, item, purchase)| purchase_present(&vault.doc, event, item, purchase))
        .collect();
    let leads_before = notice_leads(vault);
    let undone = vault
        .sync_undo(&batch.id)
        .is_ok_and(|report| report.restored > 0);
    // The same for a notice lead: the value the undo put back counts as written.
    for (index, lead) in notice_leads(vault).into_iter().enumerate() {
        if leads_before.get(index) != Some(&lead) {
            ledger.notice_writes.push((NOTICE_CATEGORIES[index], lead));
        }
    }
    // An explicit undo may remove a purchase the batch had added: that is the
    // user's own decision, so only what was there before and is gone after.
    let mut index = 0;
    ledger.purchases.retain(|(event, item, purchase)| {
        let was_present = before.get(index).copied().unwrap_or(false);
        index += 1;
        !(was_present && !purchase_present(&vault.doc, event, item, purchase))
    });
    if undone {
        let keys: Vec<String> = ledger.deleted.iter().cloned().collect();
        for key in keys {
            let owner = key.split('/').next().unwrap_or(&key).to_string();
            let touched = batch
                .entries
                .iter()
                .any(|entry| entry.key == key || (key.contains('/') && entry.key.contains(&owner)));
            if touched {
                ledger.touched_after.insert(key);
            }
        }
    }
}

/// Every invariant of the stage on one converged device.
/// 116: purchases with the automatic rate 1 in another currency that repair
/// R7 had to settle. With one such currency it moves the event to it. With two
/// or more no rate can be invented: the purchases stay, named in
/// `needs_rate` and marked in the view, so they are not a violation here.
pub(super) fn unrepaired_rate_one(
    event: &crate::model::shared_event::SharedEvent,
) -> Vec<&crate::model::shared_event::Purchase> {
    let purchases = || event.items.iter().flat_map(|item| &item.purchases);
    let auto: BTreeSet<&str> = purchases()
        .filter(|purchase| purchase.has_auto_rate_one())
        .map(|purchase| purchase.currency.as_str())
        .collect();
    if auto.len() > 1 {
        return Vec::new();
    }
    purchases()
        .filter(|purchase| purchase.needs_rate(&event.currency))
        .collect()
}

pub(super) fn invariants(vault: &UnlockedVault, ledger: &Ledger) -> Vec<Violation> {
    let mut out = Vec::new();
    check_cash_and_tree(vault, &mut out);
    check_references(vault, &mut out);
    check_natural_keys(vault, &mut out);
    check_shared_events(vault, ledger, &mut out);
    check_staged_imports(vault, &mut out);
    check_notice_settings(vault, ledger, &mut out);
    check_base_prices(vault, ledger, &mut out);
    out
}

/// 121: a switch written on one device is still there once the devices met,
/// whatever other devices saved around it; the two per-device switches are the
/// ones that device set.
fn check_notice_settings(vault: &UnlockedVault, ledger: &Ledger, out: &mut Vec<Violation>) {
    let config = &vault.doc.settings.notifications;
    let categories: BTreeSet<String> = ledger
        .notice_writes
        .iter()
        .map(|(category, _)| format!("{category:?}"))
        .collect();
    for name in categories {
        let written: Vec<u16> = ledger
            .notice_writes
            .iter()
            .filter(|(category, _)| format!("{category:?}") == name)
            .map(|(_, lead)| *lead)
            .collect();
        let now = ledger
            .notice_writes
            .iter()
            .find(|(category, _)| format!("{category:?}") == name)
            .map(|(category, _)| config.category(*category).lead_days);
        if let Some(now) = now
            && !written.contains(&now)
        {
            found(
                out,
                Item::NoticeSettings121,
                format!("{name}: lead {now} is none of the written {written:?}"),
            );
        }
    }
    if let Some((system, hide)) = ledger.device_flags.get(&vault.path)
        && (*system, *hide) != (config.system, config.hide_details_when_locked)
    {
        found(
            out,
            Item::NoticeSettings121,
            format!(
                "this device set system={system} hide={hide}, it has system={} hide={}",
                config.system, config.hide_details_when_locked
            ),
        );
    }
}

/// 26: the price of a unit in the reader's base currency is the USD price
/// converted, not the number another device wrote in its own base.
fn check_base_prices(vault: &UnlockedVault, ledger: &Ledger, out: &mut Vec<Violation>) {
    let book = vault.rate_book();
    let base = vault.base_currency().to_string();
    let rate = if base == "rub" { RUB_PER_USD } else { 1 };
    for (id, usd_price) in &ledger.base_priced {
        let Some(tx) = vault.doc.transactions.iter().find(|tx| &tx.meta().id == id) else {
            continue;
        };
        let secs = crate::pricing::history::secs_or_now(&tx.meta().timestamp);
        let Some(leg) = tx.legs().into_iter().next() else {
            continue;
        };
        let wanted = *usd_price * amount(&rate.to_string());
        let got = book.leg_price_in_base(&leg, &base, secs);
        if got != wanted {
            found(
                out,
                Item::PriceBase26,
                format!("tx {id}: price in {base} is {got:?}, converted from USD it is {wanted:?}"),
            );
        }
    }
}

/// 29: nothing in the queue names a fill that is already booked or discarded.
fn check_staged_imports(vault: &UnlockedVault, out: &mut Vec<Violation>) {
    let decided = vault.decided_import_ids();
    for pending in &vault.doc.pending_imports {
        if decided.contains(&pending.exec_id) {
            found(
                out,
                Item::StagedImports29,
                format!(
                    "pending import {} is already booked or discarded",
                    pending.exec_id
                ),
            );
        }
    }
}

fn check_cash_and_tree(vault: &UnlockedVault, out: &mut Vec<Violation>) {
    let roots = vault
        .doc
        .stores
        .iter()
        .filter(|candidate| candidate.kind == StoreKind::Cash && candidate.parent_id.is_none())
        .count();
    if roots != 1 {
        found(
            out,
            Item::CashWallets114,
            format!("{roots} root Cash wallets"),
        );
    }
    let by_id: BTreeMap<&str, &crate::model::store::Store> = vault
        .doc
        .stores
        .iter()
        .map(|candidate| (candidate.id.as_str(), candidate))
        .collect();
    for node in &vault.doc.stores {
        let Some(parent_id) = node.parent_id.as_deref() else {
            continue;
        };
        let Some(parent) = by_id.get(parent_id) else {
            found(
                out,
                Item::AccountTree25,
                format!("account {} has a missing parent {parent_id}", node.id),
            );
            continue;
        };
        if parent.parent_id.is_some() {
            // Covers a cycle too: every member of one has a parent that has one.
            found(
                out,
                Item::AccountTree25,
                format!("account {} sits under the account {parent_id}", node.id),
            );
        } else if MAX_STORE_DEPTH == 0 {
            found(
                out,
                Item::AccountTree25,
                format!("account {} is nested", node.id),
            );
        }
        if parent.kind != node.kind {
            found(
                out,
                Item::AccountTree25,
                format!("account {} is not of its parent's kind", node.id),
            );
        }
    }
}

fn check_references(vault: &UnlockedVault, out: &mut Vec<Violation>) {
    let doc = &vault.doc;
    let stores: BTreeSet<&str> = doc.stores.iter().map(|node| node.id.as_str()).collect();
    let categories: BTreeSet<&str> = doc.categories.iter().map(|cat| cat.key.as_str()).collect();
    let contacts: BTreeSet<&str> = doc
        .contacts
        .iter()
        .map(|contact| contact.id.as_str())
        .collect();
    let goals: BTreeSet<&str> = doc.goals.iter().map(|goal| goal.id.as_str()).collect();
    let classes: BTreeSet<String> = vault
        .effective_classes()
        .into_iter()
        .map(|class| class.id)
        .collect();
    let mut missing = |item: Item, what: &str, owner: String, target: &str, known: bool| {
        if !known {
            found(
                out,
                item,
                format!("{owner} names a missing {what} {target}"),
            );
        }
    };
    for tx in &doc.transactions {
        let owner = format!("transaction {}", tx.meta().id);
        for id in tx.store_ids() {
            missing(
                Item::DeletedReferenced28,
                "store",
                owner.clone(),
                &id,
                stores.contains(id.as_str()),
            );
        }
        for key in &tx.meta().categories {
            missing(
                Item::DeletedReferenced28,
                "category",
                owner.clone(),
                key,
                categories.contains(key.as_str()),
            );
        }
        for id in tx.contact_ids() {
            missing(
                Item::DeletedReferenced28,
                "contact",
                owner.clone(),
                id,
                contacts.contains(id),
            );
        }
    }
    for envelope in &doc.envelopes {
        let owner = format!("envelope {}", envelope.id);
        missing(
            Item::DeletedReferenced28,
            "store",
            owner.clone(),
            &envelope.store_id,
            stores.contains(envelope.store_id.as_str()),
        );
        if let Some(goal) = &envelope.goal_id {
            missing(
                Item::GoalEnvelopes122,
                "goal",
                owner,
                goal,
                goals.contains(goal.as_str()),
            );
        }
    }
    for terms in &doc.debt_terms {
        missing(
            Item::DeletedReferenced28,
            "contact",
            format!("debt terms {}", terms.id),
            &terms.contact_id,
            contacts.contains(terms.contact_id.as_str()),
        );
    }
    for valuation in &doc.valuations {
        missing(
            Item::DeletedReferenced28,
            "store",
            format!("valuation {}", valuation.id),
            &valuation.store_id,
            stores.contains(valuation.store_id.as_str()),
        );
    }
    for link in &doc.asset_class_of {
        missing(
            Item::AssetClasses104,
            "class",
            format!("asset {}", link.asset_key),
            &link.class_id,
            classes.contains(&link.class_id),
        );
    }
    for node in &doc.stores {
        if let Some(class) = &node.asset_class {
            missing(
                Item::AssetClasses104,
                "class",
                format!("wallet {}", node.id),
                class,
                classes.contains(class),
            );
        }
    }
}

fn check_natural_keys(vault: &UnlockedVault, out: &mut Vec<Violation>) {
    let doc = &vault.doc;
    let mut seen = BTreeSet::new();
    for terms in &doc.debt_terms {
        if !seen.insert((terms.contact_id.clone(), terms.asset.clone())) {
            found(
                out,
                Item::DebtTerms103,
                format!(
                    "second debt terms for {} in {}",
                    terms.contact_id, terms.asset
                ),
            );
        }
    }
    let mut seen = BTreeSet::new();
    for reading in &doc.meter_readings {
        let key = (
            reading.item_id.clone(),
            reading.zone.clone(),
            reading.sort_key(),
        );
        if !seen.insert(key) {
            found(
                out,
                Item::Readings105,
                format!(
                    "second reading of {} for {}",
                    reading.item_id, reading.period
                ),
            );
        }
    }
    let mut seen = BTreeSet::new();
    for valuation in &doc.valuations {
        if !seen.insert((valuation.asset_key.clone(), valuation.date.clone())) {
            found(
                out,
                Item::Readings105,
                format!(
                    "second valuation of {} on {}",
                    valuation.asset_key, valuation.date
                ),
            );
        }
    }
    let mut seen = BTreeSet::new();
    for envelope in &doc.envelopes {
        if let Some(goal) = &envelope.goal_id
            && !seen.insert((
                goal.clone(),
                envelope.store_id.clone(),
                envelope.asset.key(),
            ))
        {
            found(
                out,
                Item::GoalEnvelopes122,
                format!("second envelope of goal {goal} in {}", envelope.store_id),
            );
        }
    }
    let total = doc
        .asset_classes
        .iter()
        .fold(crate::Amount::ZERO, |sum, class| sum + class.target_pct);
    if doc
        .asset_classes
        .iter()
        .any(|class| !class.target_pct.is_zero())
        && total != crate::Amount::from(100)
    {
        found(
            out,
            Item::AssetClasses104,
            format!("shares add up to {total}"),
        );
    }
}

fn check_shared_events(vault: &UnlockedVault, ledger: &Ledger, out: &mut Vec<Violation>) {
    let doc = &vault.doc;
    let contacts: BTreeSet<&str> = doc
        .contacts
        .iter()
        .map(|contact| contact.id.as_str())
        .collect();
    // Money references: a purchase or a settlement names a payment that exists.
    // No exception: a device that lost the value reports it, but after the full
    // exchange the device holding the value has restored the payment for all.
    for event in &doc.shared_events {
        let purchases = event.items.iter().flat_map(|item| &item.purchases);
        let named = purchases
            .filter_map(|purchase| purchase.tx_id.as_deref())
            .chain(
                event
                    .settlements
                    .iter()
                    .filter_map(|settlement| settlement.tx_id.as_deref()),
            );
        for tx_id in named {
            if !doc.transactions.iter().any(|tx| tx.meta().id == tx_id) {
                found(
                    out,
                    Item::DeletedReferenced28,
                    format!("event {} names the missing operation {tx_id}", event.id),
                );
            }
        }
    }
    let mut guest_names: BTreeMap<&str, usize> = BTreeMap::new();
    for contact in &doc.contacts {
        if contact.name.starts_with("guest ") {
            *guest_names.entry(contact.name.as_str()).or_default() += 1;
        }
    }
    for (name, count) in guest_names.iter().filter(|(_, count)| **count > 1) {
        found(
            out,
            Item::GuestsAndLinks123,
            format!("{count} contacts named {name}"),
        );
    }
    let mut links: BTreeMap<String, usize> = BTreeMap::new();
    for link in vault.shared_tx_links() {
        *links.entry(link.tx_id).or_default() += 1;
    }
    for (tx_id, count) in links.iter().filter(|(_, count)| **count > 1) {
        found(
            out,
            Item::GuestsAndLinks123,
            format!("payment {tx_id} is linked {count} times"),
        );
    }
    for event in &doc.shared_events {
        let people: BTreeSet<&str> = event
            .participants
            .iter()
            .map(|person| person.id.as_str())
            .chain([ME])
            .collect();
        let mut named = |who: &str, what: &str| {
            if !people.contains(who) {
                found(
                    out,
                    Item::DeletedParticipant115,
                    format!(
                        "event {}: {what} names a missing participant {who}",
                        event.id
                    ),
                );
            }
        };
        named(&event.organizer, "the organizer");
        for settlement in &event.settlements {
            named(&settlement.participant_id, "a settlement");
        }
        for fix in &event.adjustments {
            named(&fix.participant_id, "an adjustment");
        }
        for item in &event.items {
            match &item.split {
                ItemSplit::Equal { among } => among.iter().for_each(|who| named(who, "a split")),
                ItemSplit::ByUnits { units } => {
                    units.keys().for_each(|who| named(who, "a split"));
                }
            }
            for purchase in &item.purchases {
                named(&purchase.buyer, "a purchase");
            }
        }
        for person in &event.participants {
            if let Some(contact) = &person.contact_id
                && !contacts.contains(contact.as_str())
            {
                found(
                    out,
                    Item::DeletedReferenced28,
                    format!(
                        "participant {} names a missing contact {contact}",
                        person.id
                    ),
                );
            }
        }
        for purchase in unrepaired_rate_one(event) {
            found(
                out,
                Item::EventCurrency116,
                format!(
                    "event {}: purchase {} in {} has rate 1 to {}",
                    event.id, purchase.id, purchase.currency, event.currency
                ),
            );
        }
    }
    for (event_id, item_id, purchase_id) in &ledger.purchases {
        let Some(item) = doc
            .shared_events
            .iter()
            .find(|event| &event.id == event_id)
            .and_then(|event| event.items.iter().find(|item| &item.id == item_id))
        else {
            continue;
        };
        if !item
            .purchases
            .iter()
            .any(|purchase| &purchase.id == purchase_id)
            && !link_moved_or_gone(doc, purchase_id)
        {
            found(
                out,
                Item::ParallelPurchases,
                format!(
                    "purchase {purchase_id} of item {item_id} is gone (payment {})",
                    link_state(doc, purchase_id)
                ),
            );
        }
    }
}

/// What a failure report says about the payment behind a `link-` purchase.
fn link_state(doc: &crate::model::vault::VaultDoc, purchase_id: &str) -> String {
    let payment_of = |id: &str| crate::service::sync_ids::payment_link_id(id) == purchase_id;
    let live = doc.transactions.iter().find(|tx| payment_of(&tx.meta().id));
    let dead = doc
        .sync
        .tombstones
        .iter()
        .find(|(key, _)| key.strip_prefix("transactions:").is_some_and(payment_of));
    format!(
        "live={:?} tombstone={:?}",
        live.map(|tx| tx.meta().id.clone()),
        dead.map(|(key, _)| key.clone())
    )
}

/// A payment's `link-` purchase legitimately leaves its place when the payment
/// is deleted or attached again (it moves to the new item): neither is a loss.
fn link_moved_or_gone(doc: &crate::model::vault::VaultDoc, purchase_id: &str) -> bool {
    if !purchase_id.starts_with("link-") {
        return false;
    }
    let payment_exists = doc
        .transactions
        .iter()
        .any(|tx| crate::service::sync_ids::payment_link_id(&tx.meta().id) == purchase_id);
    // Gone for good only when the payment carries a tombstone: a payment that
    // is merely absent (no tombstone) is a loss the check must still see.
    let payment_deleted = doc.sync.tombstones.keys().any(|key| {
        key.strip_prefix("transactions:")
            .is_some_and(|id| crate::service::sync_ids::payment_link_id(id) == purchase_id)
    });
    let placed_elsewhere = doc
        .shared_events
        .iter()
        .flat_map(|event| &event.items)
        .any(|item| {
            item.purchases
                .iter()
                .any(|purchase| purchase.id == purchase_id)
        });
    (!payment_exists && payment_deleted) || placed_elsewhere
}

/// One device adds a purchase to an item the other renames before it sees the
/// purchase. Returns how many purchases each device holds after they meet.
#[cfg(test)]
pub(super) fn purchase_beside_a_later_item_edit() -> (usize, usize) {
    let (_dir, mut left, mut right) = super::tests::two_devices();
    let event = left
        .create_shared_event(SharedEventDraft {
            title: "trip".into(),
            currency: "usd".into(),
            participants: vec![guest("guest".into())],
            ..Default::default()
        })
        .unwrap();
    let event_id = event.event.id.clone();
    left.add_shared_item(
        &event_id,
        SharedItemDraft {
            title: "taxi".into(),
            ..Default::default()
        },
    )
    .unwrap();
    super::tests::settle(&mut left, &mut right);
    let current = left.doc.shared_events[0].clone();
    let item = current.items[0].clone();
    // A first edit that syncs: a merged record no longer carries the stamp of
    // the whole `items` field, which would otherwise hide the loss below.
    left.update_shared_item(
        &event_id,
        &item.id,
        SharedItemDraft {
            title: "cab".into(),
            unit: item.unit.clone(),
            category_id: item.category_id.clone(),
            plan: item.plan.clone(),
            cancelled: item.cancelled,
        },
    )
    .unwrap();
    super::tests::settle(&mut left, &mut right);
    left.add_shared_purchase(
        &event_id,
        SharedPurchaseDraft {
            item_id: Some(item.id.clone()),
            buyer: current.participants[0].id.clone(),
            qty: amount("1"),
            amount: amount("10"),
            currency: "usd".into(),
            price_mode: PriceMode::Total,
            ..Default::default()
        },
    )
    .unwrap();
    // Later than the purchase, by a device that has not seen it.
    std::thread::sleep(std::time::Duration::from_millis(5));
    right
        .update_shared_item(
            &event_id,
            &item.id,
            SharedItemDraft {
                title: "taxi home".into(),
                unit: item.unit.clone(),
                category_id: item.category_id.clone(),
                plan: item.plan.clone(),
                cancelled: item.cancelled,
            },
        )
        .unwrap();
    super::tests::settle(&mut left, &mut right);
    assert_eq!(left.doc.shared_events[0].items[0].title, "taxi home");
    assert_eq!(right.doc.shared_events[0].items[0].title, "taxi home");
    (
        left.doc.shared_events[0].items[0].purchases.len(),
        right.doc.shared_events[0].items[0].purchases.len(),
    )
}

/// One device deletes a payment linked into an event, the other edits it
/// later. Returns whether the payment lives after they meet and how many
/// purchases its event then holds.
#[cfg(test)]
pub(super) fn payment_revived_after_its_deletion() -> (bool, usize) {
    let (_dir, mut left, mut right) = super::tests::two_devices();
    let store_id = left.doc.stores[0].id.clone();
    left.add_transaction(crate::service::test_support::income(
        &store_id, "100", "start",
    ))
    .unwrap();
    let paid = left
        .add_transaction(payment(&store_id, "10", "dinner"))
        .unwrap();
    let tx_id = paid.meta().id.clone();
    let event = left
        .create_shared_event(SharedEventDraft {
            title: "trip".into(),
            currency: "usd".into(),
            participants: vec![guest("guest".into())],
            ..Default::default()
        })
        .unwrap();
    let event_id = event.event.id.clone();
    left.attach_payment_to_shared_event(AttachPaymentInput {
        event_id: event_id.clone(),
        tx_id: tx_id.clone(),
        new_item_title: Some("dinner".into()),
        ..Default::default()
    })
    .unwrap();
    super::tests::settle(&mut left, &mut right);
    left.delete_transaction(&tx_id).unwrap();
    // Later than the deletion, by a device that has not seen it.
    std::thread::sleep(std::time::Duration::from_millis(5));
    let mut edited = right
        .doc
        .transactions
        .iter()
        .find(|tx| tx.meta().id == tx_id)
        .unwrap()
        .clone();
    edited.meta_mut().comment = "dinner with friends".into();
    right.update_transaction(&tx_id, edited).unwrap();
    super::tests::settle(&mut left, &mut right);
    let lives = left.doc.transactions.iter().any(|tx| tx.meta().id == tx_id);
    let purchases = left.doc.shared_events[0]
        .items
        .iter()
        .map(|item| item.purchases.len())
        .sum();
    (lives, purchases)
}

/// The deterministic form of the purchase loss: two devices add one purchase
/// each to the same item concurrently.
#[cfg(test)]
pub(super) fn two_purchases_to_one_item() -> usize {
    let (_dir, mut left, mut right) = super::tests::two_devices();
    let event = left
        .create_shared_event(SharedEventDraft {
            title: "trip".into(),
            currency: "usd".into(),
            participants: vec![guest("ann".into())],
            ..Default::default()
        })
        .unwrap();
    let event_id = event.event.id.clone();
    left.add_shared_item(
        &event_id,
        SharedItemDraft {
            title: "taxi".into(),
            ..Default::default()
        },
    )
    .unwrap();
    super::tests::settle(&mut left, &mut right);
    for (vault, price) in [(&mut left, "10"), (&mut right, "20")] {
        let current = vault.doc.shared_events[0].clone();
        vault
            .add_shared_purchase(
                &event_id,
                SharedPurchaseDraft {
                    item_id: Some(current.items[0].id.clone()),
                    buyer: current.participants[0].id.clone(),
                    qty: amount("1"),
                    amount: amount(price),
                    currency: "usd".into(),
                    price_mode: PriceMode::Total,
                    ..Default::default()
                },
            )
            .unwrap();
    }
    super::tests::settle(&mut left, &mut right);
    assert_eq!(
        left.doc.shared_events[0].items[0].purchases.len(),
        right.doc.shared_events[0].items[0].purchases.len(),
        "the devices agree even when a purchase is lost"
    );
    left.doc.shared_events[0].items[0].purchases.len()
}
