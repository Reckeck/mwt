//! Seeded randomized convergence: three devices edit at random, sync in
//! random (sometimes cut, one-directional) sessions, then exchange fully.
//! They must end with the same synced records, and nothing deleted may return.
//!
//! Replay a failure with `MWT_CONV_SEED=<seed> MWT_CONV_STEPS=<steps>`.

use super::convergence_catalog::{self as catalog, Item, KNOWN_OPEN, Violation};
use super::tests::two_devices;
use super::*;
use crate::ipc_contract::{
    SettleSharedInput, SharedEventDraft, SharedItemDraft, SharedParticipantDraft,
    SharedPurchaseDraft,
};
use crate::model::category::CategoryGroup;
use crate::model::contact::ContactKind;
use crate::model::inflation::InflationMode;
use crate::model::shared_event::{PriceMode, SettleDirection};
use crate::model::store::{Binding, StoreKind, SubType};
use crate::model::transaction::Transaction;
use crate::service::StoreExtras;
use crate::service::contacts::ContactDraft;
use crate::service::envelopes::EnvelopeDraft;
use crate::service::test_support::{amount, income, store, usd};
use std::collections::{BTreeMap, BTreeSet};

/// Edit kinds the harness may pick. Later stages (B3 slices) add kinds here:
/// envelope moves, exchange keys, adjustment lines.
const ENABLED_KINDS: &[Edit] = &[
    Edit::CreateTx,
    Edit::ChangeTxAmount,
    Edit::ChangeTxNote,
    Edit::DeleteTx,
    Edit::CreateCategory,
    Edit::RenameCategory,
    Edit::DeleteCategory,
    Edit::CreateContact,
    Edit::RenameContact,
    Edit::DeleteContact,
    Edit::SharedCreateEvent,
    Edit::SharedAddParticipant,
    Edit::SharedRemoveParticipant,
    Edit::SharedAddItem,
    Edit::SharedEditItem,
    Edit::SharedRemoveItem,
    Edit::SharedPurchase,
    Edit::SharedSettle,
    Edit::CreateSavings,
    Edit::DeleteSavings,
    Edit::ToggleEmergency,
    Edit::SetInflation,
    Edit::CreateEnvelope,
    Edit::FundEnvelope,
    Edit::MoveBetweenEnvelopes,
    Edit::DeleteEnvelope,
    Edit::CountBalance,
    Edit::RecountBalance,
    Edit::BackdatedTx,
    Edit::TerminalTimeframe,
    Edit::CarMileage,
    Edit::CarInsurance,
    Edit::AddExchangeKey,
    Edit::ChangeExchangeKey,
    Edit::RemoveExchangeKey,
    Edit::SetDebtTerms,
    Edit::AddMeterReading,
    Edit::AddValuation,
    Edit::MoveAccount,
    Edit::CreateGoal,
    Edit::DeleteGoal,
    Edit::FundGoal,
    Edit::CreatePayment,
    Edit::MakeGuestContact,
    Edit::AttachPayment,
    Edit::ChangeEventCurrency,
    Edit::SetAssetClasses,
    Edit::UndoLastBatch,
    Edit::SharedPurchaseToItem,
    Edit::FireAlert,
    Edit::ChangeAlertPrice,
    Edit::ConfirmImport,
    Edit::SetNoticeCategory,
    Edit::ToggleSystemNotices,
    Edit::OpenReminderList,
    Edit::TxWithBasePrice,
];

const DEFAULT_SEEDS: &[u64] = &[1, 2, 3, 4, 5];
const DEFAULT_STEPS: usize = 200;
const DEVICES: usize = 3;
const MAX_SETTLE_ROUNDS: usize = 10;

#[derive(Clone, Copy, Debug)]
pub(super) enum Edit {
    CreateTx,
    ChangeTxAmount,
    ChangeTxNote,
    DeleteTx,
    CreateCategory,
    RenameCategory,
    DeleteCategory,
    CreateContact,
    RenameContact,
    DeleteContact,
    SharedCreateEvent,
    SharedAddParticipant,
    SharedRemoveParticipant,
    SharedAddItem,
    SharedEditItem,
    SharedRemoveItem,
    SharedPurchase,
    SharedSettle,
    CreateSavings,
    DeleteSavings,
    ToggleEmergency,
    SetInflation,
    CreateEnvelope,
    FundEnvelope,
    MoveBetweenEnvelopes,
    DeleteEnvelope,
    CountBalance,
    RecountBalance,
    BackdatedTx,
    TerminalTimeframe,
    CarMileage,
    CarInsurance,
    AddExchangeKey,
    ChangeExchangeKey,
    RemoveExchangeKey,
    // Stage M: keys and links after a merge (`convergence_catalog.rs`).
    SetDebtTerms,
    AddMeterReading,
    AddValuation,
    MoveAccount,
    CreateGoal,
    DeleteGoal,
    FundGoal,
    CreatePayment,
    MakeGuestContact,
    AttachPayment,
    ChangeEventCurrency,
    SetAssetClasses,
    UndoLastBatch,
    SharedPurchaseToItem,
    FireAlert,
    ChangeAlertPrice,
    ConfirmImport,
    // Stage P: where a setting lives.
    SetNoticeCategory,
    ToggleSystemNotices,
    OpenReminderList,
    TxWithBasePrice,
}

/// splitmix64: tiny, deterministic, good enough for choosing steps.
pub(super) struct Rng(u64);

impl Rng {
    pub(super) fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut mixed = self.0;
        mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        mixed ^ (mixed >> 31)
    }

    pub(super) fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }
}

fn peer_of(index: usize) -> PeerRef {
    PeerRef {
        device_id: format!("device-{index}-id"),
        name: format!("device-{index}"),
    }
}

