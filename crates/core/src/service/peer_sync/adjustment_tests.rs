//! Lines of a correction sync per asset, and `before` (the ledger balance
//! every device works out again) is never an edit: a backdated operation
//! arriving on one device must not overwrite what the user counted on another.

use serde_json::{Value, json};

use super::tests::{settle, two_devices};
use super::*;
use crate::Amount;
use crate::model::store::{AssetRef, StoreKind};
use crate::model::transaction::{AdjustmentLine, AdjustmentTx, PriceStamp, Transaction, TxMeta};
use crate::service::test_support::{amount, at, holding_of, income, income_of, store, usd};
use crate::sync::merge::merge;
use crate::sync::registry::find;
use crate::sync::stamp::Stamp;
use crate::sync::state::RecordStamps;
use crate::sync::track::Tracker;

fn correction(store_id: &str, lines: &[(AssetRef, &str)], stamp: &str) -> Transaction {
    let mut meta = TxMeta::new("count");
    meta.timestamp = stamp.into();
    Transaction::Adjustment(AdjustmentTx {
        meta,
        store_id: store_id.into(),
        lines: lines
            .iter()
            .map(|(asset, after)| AdjustmentLine {
                asset: asset.clone(),
                before: Amount::ZERO,
                after: amount(after),
                price: PriceStamp::default(),
            })
            .collect(),
        fee: None,
    })
}

/// A wallet with 100 USD (and 1 EUR when `with_second`) and a correction made
/// after it, on both devices: `(store, correction id)`.
fn shared_correction(
    left: &mut UnlockedVault,
    right: &mut UnlockedVault,
    lines: &[(AssetRef, &str)],
    with_second: bool,
) -> (String, String) {
    let cash = store(left, "Cash", StoreKind::Cash, None);
    let mut salary = income(&cash, "100", "salary");
    at(&mut salary, "2026-03-01T10:00:00Z");
    left.add_transaction(salary).unwrap();
    if with_second {
        let mut coin = income_of(&cash, eur(), "1", "1.1", "coin");
        at(&mut coin, "2026-03-01T11:00:00Z");
        left.add_transaction(coin).unwrap();
    }
    let saved = left
        .add_transaction(correction(&cash, lines, "2026-03-05T10:00:00Z"))
        .unwrap();
    let id = saved.meta().id.clone();
    settle(left, right);
    (cash, id)
}

fn eur() -> AssetRef {
    AssetRef::fiat("EUR", "Euro")
}

fn lines_of(vault: &UnlockedVault, id: &str) -> Vec<AdjustmentLine> {
    vault
        .doc
        .transactions
        .iter()
        .find_map(|tx| match tx {
            Transaction::Adjustment(inner) if inner.meta.id == id => Some(inner.lines.clone()),
            _ => None,
        })
        .expect("the correction")
}

fn line_of(vault: &UnlockedVault, id: &str, asset: &AssetRef) -> AdjustmentLine {
    lines_of(vault, id)
        .into_iter()
        .find(|line| line.asset.key() == asset.key())
        .expect("the line")
}

fn set_after(vault: &mut UnlockedVault, id: &str, asset: &AssetRef, after: &str) {
    let mut tx = vault
        .doc
        .transactions
        .iter()
        .find(|tx| tx.meta().id == id)
        .cloned()
        .unwrap();
    if let Transaction::Adjustment(inner) = &mut tx {
        for line in &mut inner.lines {
            if line.asset.key() == asset.key() {
                line.after = amount(after);
            }
        }
    }
    vault.update_transaction(id, tx).unwrap();
}

