//! Seeded randomized convergence: three devices edit at random, sync in
//! random (sometimes cut, one-directional) sessions, then exchange fully.
//! They must end with the same synced records, and nothing deleted may return.
//!
//! Replay a failure with `MWT_CONV_SEED=<seed> MWT_CONV_STEPS=<steps>`.

use super::tests::two_devices;
use super::*;
use crate::model::category::CategoryGroup;
use crate::model::contact::ContactKind;
use crate::model::transaction::Transaction;
use crate::service::contacts::ContactDraft;
use crate::service::test_support::income;
use std::collections::{BTreeMap, BTreeSet};

/// Edit kinds the harness may pick. Later stages (B3 slices) add kinds here:
/// shared-event participants/items/settlements, envelope moves, exchange keys,
/// settings maps, adjustment lines, emergency scope.
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
];

const DEFAULT_SEEDS: &[u64] = &[1, 2, 3, 4, 5];
const DEFAULT_STEPS: usize = 200;
const DEVICES: usize = 3;
const MAX_SETTLE_ROUNDS: usize = 10;

#[derive(Clone, Copy)]
enum Edit {
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
}

/// splitmix64: tiny, deterministic, good enough for choosing steps.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut mixed = self.0;
        mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        mixed ^ (mixed >> 31)
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }
}

fn peer_of(index: usize) -> PeerRef {
    PeerRef {
        device_id: format!("device-{index}-id"),
        name: format!("device-{index}"),
    }
}

fn three_devices() -> (tempfile::TempDir, Vec<UnlockedVault>) {
    let (dir, left, right) = two_devices();
    let copy = dir.path().join("third.wallet");
    std::fs::copy(&left.path, &copy).unwrap();
    let mut third = UnlockedVault::open(copy, "pw").unwrap();
    third.set_device("cccccccc-third");
    (dir, vec![left, right, third])
}

/// `from` sends what `to` lacks; only `to` changes. Whether `to` applied.
fn pull(devices: &mut [UnlockedVault], from: usize, to: usize) -> bool {
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
struct Ledger {
    /// Full keys deleted on some device.
    deleted: BTreeSet<String>,
    /// Full keys created or edited after being deleted (exempt from the check).
    touched_after: BTreeSet<String>,
    counter: usize,
}

impl Ledger {
    fn fresh(&mut self) -> usize {
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

fn pick<'items, Item>(rng: &mut Rng, items: &'items [Item]) -> Option<&'items Item> {
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
        Edit::DeleteContact => {
            let ids: Vec<String> = vault
                .doc
                .contacts
                .iter()
                .map(|contact| contact.id.clone())
                .collect();
            let Some(id) = pick(rng, &ids).cloned() else {
                return;
            };
            vault.delete_contact(&id).unwrap();
            ledger.deleted.insert(format!("contacts:{id}"));
        }
    }
}

/// Categories made by this harness (names `cat<N>` / `renamed<N>` keys start with `cat`).
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

fn run(seed: u64, steps: usize) {
    let context =
        format!("seed={seed} steps={steps} (MWT_CONV_SEED={seed} MWT_CONV_STEPS={steps})");
    let (_dir, mut devices) = three_devices();
    let mut rng = Rng(seed);
    let mut ledger = Ledger::default();

    for _ in 0..steps {
        if rng.below(10) < 6 {
            let device = rng.below(DEVICES);
            let kind = ENABLED_KINDS[rng.below(ENABLED_KINDS.len())];
            apply_edit(kind, &mut devices[device], &mut rng, &mut ledger);
        } else {
            let from = rng.below(DEVICES);
            let to = (from + 1 + rng.below(DEVICES - 1)) % DEVICES;
            pull(&mut devices, from, to);
            if rng.below(2) == 0 {
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
            assert!(
                !view.contains_key(key),
                "{context}: deleted record {key} came back on device {index}"
            );
        }
    }
}

fn env_number<Number: std::str::FromStr>(name: &str) -> Option<Number> {
    std::env::var(name).ok().and_then(|raw| raw.parse().ok())
}

#[test]
fn random_edits_and_cut_sessions_converge() {
    let steps = env_number("MWT_CONV_STEPS").unwrap_or(DEFAULT_STEPS);
    let seeds: Vec<u64> = match env_number("MWT_CONV_SEED") {
        Some(seed) => vec![seed],
        None => DEFAULT_SEEDS.to_vec(),
    };
    for seed in seeds {
        run(seed, steps);
    }
}

#[test]
fn convergence_quick_fixed_seed() {
    run(42, 50);
}