/// How the devices begin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Start {
    /// One vault copied to every device: they share the Cash wallet.
    Copied,
    /// Every device made its own vault, so each has a Cash wallet of its own.
    Separate,
}

fn three_devices(start: Start) -> (Vec<tempfile::TempDir>, Vec<UnlockedVault>) {
    match start {
        Start::Copied => {
            let (dir, left, right) = two_devices();
            let copy = dir.path().join("third.wallet");
            std::fs::copy(&left.path, &copy).unwrap();
            let mut third = UnlockedVault::open(copy, "pw").unwrap();
            third.set_device("cccccccc-third");
            (vec![dir], vec![left, right, third])
        }
        Start::Separate => {
            let mut dirs = Vec::new();
            let mut devices = Vec::new();
            for name in ["aaaaaaaa-left", "bbbbbbbb-right", "cccccccc-third"] {
                let (dir, mut vault) = crate::service::test_support::empty_vault();
                vault.set_device(name);
                vault.persist().unwrap();
                dirs.push(dir);
                devices.push(vault);
            }
            (dirs, devices)
        }
    }
}

/// `from` sends what `to` lacks; only `to` changes. Whether `to` applied.
pub(super) fn pull(devices: &mut [UnlockedVault], from: usize, to: usize) -> bool {
    let from_manifest = devices[from].sync_manifest(None).unwrap();
    let to_manifest = devices[to].sync_manifest(None).unwrap();
    let (wants, _) = devices[to].sync_diff(&to_manifest, &from_manifest);
    let records = devices[from].sync_records(&wants).unwrap();
    devices[to]
        .sync_apply(&records, &peer_of(from))
        .unwrap()
        .is_some()
}

#[derive(Default)]
pub(super) struct Ledger {
    /// Full keys deleted on some device.
    pub(super) deleted: BTreeSet<String>,
    /// Full keys created or edited after being deleted (exempt from the check).
    pub(super) touched_after: BTreeSet<String>,
    counter: usize,
    /// The exchange and the car every device starts with (see `add_shared_stores`).
    pub(super) exchange_id: String,
    /// Exchange wallets (the first is `exchange_id`) the key edits pick from.
    key_stores: Vec<String>,
    car_id: String,
    /// Property, its flat asset key and a metered utility (`add_catalog_stores`).
    pub(super) property_id: String,
    pub(super) thing_key: String,
    pub(super) utility_id: String,
    /// Purchases added to shared items: (event, item, purchase).
    pub(super) purchases: Vec<(String, String, String)>,
    /// Two contacts that debt terms pile onto (`add_catalog_stores`).
    pub(super) anchors: Vec<String>,
    /// Category lead times written through the whole-object form (stage P, 121).
    pub(super) notice_writes: Vec<(crate::model::notify::NoticeCategory, u16)>,
    /// The per-device notification switches each device set last, by vault path.
    pub(super) device_flags: BTreeMap<std::path::PathBuf, (bool, bool)>,
    /// Incomes with a price in the author's base: (tx id, price in USD).
    pub(super) base_priced: Vec<(String, crate::Amount)>,
}

impl Ledger {
    pub(super) fn fresh(&mut self) -> usize {
        self.counter += 1;
        self.counter
    }
}

fn contact_draft(name: String) -> ContactDraft {
    ContactDraft {
        name,
        kind: ContactKind::Person,
        note: String::new(),
        accounts: Vec::new(),
        group: String::new(),
    }
}

pub(super) fn pick<'items, Item>(rng: &mut Rng, items: &'items [Item]) -> Option<&'items Item> {
    if items.is_empty() {
        None
    } else {
        Some(&items[rng.below(items.len())])
    }
}