#[test]
fn backdated_operation_does_not_overwrite_after() {
    let (_dir, mut left, mut right) = two_devices();
    let (cash, id) = shared_correction(&mut left, &mut right, &[(usd(), "80")], false);
    assert_eq!(line_of(&left, &id, &usd()).before, amount("100"));

    // A counts again: 70. B, which has not seen it, learns of an older income.
    set_after(&mut left, &id, &usd(), "70");
    let mut older = income(&cash, "50", "forgotten");
    at(&mut older, "2026-03-02T10:00:00Z");
    right.add_transaction(older).unwrap();
    assert_eq!(line_of(&right, &id, &usd()).before, amount("150"));

    settle(&mut left, &mut right);
    for vault in [&left, &right] {
        let line = line_of(vault, &id, &usd());
        assert_eq!(line.after, amount("70"), "the counted balance survives");
        assert_eq!(line.before, amount("150"), "before follows the ledger");
        assert_eq!(line.delta(), amount("-80"));
        assert_eq!(holding_of(vault, &cash, &usd()), amount("70"));
    }
}

#[test]
fn corrections_of_two_assets_both_kept() {
    let (_dir, mut left, mut right) = two_devices();
    let lines = [(usd(), "80"), (eur(), "2")];
    let (cash, id) = shared_correction(&mut left, &mut right, &lines, true);

    set_after(&mut left, &id, &usd(), "60");
    set_after(&mut right, &id, &eur(), "3");
    settle(&mut left, &mut right);
    for vault in [&left, &right] {
        assert_eq!(lines_of(vault, &id).len(), 2);
        assert_eq!(line_of(vault, &id, &usd()).after, amount("60"));
        assert_eq!(line_of(vault, &id, &eur()).after, amount("3"));
        assert_eq!(holding_of(vault, &cash, &usd()), amount("60"));
        assert_eq!(holding_of(vault, &cash, &eur()), amount("3"));
    }
}

fn adjustment_json(before: &str, after: &str) -> Value {
    json!({"adjustment": {
        "meta": {"id": "t1"},
        "store_id": "s",
        "lines": [{"asset": serde_json::to_value(usd()).unwrap(), "before": before, "after": after}],
        "fee": null
    }})
}

#[test]
fn recomputing_before_stamps_nothing() {
    let mut doc = json!({"transactions": [adjustment_json("100", "80")]});
    let mut tracker = Tracker::capture(&doc);
    let mut state = crate::sync::state::SyncState::default();
    let mut clock = crate::sync::stamp::Clock::new("dev");

    doc["transactions"][0]["adjustment"]["lines"][0]["before"] = json!("150");
    let changed = tracker.stamp(&doc, &mut state, &mut clock);
    assert!(changed.is_empty(), "before alone is no change: {changed:?}");
    assert!(state.stamps.is_empty());

    doc["transactions"][0]["adjustment"]["lines"][0]["after"] = json!("70");
    let changed = tracker.stamp(&doc, &mut state, &mut clock);
    assert_eq!(changed, vec!["transactions:t1".to_string()]);
    let fields = &state.stamps["transactions:t1"].fields;
    assert_eq!(fields.len(), 1, "only the edited line: {fields:?}");
}

fn state_of(value: Value, fields: &[(&str, u64)]) -> RecordState {
    RecordState {
        value: Some(value),
        stamps: Some(RecordStamps {
            created: Stamp::new(1, 0, "a"),
            fields: fields
                .iter()
                .map(|(name, wall)| ((*name).to_string(), Stamp::new(*wall, 0, "a")))
                .collect(),
        }),
        tombstone: None,
    }
}

const LINE_KEY: &str = "asset.symbol+asset.coingecko_id+asset.chain+asset.contract";

fn line_entry() -> String {
    let line = adjustment_json("0", "0")["adjustment"]["lines"][0].clone();
    let key = crate::sync::view::item_key_for_tests(&line, LINE_KEY).expect("key");
    format!("lines.{key}")
}