fn apply_edit(kind: Edit, vault: &mut UnlockedVault, rng: &mut Rng, ledger: &mut Ledger) {
    match kind {
        Edit::CreateTx => {
            let store = vault.doc.stores[0].id.clone();
            let amount = format!("{}", 1 + rng.below(1000));
            vault
                .add_transaction(income(&store, &amount, "conv"))
                .unwrap();
        }
        Edit::ChangeTxAmount | Edit::ChangeTxNote => {
            let ids: Vec<String> = vault
                .doc
                .transactions
                .iter()
                .map(|tx| tx.meta().id.clone())
                .collect();
            let Some(id) = pick(rng, &ids).cloned() else {
                return;
            };
            let mut tx: Transaction = vault
                .doc
                .transactions
                .iter()
                .find(|tx| tx.meta().id == id)
                .unwrap()
                .clone();
            let number = ledger.fresh();
            if let (Edit::ChangeTxAmount, Transaction::Topup(topup)) = (kind, &mut tx) {
                topup.amount = crate::Amount::parse(&format!("{}", 1 + rng.below(1000))).unwrap();
            } else {
                tx.meta_mut().comment = format!("note {number}");
            }
            vault.update_transaction(&id, tx).unwrap();
            ledger.touched_after.insert(format!("transactions:{id}"));
        }
        Edit::DeleteTx => {
            let ids: Vec<String> = vault
                .doc
                .transactions
                .iter()
                .map(|tx| tx.meta().id.clone())
                .collect();
            let Some(id) = pick(rng, &ids).cloned() else {
                return;
            };
            vault.delete_transaction(&id).unwrap();
            ledger.deleted.insert(format!("transactions:{id}"));
        }
        Edit::CreateCategory => {
            let number = ledger.fresh();
            vault
                .create_category(None, &format!("cat{number}"), CategoryGroup::Daily)
                .unwrap();
        }
        Edit::RenameCategory => {
            let keys = created_categories(vault);
            let Some(key) = pick(rng, &keys).cloned() else {
                return;
            };
            let number = ledger.fresh();
            vault
                .update_category(
                    &key,
                    Some(&format!("renamed{number}")),
                    CategoryGroup::Daily,
                    None,
                    None,
                )
                .unwrap();
            ledger.touched_after.insert(format!("categories:{key}"));
        }
        Edit::DeleteCategory => {
            let keys = created_categories(vault);
            let Some(key) = pick(rng, &keys).cloned() else {
                return;
            };
            vault.delete_category(&key).unwrap();
            ledger.deleted.insert(format!("categories:{key}"));
        }
        Edit::CreateContact => {
            let number = ledger.fresh();
            vault
                .create_contact(contact_draft(format!("contact {number}")))
                .unwrap();
        }
        Edit::RenameContact => {
            let ids: Vec<String> = vault
                .doc
                .contacts
                .iter()
                .map(|contact| contact.id.clone())
                .collect();
            let Some(id) = pick(rng, &ids).cloned() else {
                return;
            };
            let number = ledger.fresh();
            vault
                .update_contact(&id, contact_draft(format!("renamed contact {number}")))
                .unwrap();
            ledger.touched_after.insert(format!("contacts:{id}"));
        }
        Edit::SharedCreateEvent
        | Edit::SharedAddParticipant
        | Edit::SharedRemoveParticipant
        | Edit::SharedAddItem
        | Edit::SharedEditItem
        | Edit::SharedRemoveItem
        | Edit::SharedPurchase
        | Edit::SharedSettle => apply_shared(kind, vault, rng, ledger),
        Edit::CreateSavings | Edit::DeleteSavings | Edit::ToggleEmergency | Edit::SetInflation => {
            apply_fund(kind, vault, rng, ledger);
        }
        Edit::CreateEnvelope
        | Edit::FundEnvelope
        | Edit::MoveBetweenEnvelopes
        | Edit::DeleteEnvelope => apply_envelope(kind, vault, rng, ledger),
        Edit::CountBalance | Edit::RecountBalance | Edit::BackdatedTx => {
            apply_adjustment(kind, vault, rng, ledger);
        }
        Edit::AddExchangeKey | Edit::ChangeExchangeKey | Edit::RemoveExchangeKey => {
            apply_exchange_key(kind, vault, rng, ledger);
        }
        Edit::TerminalTimeframe | Edit::CarMileage | Edit::CarInsurance => {
            apply_device_and_car(kind, vault, rng, ledger);
        }
        Edit::SetDebtTerms
        | Edit::AddMeterReading
        | Edit::AddValuation
        | Edit::MoveAccount
        | Edit::CreateGoal
        | Edit::DeleteGoal
        | Edit::FundGoal
        | Edit::CreatePayment
        | Edit::MakeGuestContact
        | Edit::AttachPayment
        | Edit::ChangeEventCurrency
        | Edit::SetAssetClasses
        | Edit::UndoLastBatch
        | Edit::SharedPurchaseToItem
        | Edit::FireAlert
        | Edit::ChangeAlertPrice
        | Edit::ConfirmImport
        | Edit::SetNoticeCategory
        | Edit::ToggleSystemNotices
        | Edit::OpenReminderList
        | Edit::TxWithBasePrice => catalog::apply_catalog_edit(kind, vault, rng, ledger),
        Edit::DeleteContact => {
            // The anchors stay: debt terms of the catalog edits pile onto them.
            let ids: Vec<String> = vault
                .doc
                .contacts
                .iter()
                .filter(|contact| !ledger.anchors.contains(&contact.id))
                .map(|contact| contact.id.clone())
                .collect();
            let Some(id) = pick(rng, &ids).cloned() else {
                return;
            };
            // A guest linked to an event refuses the deletion: nothing happened.
            if vault.delete_contact(&id).is_err() {
                return;
            }
            ledger.deleted.insert(format!("contacts:{id}"));
        }
    }
}

pub(super) fn event_ids(vault: &UnlockedVault) -> Vec<String> {
    vault
        .doc
        .shared_events
        .iter()
        .map(|event| event.id.clone())
        .collect()
}

pub(super) fn guest(name: String) -> SharedParticipantDraft {
    SharedParticipantDraft {
        name,
        ..Default::default()
    }
}

/// Shared-event edits. A refusal of the core (a busy participant, a
/// settlement over the debt) is a normal outcome of a random edit.
fn apply_shared(kind: Edit, vault: &mut UnlockedVault, rng: &mut Rng, ledger: &mut Ledger) {
    let number = ledger.fresh();
    let Some(event) = pick(rng, &event_ids(vault)).cloned() else {
        if matches!(kind, Edit::SharedCreateEvent | Edit::SharedAddParticipant) {
            vault
                .create_shared_event(SharedEventDraft {
                    title: format!("event {number}"),
                    currency: "usd".into(),
                    participants: vec![guest(format!("guest {number}"))],
                    ..Default::default()
                })
                .unwrap();
        }
        return;
    };
    let current = vault
        .doc
        .shared_events
        .iter()
        .find(|candidate| candidate.id == event)
        .unwrap()
        .clone();
    match kind {
        Edit::SharedCreateEvent => {
            vault
                .create_shared_event(SharedEventDraft {
                    title: format!("event {number}"),
                    currency: "usd".into(),
                    participants: vec![guest(format!("guest {number}"))],
                    ..Default::default()
                })
                .unwrap();
        }
        Edit::SharedAddParticipant => {
            vault
                .add_shared_participant(&event, guest(format!("guest {number}")))
                .unwrap();
        }
        Edit::SharedRemoveParticipant => {
            let Some(person) = pick(rng, &current.participants).cloned() else {
                return;
            };
            if vault.remove_shared_participant(&event, &person.id).is_ok() {
                ledger
                    .deleted
                    .insert(format!("{event}/participant/{}", person.id));
            }
        }
        Edit::SharedAddItem => {
            vault
                .add_shared_item(
                    &event,
                    SharedItemDraft {
                        title: format!("item {number}"),
                        ..Default::default()
                    },
                )
                .unwrap();
        }
        Edit::SharedEditItem => {
            let Some(item) = pick(rng, &current.items).cloned() else {
                return;
            };
            let draft = SharedItemDraft {
                title: format!("edited {number}"),
                unit: item.unit.clone(),
                category_id: item.category_id.clone(),
                plan: item.plan.clone(),
                cancelled: item.cancelled,
            };
            if vault.update_shared_item(&event, &item.id, draft).is_ok() {
                ledger
                    .touched_after
                    .insert(format!("{event}/item/{}", item.id));
            }
        }
        Edit::SharedRemoveItem => {
            let Some(item) = pick(rng, &current.items).cloned() else {
                return;
            };
            if vault.delete_shared_item(&event, &item.id).is_ok() {
                ledger.deleted.insert(format!("{event}/item/{}", item.id));
            }
        }
        Edit::SharedPurchase => {
            let Some(person) = pick(rng, &current.participants).cloned() else {
                return;
            };
            let _ = vault.add_shared_purchase(
                &event,
                SharedPurchaseDraft {
                    new_item_title: Some(format!("bought {number}")),
                    buyer: person.id,
                    qty: amount("1"),
                    amount: amount(&format!("{}", 10 + rng.below(90))),
                    currency: "usd".into(),
                    price_mode: PriceMode::Total,
                    ..Default::default()
                },
            );
        }
        Edit::SharedSettle => {
            let Some(person) = pick(rng, &current.participants).cloned() else {
                return;
            };
            for direction in [SettleDirection::ToOrganizer, SettleDirection::FromOrganizer] {
                let settled = vault.settle_shared(SettleSharedInput {
                    event_id: event.clone(),
                    participant_id: person.id.clone(),
                    direction,
                    amount: amount("1"),
                    store_id: None,
                    at: None,
                });
                if settled.is_ok() {
                    break;
                }
            }
        }
        _ => {}
    }
}

fn envelope_ids(vault: &UnlockedVault) -> Vec<String> {
    vault
        .doc
        .envelopes
        .iter()
        .map(|envelope| envelope.id.clone())
        .collect()
}

/// Envelope journal edits. A refusal (nothing free, nothing to release) is a
/// normal outcome of a random edit.
fn apply_envelope(kind: Edit, vault: &mut UnlockedVault, rng: &mut Rng, ledger: &mut Ledger) {
    let number = ledger.fresh();
    let ids = envelope_ids(vault);
    match kind {
        Edit::CreateEnvelope => {
            let store_id = vault.doc.stores[0].id.clone();
            let _ = vault.create_envelope(EnvelopeDraft {
                store_id,
                name: format!("envelope {number}"),
                asset: usd(),
                target: None,
                goal_id: None,
            });
        }
        Edit::FundEnvelope => {
            let Some(id) = pick(rng, &ids).cloned() else {
                return;
            };
            // Mostly top-ups, sometimes a release.
            let sign = if rng.below(4) == 0 { "-" } else { "" };
            let by = amount(&format!("{sign}{}", 1 + rng.below(20)));
            if vault.fund_envelope(&id, by).is_ok() {
                ledger.touched_after.insert(format!("envelopes:{id}"));
            }
        }
        Edit::MoveBetweenEnvelopes => {
            let (Some(from), Some(to)) = (pick(rng, &ids).cloned(), pick(rng, &ids).cloned())
            else {
                return;
            };
            let by = amount(&format!("{}", 1 + rng.below(10)));
            if from != to && vault.move_between_envelopes(&from, &to, by).is_ok() {
                ledger.touched_after.insert(format!("envelopes:{from}"));
                ledger.touched_after.insert(format!("envelopes:{to}"));
            }
        }
        Edit::DeleteEnvelope => {
            let Some(id) = pick(rng, &ids).cloned() else {
                return;
            };
            if vault.delete_envelope(&id).is_ok() {
                ledger.deleted.insert(format!("envelopes:{id}"));
            }
        }
        _ => {}
    }
}

fn savings_ids(vault: &UnlockedVault) -> Vec<String> {
    vault
        .doc
        .stores
        .iter()
        .filter(|candidate| candidate.name.starts_with("sv"))
        .map(|candidate| candidate.id.clone())
        .collect()
}

/// Emergency-fund scope and per-currency inflation modes.
fn apply_fund(kind: Edit, vault: &mut UnlockedVault, rng: &mut Rng, ledger: &mut Ledger) {
    let number = ledger.fresh();
    match kind {
        Edit::CreateSavings => {
            let bank = store(vault, &format!("bank {number}"), StoreKind::Bank, None);
            let extras = StoreExtras {
                sub_type: Some(SubType::Savings),
                binding: Some(Binding::Asset(usd())),
                ..StoreExtras::default()
            };
            vault
                .create_store_with(
                    &format!("sv{number}"),
                    StoreKind::Bank,
                    "#000",
                    "",
                    Some(bank),
                    extras,
                )
                .unwrap();
        }
        Edit::DeleteSavings => {
            let Some(id) = pick(rng, &savings_ids(vault)).cloned() else {
                return;
            };
            if vault.delete_store(&id).is_ok() {
                ledger.deleted.insert(format!("stores:{id}"));
            }
        }
        Edit::ToggleEmergency => {
            let Some(id) = pick(rng, &savings_ids(vault)).cloned() else {
                return;
            };
            let on = rng.below(3) != 0;
            vault.emergency_fund_toggle(&id, on).unwrap();
        }
        Edit::SetInflation => {
            let currency = ["usd", "eur", "thb"][rng.below(3)];
            let mode = (rng.below(4) != 0).then_some(InflationMode::Manual);
            vault.set_inflation_mode(currency, mode).unwrap();
        }
        _ => {}
    }
}