#[test]
fn a_record_differing_only_in_before_is_no_change() {
    let entry = line_entry();
    let local = state_of(adjustment_json("100", "80"), &[(&entry, 5)]);
    let remote = state_of(adjustment_json("150", "80"), &[(&entry, 9)]);
    let incoming = std::collections::BTreeMap::from([("transactions:t1".to_string(), remote)]);
    let planned = crate::sync::plan::plan(&incoming, |_| local.clone());
    assert_eq!(planned.summary.total, 0, "{:?}", planned.summary);
    assert!(planned.summary.overwritten.is_empty());
    assert!(planned.summary.collections.is_empty());

    // The same stamps and a different `before`: nothing at all to apply.
    let same = state_of(adjustment_json("150", "80"), &[(&entry, 5)]);
    let incoming = std::collections::BTreeMap::from([("transactions:t1".to_string(), same)]);
    let planned = crate::sync::plan::plan(&incoming, |_| local.clone());
    assert!(planned.changes.is_empty());

    // A real edit of `after` on both sides is still a conflict.
    let edited = state_of(adjustment_json("150", "60"), &[(&entry, 9)]);
    let mine = state_of(adjustment_json("100", "70"), &[(&entry, 5)]);
    let incoming = std::collections::BTreeMap::from([("transactions:t1".to_string(), edited)]);
    let planned = crate::sync::plan::plan(&incoming, |_| mine.clone());
    assert_eq!(planned.summary.overwritten.len(), 1);
}

#[test]
fn an_old_device_can_read_a_correction() {
    let (_dir, mut left, mut right) = two_devices();
    let lines = [(usd(), "80"), (eur(), "2")];
    let (_cash, id) = shared_correction(&mut left, &mut right, &lines, true);
    set_after(&mut left, &id, &usd(), "60");
    let key = format!("transactions:{id}");
    for vault in [&mut left, &mut right] {
        let sent = vault.sync_records(std::slice::from_ref(&key)).unwrap();
        let value = sent[&key].value.clone().expect("the record");
        let sent_lines = value["adjustment"]["lines"].as_array().expect("lines");
        assert_eq!(sent_lines.len(), 2);
        for line in sent_lines {
            assert!(
                line.get("before").is_some(),
                "an old device requires before"
            );
        }
        // The typed form an old device reads (`before` has no default).
        serde_json::from_value::<Transaction>(value).expect("readable");
    }
}

#[test]
fn old_lines_head_stamp_is_a_floor() {
    let coll = find("transactions").expect("rule");
    let entry = line_entry();
    // This side edited the line at 5. The older device wrote `lines` as a
    // whole at 3 (older) or 9 (newer).
    let local = state_of(adjustment_json("100", "70"), &[(&entry, 5)]);
    for (head_wall, local_wins) in [(3, true), (9, false)] {
        let remote = state_of(adjustment_json("120", "80"), &[("lines", head_wall)]);
        let there = merge(coll, &local, &remote).state;
        let back = merge(coll, &remote, &local).state;
        assert_eq!(there, back, "merge must not depend on the side");
        let value = there.value.expect("record");
        let after = &value["adjustment"]["lines"][0]["after"];
        assert_eq!(after, if local_wins { "70" } else { "80" });
    }
}

/// Two corrections at one second: the list order differs between devices after
/// a sync, so they are ordered by the creation stamp of their records (the
/// later-created one still wins, as `corrections_at_the_same_moment_settle`
/// wants).
#[test]
fn simultaneous_corrections_end_with_the_same_before_everywhere() {
    let (_dir, mut left, mut right) = two_devices();
    let (cash, _id) = shared_correction(&mut left, &mut right, &[(usd(), "80")], false);
    let moment = "2026-03-06T10:00:00Z";
    let first = left
        .add_transaction(correction(&cash, &[(usd(), "60")], moment))
        .unwrap();
    let second = right
        .add_transaction(correction(&cash, &[(usd(), "50")], moment))
        .unwrap();
    settle(&mut left, &mut right);
    for id in [&first.meta().id, &second.meta().id] {
        assert_eq!(
            line_of(&left, id, &usd()).before,
            line_of(&right, id, &usd()).before,
            "before must not depend on the device"
        );
    }
}