/// Categories made by this harness (names `cat<N>` / `renamed<N>` keys start with `cat`).
/// A few fixed moments, so corrections and backdated operations of different
/// devices land in the same second now and then.
const MOMENTS: &[&str] = &[
    "2026-03-01T10:00:00Z",
    "2026-03-02T10:00:00Z",
    "2026-03-03T10:00:00Z",
];

fn adjustment_ids(vault: &UnlockedVault) -> Vec<String> {
    vault
        .doc
        .transactions
        .iter()
        .filter(|tx| matches!(tx, Transaction::Adjustment(_)))
        .map(|tx| tx.meta().id.clone())
        .collect()
}

fn apply_adjustment(kind: Edit, vault: &mut UnlockedVault, rng: &mut Rng, ledger: &mut Ledger) {
    let store_id = vault.doc.stores[0].id.clone();
    match kind {
        Edit::CountBalance => {
            let mut meta = crate::model::transaction::TxMeta::new("count");
            // The fixed moments make two corrections of one account land in
            // one second now and then: they are ordered by creation stamp.
            meta.timestamp = MOMENTS[rng.below(MOMENTS.len())].into();
            let tx = Transaction::Adjustment(crate::model::transaction::AdjustmentTx {
                meta,
                store_id,
                lines: vec![crate::model::transaction::AdjustmentLine {
                    asset: usd(),
                    before: crate::Amount::ZERO,
                    after: amount(&format!("{}", 1 + rng.below(1000))),
                    price: Default::default(),
                }],
                fee: None,
            });
            // Nothing to correct when the balance already matches: not an error here.
            let _ = vault.add_transaction(tx);
        }
        Edit::RecountBalance => {
            let ids = adjustment_ids(vault);
            let Some(id) = pick(rng, &ids).cloned() else {
                return;
            };
            let mut tx = vault
                .doc
                .transactions
                .iter()
                .find(|tx| tx.meta().id == id)
                .unwrap()
                .clone();
            if let Transaction::Adjustment(inner) = &mut tx {
                inner.lines[0].after = amount(&format!("{}", 1 + rng.below(1000)));
            }
            let _ = vault.update_transaction(&id, tx);
            ledger.touched_after.insert(format!("transactions:{id}"));
        }
        _ => {
            let mut tx = income(&store_id, &format!("{}", 1 + rng.below(1000)), "late");
            tx.meta_mut().timestamp = MOMENTS[rng.below(MOMENTS.len())].into();
            vault.add_transaction(tx).unwrap();
        }
    }
}

/// An exchange (its terminal is device-local, its import settings sync) and a
/// car (mileage and insurance are separate fields), made on device 0 and
/// pulled by the others before the random edits start.
fn add_shared_stores(devices: &mut [UnlockedVault], ledger: &mut Ledger) {
    ledger.exchange_id = devices[0]
        .create_store_with(
            "conv exchange",
            StoreKind::Exchange,
            "",
            "",
            None,
            StoreExtras {
                provider: Some(crate::model::store::ExchangeProvider::Bybit),
                ..StoreExtras::default()
            },
        )
        .unwrap()
        .id;
    ledger.key_stores.push(ledger.exchange_id.clone());
    for number in 0..2 {
        let id = devices[0]
            .create_store_with(
                &format!("conv key exchange {number}"),
                StoreKind::Exchange,
                "",
                "",
                None,
                StoreExtras {
                    provider: Some(crate::model::store::ExchangeProvider::Bybit),
                    ..StoreExtras::default()
                },
            )
            .unwrap()
            .id;
        ledger.key_stores.push(id);
    }
    ledger.car_id = devices[0]
        .create_store_with(
            "conv car",
            StoreKind::Property,
            "#555",
            "",
            None,
            StoreExtras {
                property: Some(crate::model::store::PropertyInfo {
                    category: crate::model::store::PropertyCategory::Car,
                    car: Some(crate::model::store::CarInfo::default()),
                    ..Default::default()
                }),
                ..StoreExtras::default()
            },
        )
        .unwrap()
        .id;
    for to in 1..DEVICES {
        pull(devices, 0, to);
    }
}

/// Exchange API keys (dummy strings): add or replace, retarget, clear. The
/// clear mirrors `clear_exchange_key` in the app crate.
fn apply_exchange_key(kind: Edit, vault: &mut UnlockedVault, rng: &mut Rng, ledger: &mut Ledger) {
    let store = ledger.key_stores[rng.below(ledger.key_stores.len())].clone();
    let number = ledger.fresh();
    match kind {
        Edit::AddExchangeKey => {
            vault
                .set_exchange_key(
                    &store,
                    &format!("dummy-key-{number}"),
                    &format!("dummy-secret-{number}"),
                )
                .unwrap();
        }
        Edit::ChangeExchangeKey => {
            if !vault
                .doc
                .exchanges
                .bybit
                .iter()
                .any(|entry| entry.store_id == store)
            {
                return;
            }
            if rng.below(2) == 0 {
                let target = ledger.key_stores[rng.below(ledger.key_stores.len())].clone();
                let spot = (rng.below(2) == 0).then(|| target.clone());
                let futures = (rng.below(2) == 0).then_some(target);
                // Targets must sit inside the wallet; a refusal is a legal no-op.
                let _ = vault.set_exchange_targets(&store, spot, futures);
            } else {
                vault
                    .set_exchange_key(
                        &store,
                        &format!("dummy-key-{number}"),
                        &format!("dummy-secret-{number}"),
                    )
                    .unwrap();
            }
        }
        _ => {
            vault
                .doc
                .exchanges
                .bybit
                .retain(|entry| entry.store_id != store);
            vault.persist().unwrap();
        }
    }
}

const TIMEFRAMES: &[&str] = &["1m", "15m", "1h", "4h", "1d"];
const INSURED_UNTIL: &[&str] = &["2027-01-01", "2027-02-01", "2027-03-01"];

/// The terminal edit is device-local: it must never reach another device.
fn apply_device_and_car(kind: Edit, vault: &mut UnlockedVault, rng: &mut Rng, ledger: &mut Ledger) {
    match kind {
        Edit::TerminalTimeframe => {
            let patch = crate::service::TerminalPatch {
                timeframe: Some(TIMEFRAMES[rng.below(TIMEFRAMES.len())].into()),
                ..Default::default()
            };
            vault
                .update_terminal_state(&ledger.exchange_id, patch)
                .unwrap();
        }
        Edit::CarMileage => {
            let day = chrono::NaiveDate::from_ymd_opt(2026, 3, 1 + rng.below(28) as u32).unwrap();
            vault
                .vehicle_update_mileage(&ledger.car_id, 1000 + rng.below(90_000) as u32, day)
                .unwrap();
        }
        _ => {
            let until = INSURED_UNTIL[rng.below(INSURED_UNTIL.len())];
            for store in &mut vault.doc.stores {
                if store.id == ledger.car_id
                    && let Some(car) = store.property.as_mut().and_then(|info| info.car.as_mut())
                {
                    car.osago_until = Some(until.into());
                }
            }
            vault.persist().unwrap();
        }
    }
}

fn created_categories(vault: &UnlockedVault) -> Vec<String> {
    vault
        .doc
        .categories
        .iter()
        .filter(|category| category.key.starts_with("cat"))
        .map(|category| category.key.clone())
        .collect()
}

fn view_of(vault: &mut UnlockedVault) -> BTreeMap<String, Value> {
    let value = vault.doc_value().unwrap();
    view::records(&value)
}

/// Runs one seed. Convergence and the old checks are asserted at once; the
/// catalog invariants are returned, so a caller decides which are tolerated.
fn run(seed: u64, steps: usize, start: Start) -> Vec<Violation> {
    let context = format!(
        "seed={seed} steps={steps} start={start:?} (MWT_CONV_SEED={seed} MWT_CONV_STEPS={steps})"
    );
    let (_dirs, mut devices) = three_devices(start);
    let mut rng = Rng(seed);
    let mut ledger = Ledger::default();
    // Device 1 is the Russian one with the ruble base, the others use dollars.
    devices[1].set_base_currency("rub");
    for device in &mut devices {
        catalog::seed_rub_rate(device);
    }
    let trace = std::env::var("MWT_CONV_TRACE").is_ok();
    add_shared_stores(&mut devices, &mut ledger);
    catalog::add_catalog_stores(&mut devices, &mut ledger);

    for _ in 0..steps {
        if rng.below(10) < 6 {
            let device = rng.below(DEVICES);
            let kind = ENABLED_KINDS[rng.below(ENABLED_KINDS.len())];
            if trace {
                eprintln!("edit {kind:?} on {device}");
            }
            apply_edit(kind, &mut devices[device], &mut rng, &mut ledger);
        } else {
            let from = rng.below(DEVICES);
            let to = (from + 1 + rng.below(DEVICES - 1)) % DEVICES;
            if trace {
                eprintln!("pull {from} -> {to}");
            }
            pull(&mut devices, from, to);
            if rng.below(2) == 0 {
                if trace {
                    eprintln!("pull {to} -> {from}");
                }
                pull(&mut devices, to, from);
            }
        }
    }

    let mut settled = false;
    for _ in 0..MAX_SETTLE_ROUNDS {
        let mut changed = false;
        for from in 0..DEVICES {
            for to in 0..DEVICES {
                if from != to {
                    changed |= pull(&mut devices, from, to);
                }
            }
        }
        if !changed {
            settled = true;
            break;
        }
    }
    assert!(settled, "{context}: devices did not settle");

    let views: Vec<BTreeMap<String, Value>> = devices.iter_mut().map(view_of).collect();
    for (index, other) in views.iter().enumerate().skip(1) {
        let first = &views[0];
        let keys: BTreeSet<&String> = first.keys().chain(other.keys()).collect();
        for key in keys {
            assert!(
                first.get(key) == other.get(key),
                "{context}: record {key} differs between device 0 and device {index}:\n  0: {:?}\n  {index}: {:?}",
                first.get(key),
                other.get(key)
            );
        }
    }

    for key in ledger.deleted.difference(&ledger.touched_after) {
        for (index, view) in views.iter().enumerate() {
            // Rule R3: a record that live money names is put back on purpose.
            if devices[index].money_names_record(key) {
                continue;
            }
            assert!(
                !view.contains_key(key),
                "{context}: deleted record {key} came back on device {index}"
            );
        }
        // `<event>/item/<id>` and `<event>/participant/<id>`: entries inside a record.
        if let Some((event, rest)) = key.split_once('/') {
            for (index, device) in devices.iter().enumerate() {
                let Some(found) = device
                    .doc
                    .shared_events
                    .iter()
                    .find(|found| found.id == event)
                else {
                    continue;
                };
                if rest
                    .split_once('/')
                    .is_some_and(|(_, id)| device.event_names_participant(event, id))
                {
                    continue;
                }
                let present = match rest.split_once('/') {
                    Some(("item", id)) => found.items.iter().any(|item| item.id == id),
                    Some((_, id)) => found.participants.iter().any(|person| person.id == id),
                    None => false,
                };
                assert!(
                    !present,
                    "{context}: removed entry {key} came back on device {index}"
                );
            }
        }
    }
    catalog::check_alerts_agree(&devices, &context);
    for (index, device) in devices.iter().enumerate() {
        for envelope in &device.doc.envelopes {
            assert_eq!(
                envelope.allocated,
                envelope.moves_sum(),
                "{context}: envelope {} is not the sum of its moves on device {index}",
                envelope.id
            );
            assert!(
                !envelope
                    .moves
                    .iter()
                    .any(|entry| entry.id.starts_with("fix:")),
                "{context}: a correction appeared between new devices on device {index}"
            );
        }
    }
    for (index, device) in devices.iter().enumerate() {
        for id in &device.doc.emergency_fund.scope.store_ids {
            assert!(
                device.find_store(id).is_some(),
                "{context}: the emergency fund names a missing store {id} on device {index}"
            );
        }
    }
    if std::env::var("MWT_CONV_STATS").is_ok() {
        let doc = &devices[0].doc;
        eprintln!(
            "stats seed={seed} {start:?}: debt_terms={} readings={} valuations={} goals={} goal_envelopes={} envelopes={} cash={} sv={} bank={} events={} contacts={} classes={} links={} purchases={}",
            doc.debt_terms.len(),
            doc.meter_readings.len(),
            doc.valuations.len(),
            doc.goals.len(),
            doc.envelopes
                .iter()
                .filter(|envelope| envelope.goal_id.is_some())
                .count(),
            doc.envelopes.len(),
            doc.stores
                .iter()
                .filter(|node| node.kind == StoreKind::Cash)
                .count(),
            doc.stores
                .iter()
                .filter(|node| node.name.starts_with("sv"))
                .count(),
            doc.stores
                .iter()
                .filter(|node| node.name.starts_with("bank"))
                .count(),
            doc.shared_events.len(),
            doc.contacts.len(),
            doc.asset_classes.len(),
            devices[0].shared_tx_links().len(),
            ledger.purchases.len(),
        );
    }
    let mut violations = Vec::new();
    for (index, device) in devices.iter().enumerate() {
        for mut violation in catalog::invariants(device, &ledger) {
            violation.text = format!("device {index}: {}", violation.text);
            violations.push(violation);
        }
    }
    violations.extend(catalog::reminder_text_ping_pong(&mut devices));
    violations
}

/// `run`, failing on every violation of an item that is not in `open`.
fn run_tolerating(seed: u64, steps: usize, start: Start, open: &[Item]) {
    let violations = run(seed, steps, start);
    let fresh: Vec<String> = violations
        .iter()
        .filter(|violation| !open.contains(&violation.item))
        .map(|violation| format!("{:?}: {}", violation.item, violation.text))
        .collect();
    assert!(
        fresh.is_empty(),
        "seed={seed} steps={steps} start={start:?}: invariants broken:\n  {}",
        fresh.join("\n  ")
    );
}

fn env_number<Number: std::str::FromStr>(name: &str) -> Option<Number> {
    std::env::var(name).ok().and_then(|raw| raw.parse().ok())
}

fn seeds_from_env() -> Vec<u64> {
    match env_number("MWT_CONV_SEED") {
        Some(seed) => vec![seed],
        None => DEFAULT_SEEDS.to_vec(),
    }
}

#[test]
fn random_edits_and_cut_sessions_converge() {
    let steps = env_number("MWT_CONV_STEPS").unwrap_or(DEFAULT_STEPS);
    for seed in seeds_from_env() {
        run_tolerating(seed, steps, Start::Copied, KNOWN_OPEN);
    }
}

#[test]
fn separately_created_devices_converge() {
    let steps = env_number("MWT_CONV_STEPS").unwrap_or(DEFAULT_STEPS);
    for seed in seeds_from_env() {
        run_tolerating(seed, steps, Start::Separate, KNOWN_OPEN);
    }
}

#[test]
fn convergence_quick_fixed_seed() {
    run_tolerating(42, 50, Start::Copied, KNOWN_OPEN);
}

/// Guard: the same runs with no item tolerated (the strict form of the tests
/// above, with a per-item report). Red on purpose while `KNOWN_OPEN` lists
/// the stage P items, so it is ignored until the last of them is fixed; run it
/// with `cargo test -p trade-core random_edits_with_no_item -- --ignored`.
#[test]
#[ignore = "red on purpose until the stage P fixes (121, 33, 26) land; shows which invariants break"]
fn random_edits_with_no_item_tolerated_stay_green() {
    let steps = env_number("MWT_CONV_STEPS").unwrap_or(DEFAULT_STEPS);
    let mut report = Vec::new();
    for start in [Start::Copied, Start::Separate] {
        for seed in seeds_from_env() {
            let mut by_item: BTreeMap<Item, usize> = BTreeMap::new();
            for violation in run(seed, steps, start) {
                eprintln!(
                    "seed={seed} {start:?} {:?}: {}",
                    violation.item, violation.text
                );
                *by_item.entry(violation.item).or_default() += 1;
            }
            if !by_item.is_empty() {
                report.push(format!("seed={seed} start={start:?}: {by_item:?}"));
            }
        }
    }
    assert!(
        report.is_empty(),
        "invariants broken:\n  {}",
        report.join("\n  ")
    );
}

/// Two devices each add a purchase to the same item before they meet.
#[test]
fn parallel_purchases_to_one_item_both_survive() {
    assert_eq!(catalog::two_purchases_to_one_item(), 2);
}

fn violations_of(items: &[Item], vault: &UnlockedVault) -> Vec<String> {
    catalog::invariants(vault, &Ledger::default())
        .into_iter()
        .filter(|violation| items.contains(&violation.item))
        .map(|violation| violation.text)
        .collect()
}

/// One device moves an account under a wallet, the other deletes that
/// (empty) wallet: the account is left under a missing parent.
#[test]
fn move_beside_delete_keeps_the_account_tree_valid() {
    use super::tests::settle;
    let (_dir, mut left, mut right) = two_devices();
    let first = store(&mut left, "bank 1", StoreKind::Bank, None);
    let second = store(&mut left, "bank 2", StoreKind::Bank, None);
    let savings = left
        .create_store_with(
            "sv1",
            StoreKind::Bank,
            "#000",
            "",
            Some(first),
            StoreExtras {
                sub_type: Some(SubType::Savings),
                binding: Some(Binding::Asset(usd())),
                ..StoreExtras::default()
            },
        )
        .unwrap()
        .id;
    settle(&mut left, &mut right);
    left.move_store(&savings, Some(&second)).unwrap();
    right.delete_store(&second).unwrap();
    settle(&mut left, &mut right);
    for device in [&left, &right] {
        assert_eq!(
            violations_of(&[Item::AccountTree25], device),
            Vec::<String>::new()
        );
    }
}

/// One device changes the currency of a bare event, the other buys in it.
#[test]
fn currency_change_beside_a_purchase_keeps_the_rates_valid() {
    use super::tests::settle;
    let (_dir, mut left, mut right) = two_devices();
    let event = left
        .create_shared_event(SharedEventDraft {
            title: "trip".into(),
            currency: "usd".into(),
            participants: vec![guest("ann".into())],
            ..Default::default()
        })
        .unwrap()
        .event;
    settle(&mut left, &mut right);
    left.update_shared_event(
        &event.id,
        SharedEventDraft {
            title: event.title.clone(),
            currency: "eur".into(),
            rounding_step: Some(event.rounding_step),
            organizer: event.organizer.clone(),
            i_participate: event.i_participate,
            ..Default::default()
        },
    )
    .unwrap();
    let buyer = right.doc.shared_events[0].participants[0].id.clone();
    right
        .add_shared_purchase(
            &event.id,
            SharedPurchaseDraft {
                new_item_title: Some("taxi".into()),
                buyer,
                qty: amount("1"),
                amount: amount("10"),
                currency: "usd".into(),
                price_mode: PriceMode::Total,
                ..Default::default()
            },
        )
        .unwrap();
    settle(&mut left, &mut right);
    for device in [&left, &right] {
        assert_eq!(
            violations_of(&[Item::EventCurrency116], device),
            Vec::<String>::new()
        );
    }
}

/// A guest made a contact, the contact deleted on the same device: the event
/// must not be left naming a contact that nobody can restore.
#[test]
fn deleting_a_guest_contact_leaves_no_dangling_participant() {
    use super::tests::settle;
    let (_dir, mut left, mut right) = two_devices();
    let event = left
        .create_shared_event(SharedEventDraft {
            title: "trip".into(),
            currency: "usd".into(),
            participants: vec![guest("ann".into())],
            ..Default::default()
        })
        .unwrap()
        .event;
    let person = event.participants[0].id.clone();
    let contact = left
        .make_shared_participant_contact(&event.id, &person)
        .unwrap();
    // Either answer is fine for the guard: a refusal, or the link goes too.
    let _ = left.delete_contact(&contact.id);
    settle(&mut left, &mut right);
    for device in [&left, &right] {
        assert_eq!(
            violations_of(&[Item::DeletedReferenced28], device),
            Vec::<String>::new()
        );
    }
}

/// The peer restores a deleted guest contact while a participant names it: once
/// the participant is gone, a deletion that succeeded must still hold.
#[test]
fn deleted_guest_contact_stays_deleted_once_its_participant_goes() {
    use super::tests::settle;
    let (_dir, mut left, mut right) = two_devices();
    let event = left
        .create_shared_event(SharedEventDraft {
            title: "trip".into(),
            currency: "usd".into(),
            participants: vec![guest("ann".into()), guest("bob".into())],
            ..Default::default()
        })
        .unwrap()
        .event;
    let person = event.participants[0].id.clone();
    let contact = left
        .make_shared_participant_contact(&event.id, &person)
        .unwrap();
    settle(&mut left, &mut right);
    let deleted = left.delete_contact(&contact.id).is_ok();
    settle(&mut left, &mut right);
    left.remove_shared_participant(&event.id, &person).unwrap();
    settle(&mut left, &mut right);
    for device in [&left, &right] {
        let alive = device.doc.contacts.iter().any(|row| row.id == contact.id);
        assert!(
            !(deleted && alive),
            "deleted contact {} is back",
            contact.id
        );
    }
}

/// One device deletes a payment while another attaches it to an event: the
/// purchase must not outlive the payment it stands for, nor the other way round.
#[test]
fn attaching_a_payment_beside_its_deletion_leaves_no_phantom_purchase() {
    use super::tests::settle;
    let (_dir, mut left, mut right) = two_devices();
    let store_id = left.doc.stores[0].id.clone();
    left.add_transaction(income(&store_id, "100", "start"))
        .unwrap();
    let paid = left
        .add_transaction(crate::service::test_support::payment(
            &store_id, "3", "taxi",
        ))
        .unwrap();
    let event = left
        .create_shared_event(SharedEventDraft {
            title: "trip".into(),
            currency: "usd".into(),
            participants: vec![guest("ann".into())],
            ..Default::default()
        })
        .unwrap()
        .event;
    settle(&mut left, &mut right);
    let tx_id = paid.meta().id.clone();
    left.delete_transaction(&tx_id).unwrap();
    right
        .attach_payment_to_shared_event(crate::ipc_contract::AttachPaymentInput {
            event_id: event.id.clone(),
            tx_id: tx_id.clone(),
            new_item_title: Some("attached".into()),
            ..Default::default()
        })
        .unwrap();
    settle(&mut left, &mut right);
    for device in [&left, &right] {
        let payment_alive = device
            .doc
            .transactions
            .iter()
            .any(|tx| tx.meta().id == tx_id);
        let linked = device.doc.shared_events[0]
            .items
            .iter()
            .flat_map(|item| &item.purchases)
            .any(|purchase| purchase.tx_id.as_ref() == Some(&tx_id));
        assert_eq!(linked, payment_alive, "purchase and payment disagree");
    }
}
