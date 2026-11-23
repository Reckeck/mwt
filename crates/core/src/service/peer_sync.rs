//! The vault side of device-to-device sync: stamping local changes on every
//! save, describing the records to another device, applying and undoing what
//! comes back (docs/sync.md). The pure parts live in [`crate::sync`].

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::UnlockedVault;
use crate::error::Result;
use crate::ipc_contract::SyncCutReason;
use crate::ipc_contract::security::SecurityLevel;
#[cfg(test)]
use crate::model::transaction::TxOrigin;
pub use crate::sync::history::AppliedBatch;
use crate::sync::manifest::Manifest;
#[cfg(test)]
use crate::sync::manifest_view::is_crypto_key;
use crate::sync::{RecordState, Stamp, TrustedPeer};
// What the test modules below reach through `use super::*`.
#[cfg(test)]
use {
    crate::error::Error, crate::model::vault::VaultDoc, crate::sync::journal::SyncJournalOutcome,
    crate::sync::view, serde_json::Value,
};

/// Told the keys of every local save, so a live session can send them on.
pub trait SyncTap: Send + Sync {
    fn changed(&self, keys: &[String]);
}

/// A [`SyncTap`] as the kernel's change sink (the kernel crate cannot name `SyncTap`).
pub(crate) struct TapSink(pub(crate) Arc<dyn SyncTap>);

impl crate::kernel::events::ChangeSink for TapSink {
    fn changed(&self, keys: &[String]) {
        self.0.changed(keys);
    }
}

/// The device a batch came from.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PeerRef {
    pub device_id: String,
    pub name: String,
}

/// What an undo did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct UndoReport {
    pub restored: u32,
    /// Labels of records changed again since the batch: left as they are.
    pub skipped: Vec<String>,
}

/// What came of reading an external copy into the open vault.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalMerge {
    /// The copy was taken in; the batch it produced, if it changed anything.
    Done(Option<AppliedBatch>),
    /// The copy does not open with this password or is another vault: the
    /// caller falls back to asking the user which side to keep.
    Unmergeable,
}

impl UnlockedVault {
    // Delegates to `usecases::sync::peer_sync`; removed in T24.
    #[cfg(test)]
    pub(crate) fn compact_tombstones(&mut self) {
        crate::usecases::sync::peer_sync::compact_tombstones(self)
    } // removed in T24
    #[cfg(test)]
    pub(crate) fn open_copy(&self, remote: &[u8], password: &str) -> Result<UnlockedVault> {
        crate::usecases::sync::peer_sync::open_copy(self, remote, password)
    } // removed in T24
    #[cfg(test)]
    pub(crate) fn swap_doc(
        &mut self,
        doc: crate::model::vault::VaultDoc,
    ) -> crate::model::vault::VaultDoc {
        crate::usecases::sync::peer_sync::swap_doc(self, doc)
    } // removed in T24
    #[cfg(test)]
    pub(crate) fn dedupe_generated(&mut self, batch_id: &str, carry: bool) -> Result<()> {
        crate::usecases::sync::peer_sync::dedupe_generated(self, batch_id, carry)
    } // removed in T24
    pub(crate) fn stamp_past_known_peer(
        &mut self,
        device_id: &str,
        public_key: Option<&str>,
    ) -> Stamp {
        crate::usecases::sync::peer_sync::stamp_past_known_peer(self, device_id, public_key)
    } // removed in T24
    pub(crate) fn notice_in_russian(&self) -> bool {
        crate::usecases::sync::peer_sync::notice_in_russian(self)
    } // removed in T24
    pub(crate) fn doc_value(&mut self) -> Result<serde_json::Value> {
        crate::usecases::sync::peer_sync::doc_value(self)
    } // removed in T24
    pub(crate) fn start_tracking(&mut self) -> Result<()> {
        crate::usecases::sync::peer_sync::start_tracking(self)
    } // removed in T24
    pub(crate) fn track_changes(&mut self) -> Result<Vec<String>> {
        crate::usecases::sync::peer_sync::track_changes(self)
    } // removed in T24
    pub(crate) fn sync_forget_log_record(&mut self, key: &str) -> Result<()> {
        crate::usecases::sync::peer_sync::sync_forget_log_record(self, key)
    } // removed in T24
    pub(crate) fn apply_batch(
        &mut self,
        incoming: &BTreeMap<String, RecordState>,
        peer: &PeerRef,
        carry: bool,
    ) -> Result<Option<AppliedBatch>> {
        crate::usecases::sync::peer_sync::apply_batch(self, incoming, peer, carry)
    } // removed in T24
    pub(crate) fn remove_sync_backups(&self) {
        crate::usecases::sync::peer_sync::remove_sync_backups(self)
    } // removed in T24
    pub fn sync_peers_to_send(&mut self) -> Result<Vec<TrustedPeer>> {
        crate::usecases::sync::peer_sync::sync_peers_to_send(self)
    } // removed in T24
    pub fn set_device(&mut self, device_id: &str) {
        crate::usecases::sync::peer_sync::set_device(self, device_id)
    } // removed in T24
    pub fn set_sync_tap(&mut self, tap: Option<Arc<dyn SyncTap>>) {
        crate::usecases::sync::peer_sync::set_sync_tap(self, tap)
    } // removed in T24
    pub fn sync_state(&self) -> &crate::sync::SyncState {
        crate::usecases::sync::peer_sync::sync_state(self)
    } // removed in T24
    pub fn sync_manifest_marked(
        &mut self,
        union_since_ms: Option<u64>,
    ) -> Result<(Manifest, Stamp)> {
        crate::usecases::sync::peer_sync::sync_manifest_marked(self, union_since_ms)
    } // removed in T24
    pub fn sync_manifest_marked_for(
        &mut self,
        union_since_ms: Option<u64>,
        peer: SecurityLevel,
    ) -> Result<(Manifest, Stamp, usize)> {
        crate::usecases::sync::peer_sync::sync_manifest_marked_for(self, union_since_ms, peer)
    } // removed in T24
    pub fn sync_apply(
        &mut self,
        incoming: &BTreeMap<String, RecordState>,
        peer: &PeerRef,
    ) -> Result<Option<AppliedBatch>> {
        crate::usecases::sync::peer_sync::sync_apply(self, incoming, peer)
    } // removed in T24
    pub fn probe_external(&self, remote: &[u8]) -> Result<crate::storage::SnapshotFormat> {
        crate::usecases::sync::peer_sync::probe_external(self, remote)
    } // removed in T24
    pub fn merge_from(
        &mut self,
        other: &mut UnlockedVault,
        name: &str,
    ) -> Result<Option<AppliedBatch>> {
        crate::usecases::sync::peer_sync::merge_from(self, other, name)
    } // removed in T24
    pub fn merge_external_copy(
        &mut self,
        remote: &[u8],
        password: &str,
        name: &str,
    ) -> Result<ExternalMerge> {
        crate::usecases::sync::peer_sync::merge_external_copy(self, remote, password, name)
    } // removed in T24
    pub fn take_external_copy(&mut self, remote: &[u8], password: &str) -> Result<ExternalMerge> {
        crate::usecases::sync::peer_sync::take_external_copy(self, remote, password)
    } // removed in T24
    pub fn sync_record_transfer(
        &mut self,
        device_id: &str,
        device_name: &str,
        public_key: &str,
        ru: bool,
    ) -> Result<crate::model::notify::NoticeRecord> {
        crate::usecases::sync::peer_sync::sync_record_transfer(
            self,
            device_id,
            device_name,
            public_key,
            ru,
        )
    } // removed in T24
    pub fn sync_undo(&mut self, batch_id: &str) -> Result<UndoReport> {
        crate::usecases::sync::peer_sync::sync_undo(self, batch_id)
    } // removed in T24
    pub fn adopt_received_copy(&mut self) -> Result<()> {
        crate::usecases::sync::peer_sync::adopt_received_copy(self)
    } // removed in T24
    pub fn sync_merge_peers(&mut self, remote: &[TrustedPeer]) -> Result<bool> {
        crate::usecases::sync::peer_sync::sync_merge_peers(self, remote)
    } // removed in T24
    pub fn sync_pause_peer(&mut self, device_id: &str) -> Result<()> {
        crate::usecases::sync::peer_sync::sync_pause_peer(self, device_id)
    } // removed in T24
    pub fn sync_resume_peer(&mut self, device_id: &str) -> Result<()> {
        crate::usecases::sync::peer_sync::sync_resume_peer(self, device_id)
    } // removed in T24
    pub fn sync_is_paused(&self, device_id: &str) -> bool {
        crate::usecases::sync::peer_sync::sync_is_paused(self, device_id)
    } // removed in T24
    pub fn sync_cut_by(&mut self, device_id: &str, reason: SyncCutReason) -> Result<bool> {
        crate::usecases::sync::peer_sync::sync_cut_by(self, device_id, reason)
    } // removed in T24
    pub fn sync_clear_cut(&mut self, device_id: &str) -> Result<bool> {
        crate::usecases::sync::peer_sync::sync_clear_cut(self, device_id)
    } // removed in T24
    pub fn sync_cut_reason(&self, device_id: &str) -> Option<SyncCutReason> {
        crate::usecases::sync::peer_sync::sync_cut_reason(self, device_id)
    } // removed in T24
    pub fn sync_ack_tombstones(&mut self, device_id: &str, theirs: &Manifest) -> Result<()> {
        crate::usecases::sync::peer_sync::sync_ack_tombstones(self, device_id, theirs)
    } // removed in T24
    pub fn sync_mark_synced(&mut self, device_id: &str, mark: Option<&Stamp>) -> Result<()> {
        crate::usecases::sync::peer_sync::sync_mark_synced(self, device_id, mark)
    } // removed in T24
    pub fn sync_backup(&mut self) -> Result<PathBuf> {
        crate::usecases::sync::peer_sync::sync_backup(self)
    } // removed in T24
    pub fn has_sync_backup(&self) -> bool {
        crate::usecases::sync::peer_sync::has_sync_backup(self)
    } // removed in T24
    pub fn sync_restore_backup(&mut self) -> Result<()> {
        crate::usecases::sync::peer_sync::sync_restore_backup(self)
    } // removed in T24
}

#[cfg(test)]
mod adjustment_tests;
#[cfg(test)]
mod alert_tests;
#[cfg(test)]
mod attachment_cleanup_tests;
mod bad_record;
#[cfg(test)]
mod bad_record_tests;
#[cfg(test)]
mod clock_tests;
#[cfg(test)]
mod convergence_catalog;
#[cfg(test)]
mod convergence_tests;
#[cfg(test)]
mod envelope_tests;
#[cfg(test)]
mod inflation_tests;
#[cfg(test)]
mod keep_tests;
#[cfg(test)]
mod log_tests;
#[cfg(test)]
mod marks_tests;
#[cfg(test)]
mod nested_tests;
#[cfg(test)]
mod notice_settings_tests;
#[cfg(test)]
mod notice_tests;
#[cfg(test)]
mod preview_tests;
#[cfg(test)]
mod purchase_tests;
#[cfg(test)]
mod recreate_tests;
#[cfg(test)]
mod reminder_tests;
mod repair;
mod repair_cash;
mod repair_classes;
mod repair_events;
#[cfg(test)]
mod repair_events_tests;
mod repair_imports;
mod repair_natural;
mod repair_report;
mod repair_restore;
#[cfg(test)]
mod repair_restore_payment_tests;
#[cfg(test)]
mod repair_restore_tests;
mod repoint;
#[cfg(test)]
mod repoint_tests;
#[cfg(test)]
mod rollback_tests;
#[cfg(test)]
mod run_key_tests;
#[cfg(test)]
mod seen_tests;
#[cfg(test)]
mod stale_base_tests;
#[cfg(test)]
mod terminal_tests;
#[cfg(test)]
mod tombstone_tests;
#[cfg(test)]
mod transfer_tests;
#[cfg(test)]
mod unstamped_tests;

#[cfg(test)]
mod tests {
    use super::super::test_support::{empty_vault, income};
    use super::*;
    use crate::model::transaction::Transaction;
    use crate::sync::manifest::diff;

    /// The same vault on two devices: a copy of one file, opened twice.
    pub(super) fn two_devices() -> (tempfile::TempDir, UnlockedVault, UnlockedVault) {
        let (dir, mut left) = empty_vault();
        left.set_device("aaaaaaaa-left");
        left.persist().unwrap();
        let copy = dir.path().join("copy.wallet");
        std::fs::copy(&left.path, &copy).unwrap();
        let mut right = UnlockedVault::open(copy, "pw").unwrap();
        right.set_device("bbbbbbbb-right");
        (dir, left, right)
    }

    fn peer(name: &str) -> PeerRef {
        PeerRef {
            device_id: format!("{name}-id"),
            name: name.into(),
        }
    }

    /// One exchange as a session runs it; returns what each side applied.
    pub(super) fn exchange(
        left: &mut UnlockedVault,
        right: &mut UnlockedVault,
    ) -> (Option<AppliedBatch>, Option<AppliedBatch>) {
        let left_manifest = crate::usecases::sync::read::sync_manifest(left, None).unwrap();
        let right_manifest = crate::usecases::sync::read::sync_manifest(right, None).unwrap();
        let (left_wants, _) =
            crate::usecases::sync::read::sync_diff(left, &left_manifest, &right_manifest);
        let (right_wants, _) =
            crate::usecases::sync::read::sync_diff(right, &right_manifest, &left_manifest);
        let to_left = crate::usecases::sync::read::sync_records(right, &left_wants).unwrap();
        let to_right = crate::usecases::sync::read::sync_records(left, &right_wants).unwrap();
        let applied_left = left.sync_apply(&to_left, &peer("right")).unwrap();
        let applied_right = right.sync_apply(&to_right, &peer("left")).unwrap();
        (applied_left, applied_right)
    }

    /// Exchange until nothing moves; both sides must then agree.
    pub(super) fn settle(left: &mut UnlockedVault, right: &mut UnlockedVault) {
        for _ in 0..4 {
            if exchange(left, right) == (None, None) {
                break;
            }
        }
        assert_eq!(
            crate::usecases::sync::read::sync_manifest(left, None).unwrap(),
            crate::usecases::sync::read::sync_manifest(right, None).unwrap(),
            "devices did not converge"
        );
    }

    struct NoWake;
    impl super::super::saving::SaveHook for NoWake {
        fn dirty(&self) {}
    }

    /// Collects what the vault announces, in place of a live link.
    #[derive(Default)]
    struct Announced(std::sync::Mutex<Vec<String>>);

    impl SyncTap for Announced {
        fn changed(&self, keys: &[String]) {
            self.0.lock().unwrap().extend(keys.iter().cloned());
        }
    }

    #[test]
    fn an_edit_just_before_a_sync_call_is_pushed_in_the_same_session() {
        let (_dir, mut left, right) = two_devices();
        let tap = Arc::new(Announced::default());
        left.set_sync_tap(Some(tap.clone()));
        left.set_write_behind(Some(Arc::new(NoWake)));
        left.start_tracking().unwrap();
        let cash = cash_id(&left);
        left.doc.stores[0].name = "Renamed in a hurry".into();
        left.persist().unwrap();
        // A sync call stamps the pending edit before the save-time push.
        crate::usecases::sync::read::sync_manifest(&mut left, None).unwrap();
        let job = left.prepare_flush().unwrap().expect("dirty");
        drop(job);
        let sent = tap.0.lock().unwrap().clone();
        assert!(
            sent.iter().any(|key| key.contains(&cash)),
            "the edit was stamped by the sync call and never announced: {sent:?}"
        );
        drop(right);
    }

    #[test]
    fn set_sync_tap_still_receives_the_changed_keys() {
        let (_dir, mut left, right) = two_devices();
        let tap = Arc::new(Announced::default());
        left.set_sync_tap(Some(tap.clone()));
        left.set_write_behind(Some(Arc::new(NoWake)));
        left.start_tracking().unwrap();
        let cash = cash_id(&left);
        let stamped_before = left.doc.sync.stamps.clone();
        left.doc.stores[0].name = "Renamed for the tap".into();
        left.persist().unwrap();
        assert!(
            tap.0.lock().unwrap().is_empty(),
            "persist announces nothing"
        );
        drop(left.prepare_flush().unwrap().expect("dirty"));
        let mut expected: Vec<String> = left
            .doc
            .sync
            .stamps
            .iter()
            .filter(|(key, stamp)| stamped_before.get(*key) != Some(*stamp))
            .map(|(key, _)| key.clone())
            .collect();
        expected.sort();
        let mut sent = tap.0.lock().unwrap().clone();
        sent.sort();
        assert!(sent.iter().any(|key| key.contains(&cash)));
        assert_eq!(sent, expected, "the tap got exactly the changed keys");
        drop(right);
    }

    #[test]
    fn a_write_now_batch_announces_its_dedupe_keys_right_after_persist() {
        let (_dir, mut left, mut right) = two_devices();
        let tap = Arc::new(Announced::default());
        left.set_sync_tap(Some(tap.clone()));
        rent(&mut left, "first");
        pause();
        rent(&mut left, "second");
        right.doc.stores[0].color = "#123456".into();
        right.persist().unwrap();
        tap.0.lock().unwrap().clear();
        let (applied, _) = exchange(&mut left, &mut right);
        let batch = applied.expect("left applied the store change");
        assert_eq!(left.doc.transactions.len(), 1, "dedupe removed a copy");
        let sent = tap.0.lock().unwrap().clone();
        assert!(
            sent.iter().any(|key| key.starts_with("transactions:")),
            "the dedupe deletion of batch {} was not announced: {sent:?}",
            batch.id
        );
    }

    #[test]
    fn a_sync_tap_is_a_change_sink() {
        use crate::kernel::events::ChangeSink;
        let tap = Arc::new(Announced::default());
        ChangeSink::changed(&TapSink(tap.clone()), &["a:1".to_string()]);
        assert_eq!(tap.0.lock().unwrap().clone(), vec!["a:1".to_string()]);
    }

    fn fx_point(currency: &str, ts: &str) -> crate::model::vault::FxPoint {
        crate::model::vault::FxPoint {
            currency: currency.into(),
            per_usd: crate::Amount::from(2u32),
            ts: ts.into(),
            source: "test".into(),
        }
    }

    /// How many cache records the next exchange would move, both ways.
    fn cache_records_moving(left: &mut UnlockedVault, right: &mut UnlockedVault) -> usize {
        let left_manifest = crate::usecases::sync::read::sync_manifest(left, None).unwrap();
        let right_manifest = crate::usecases::sync::read::sync_manifest(right, None).unwrap();
        let (left_wants, _) =
            crate::usecases::sync::read::sync_diff(left, &left_manifest, &right_manifest);
        let (right_wants, _) =
            crate::usecases::sync::read::sync_diff(right, &right_manifest, &left_manifest);
        left_wants
            .iter()
            .chain(&right_wants)
            .filter(|key| key.starts_with("fx_cache:") || key.starts_with("price_cache:"))
            .count()
    }

    #[test]
    fn an_fx_refresh_on_each_copy_asks_nothing() {
        let (_dir, mut left, mut right) = two_devices();
        let currencies = ["chf", "jpy", "pln"];
        crate::usecases::money::rates::apply_fx(
            &mut left,
            &currencies.map(|code| fx_point(code, "2026-11-03T10:00:00Z")),
        );
        crate::usecases::money::rates::apply_fx(
            &mut right,
            &currencies.map(|code| fx_point(code, "2026-11-03T12:00:00Z")),
        );
        // Whatever an older peer would still offer, point by point.
        let keys: Vec<String> = left
            .doc
            .fx_cache
            .iter()
            .map(|point| format!("fx_cache:{}|{}", point.currency, point.ts))
            .collect();
        assert!(!keys.is_empty());
        let incoming = crate::usecases::sync::read::sync_records(&mut left, &keys).unwrap();
        let summary = crate::usecases::sync::read::sync_preview(&mut right, &incoming).unwrap();
        assert_eq!(
            summary.total, 0,
            "cache points reached the question: {summary:?}"
        );
        assert!(summary.collections.is_empty(), "{summary:?}");
    }

    #[test]
    fn thinned_cache_points_are_not_requested_again() {
        let (_dir, mut left, mut right) = two_devices();
        // The base currency is a device setting: only the left copy keeps
        // euro history; the right one thins whatever arrives to one point.
        left.base_currency = "eur".into();
        for hour in ["08", "10", "12"] {
            crate::usecases::money::rates::apply_fx(
                &mut left,
                &[fx_point("eur", &format!("2026-11-03T{hour}:00:00Z"))],
            );
        }
        exchange(&mut left, &mut right);
        exchange(&mut left, &mut right);
        assert_eq!(cache_records_moving(&mut left, &mut right), 0);
    }

    #[test]
    fn a_cache_point_of_a_currency_in_use_still_syncs() {
        let (_dir, mut left, mut right) = two_devices();
        for vault in [&mut left, &mut right] {
            vault.doc.settings.budget_currency = "eur".into();
            vault.persist().unwrap();
        }
        crate::usecases::money::rates::apply_fx(
            &mut left,
            &[fx_point("eur", "2026-11-03T10:00:00Z")],
        );
        crate::usecases::money::rates::apply_fx(
            &mut right,
            &[fx_point("eur", "2026-11-03T12:00:00Z")],
        );
        settle(&mut left, &mut right);
        for vault in [&left, &right] {
            let stamps: Vec<&str> = vault
                .doc
                .fx_cache
                .iter()
                .filter(|point| point.currency == "eur")
                .map(|point| point.ts.as_str())
                .collect();
            assert!(stamps.contains(&"2026-11-03T10:00:00Z"), "{stamps:?}");
            assert!(stamps.contains(&"2026-11-03T12:00:00Z"), "{stamps:?}");
        }
        assert_eq!(cache_records_moving(&mut left, &mut right), 0);
    }

    fn snapshot(
        month: &str,
        color: crate::model::planning::HealthColor,
        reasons: Vec<crate::model::planning::HealthReasonCode>,
    ) -> Option<crate::model::planning::HealthSnapshot> {
        Some(crate::model::planning::HealthSnapshot {
            month: month.into(),
            color,
            reasons,
        })
    }

    #[test]
    fn health_last_with_an_empty_slot_converges() {
        use crate::model::planning::{HealthColor, HealthReasonCode};
        let (_dir, mut left, mut right) = two_devices();
        left.doc.health_last = snapshot(
            "2026-10",
            HealthColor::Yellow,
            vec![HealthReasonCode::NoHistory],
        );
        left.persist().unwrap();
        settle(&mut left, &mut right);
        assert_eq!(right.doc.health_last, left.doc.health_last);
        assert!(right.doc.health_last.is_some());

        // Emptying the slot travels like any other edit.
        left.doc.health_last = None;
        left.persist().unwrap();
        settle(&mut left, &mut right);
        assert_eq!(right.doc.health_last, None);
        assert_eq!(left.doc.health_last, None);
    }

    #[test]
    fn two_health_snapshots_do_not_mix_fields() {
        use crate::model::planning::{HealthColor, HealthReasonCode};
        let (_dir, mut left, mut right) = two_devices();
        let base = snapshot("2026-09", HealthColor::Green, vec![]);
        left.doc.health_last = base.clone();
        left.persist().unwrap();
        settle(&mut left, &mut right);
        // The right device writes a whole snapshot first ...
        let earlier = snapshot(
            "2026-10",
            HealthColor::Red,
            vec![HealthReasonCode::NoHistory],
        );
        right.doc.health_last = earlier;
        right.persist().unwrap();
        // ... the left one a later snapshot that keeps the colour of the base.
        let later = snapshot(
            "2026-10",
            HealthColor::Green,
            vec![HealthReasonCode::CostlyLoan],
        );
        left.doc.health_last = later.clone();
        left.persist().unwrap();
        settle(&mut left, &mut right);
        assert_eq!(left.doc.health_last, later, "the later snapshot wins whole");
        assert_eq!(right.doc.health_last, later);
    }

    #[test]
    fn a_repeated_session_with_equal_health_makes_no_batch() {
        use crate::model::planning::{HealthColor, HealthReasonCode};
        let (_dir, mut left, mut right) = two_devices();
        // Both empty.
        settle(&mut left, &mut right);
        assert_eq!(exchange(&mut left, &mut right), (None, None));
        // Both equal after a sync.
        left.doc.health_last = snapshot(
            "2026-10",
            HealthColor::Red,
            vec![HealthReasonCode::NoHistory],
        );
        left.persist().unwrap();
        settle(&mut left, &mut right);
        assert_eq!(exchange(&mut left, &mut right), (None, None));
        assert_eq!(exchange(&mut left, &mut right), (None, None));
    }

    fn cash_id(vault: &UnlockedVault) -> String {
        vault.doc.stores[0].id.clone()
    }

    fn comments(vault: &UnlockedVault) -> Vec<String> {
        let mut out: Vec<String> = vault
            .doc
            .transactions
            .iter()
            .map(|tx| tx.meta().comment.clone())
            .collect();
        out.sort();
        out
    }

    /// `count` hourly BTC quotes from 2025-01-01, `offset` seconds late.
    fn hourly_prices(count: i64, offset: i64) -> Vec<crate::model::vault::PricePoint> {
        use time::format_description::well_known::Rfc3339;
        let base = 1_735_689_600;
        (0..count)
            .map(|hour| crate::model::vault::PricePoint {
                asset_key: "crypto:btc".into(),
                value_usd: crate::Amount::from(u32::try_from(hour).unwrap() + 1),
                ts: time::OffsetDateTime::from_unix_timestamp(base + hour * 3600 + offset)
                    .unwrap()
                    .format(&Rfc3339)
                    .unwrap(),
                source: "test".into(),
                change_24h_pct: None,
            })
            .collect()
    }

    fn price_keys(vault: &UnlockedVault) -> Vec<String> {
        let mut keys: Vec<String> = vault
            .doc
            .price_cache
            .iter()
            .map(|point| format!("{}@{}", point.asset_key, point.ts))
            .collect();
        keys.sort();
        keys
    }

    fn set_prices(vault: &mut UnlockedVault, points: Vec<crate::model::vault::PricePoint>) {
        vault.doc.price_cache = points;
        vault.persist().unwrap();
    }

    fn within_caps(vault: &UnlockedVault) -> bool {
        use crate::pricing::history::{HISTORY_CAP, HOURLY_KEEP};
        vault.doc.price_cache.len() <= HISTORY_CAP
            && vault.doc.price_cache.len() < 2 * HOURLY_KEEP + 400
    }

    fn file_bytes(vault: &UnlockedVault) -> Vec<u8> {
        std::fs::read(&vault.path).unwrap()
    }

    const COPY: &str = "Внешняя копия";

    #[test]
    fn merge_keeps_an_edit_made_only_locally_and_one_made_only_remotely() {
        // A edits X and locks; C saves without X; A opens: X must survive.
        let (_dir, mut left, mut right) = two_devices();
        let cash = cash_id(&left);
        crate::usecases::money::transactions::add_transaction(&mut left, income(&cash, "10", "X"))
            .unwrap();
        crate::usecases::money::transactions::add_transaction(&mut right, income(&cash, "20", "Y"))
            .unwrap();

        let outcome = left
            .merge_external_copy(&file_bytes(&right), "pw", COPY)
            .unwrap();
        let ExternalMerge::Done(Some(batch)) = outcome else {
            panic!("expected an applied batch, got {outcome:?}");
        };
        assert_eq!(batch.peer_name, COPY);
        assert_eq!(comments(&left), ["X", "Y"]);
        // What a push would now send holds both.
        let reopened = UnlockedVault::open(left.path.clone(), "pw").unwrap();
        assert_eq!(comments(&reopened), ["X", "Y"]);
    }

    #[test]
    fn merge_refuses_another_vault() {
        let (_dir, mut left, _right) = two_devices();
        let (_other_dir, other) = empty_vault();
        assert_ne!(left.doc.id, other.doc.id);
        let mut other = other;
        assert!(left.merge_from(&mut other, COPY).is_err());
        assert_eq!(
            left.merge_external_copy(&file_bytes(&other), "pw", COPY)
                .unwrap(),
            ExternalMerge::Unmergeable
        );
    }

    #[test]
    fn merge_keeps_the_local_bookkeeping_and_leaves_the_remote_one() {
        let (_dir, mut left, mut right) = two_devices();
        let cash = cash_id(&left);
        crate::usecases::money::transactions::add_transaction(
            &mut left,
            income(&cash, "10", "mine"),
        )
        .unwrap();
        crate::usecases::money::transactions::add_transaction(
            &mut right,
            income(&cash, "20", "theirs"),
        )
        .unwrap();
        // The remote carries bookkeeping of its own.
        right.doc.sync.paused.insert("foreign-device".into());
        right
            .doc
            .sync
            .tombstones_seen
            .insert("transactions:foreign".into(), 1);
        right.persist().unwrap();
        let remote_batches = right.doc.sync.batches.clone();
        let remote_journal = right.doc.sync.journal.clone();

        let batches_before: Vec<String> = left
            .doc
            .sync
            .batches
            .iter()
            .map(|batch| batch.id.clone())
            .collect();
        let journal_before = left.doc.sync.journal.len();
        left.merge_external_copy(&file_bytes(&right), "pw", COPY)
            .unwrap();

        let batches_after: Vec<String> = left
            .doc
            .sync
            .batches
            .iter()
            .map(|batch| batch.id.clone())
            .collect();
        assert_eq!(batches_after[..batches_before.len()], batches_before[..]);
        // Only the merge's own batch was added.
        assert_eq!(batches_after.len(), batches_before.len() + 1);
        assert!(
            remote_batches
                .iter()
                .all(|batch| !batches_after.contains(&batch.id))
        );
        assert_eq!(left.doc.sync.journal.len(), journal_before + 1);
        assert!(
            remote_journal
                .iter()
                .all(|entry| !left.doc.sync.journal.contains(entry))
        );
        assert!(!left.doc.sync.paused.contains("foreign-device"));
        assert!(
            !left
                .doc
                .sync
                .tombstones_seen
                .contains_key("transactions:foreign")
        );
    }

    #[test]
    fn merge_with_a_wrong_password_is_unmergeable() {
        let (_dir, mut left, mut right) = two_devices();
        let cash = cash_id(&left);
        crate::usecases::money::transactions::add_transaction(
            &mut right,
            income(&cash, "20", "theirs"),
        )
        .unwrap();
        assert_eq!(
            left.merge_external_copy(&file_bytes(&right), "not-the-password", COPY)
                .unwrap(),
            ExternalMerge::Unmergeable
        );
        assert!(comments(&left).is_empty());
    }

    #[test]
    fn a_pull_from_a_copy_with_a_foreign_sync_state_keeps_the_local_one() {
        let (_dir, mut left, mut right) = two_devices();
        let cash = cash_id(&left);
        crate::usecases::money::transactions::add_transaction(
            &mut right,
            income(&cash, "20", "theirs"),
        )
        .unwrap();
        right.doc.sync.paused.insert("foreign-device".into());
        crate::usecases::sync::trust::sync_trust_peer(&mut right, "foreign-id", "Foreign", "key")
            .unwrap();
        left.doc.sync.paused.insert("local-device".into());
        left.persist().unwrap();
        let local_sync = left.doc.sync.clone();

        let outcome = left.take_external_copy(&file_bytes(&right), "pw").unwrap();
        assert_eq!(outcome, ExternalMerge::Done(None));
        assert_eq!(comments(&left), ["theirs"]);
        assert_eq!(left.doc.sync.paused, local_sync.paused);
        // Trusted peers travel between devices: the copy's join the local ones.
        assert!(
            local_sync
                .peers
                .iter()
                .all(|peer| left.doc.sync.peers.contains(peer))
        );
        assert!(
            left.doc
                .sync
                .peers
                .iter()
                .any(|peer| peer.device_id == "foreign-id")
        );
        assert_eq!(left.doc.sync.batches, local_sync.batches);
        assert_eq!(left.doc.sync.journal, local_sync.journal);
        let reopened = UnlockedVault::open(left.path.clone(), "pw").unwrap();
        assert_eq!(comments(&reopened), ["theirs"]);
        assert!(!reopened.doc.sync.paused.contains("foreign-device"));
    }

    fn tx_key(vault: &mut UnlockedVault) -> String {
        crate::usecases::sync::read::sync_manifest(vault, None)
            .unwrap()
            .keys()
            .find(|key| key.starts_with("transactions:"))
            .cloned()
            .expect("a transaction record")
    }

    fn set_comment(vault: &mut UnlockedVault, comment: &str) {
        let mut tx = vault.doc.transactions[0].clone();
        let id = tx.meta().id.clone();
        tx.meta_mut().comment = comment.into();
        crate::usecases::money::transactions::update_transaction(vault, &id, tx).unwrap();
    }

    /// One shared transaction on both sides.
    fn shared_transaction() -> (tempfile::TempDir, UnlockedVault, UnlockedVault, String) {
        let (dir, mut left, mut right) = two_devices();
        let cash = cash_id(&left);
        crate::usecases::money::transactions::add_transaction(
            &mut left,
            income(&cash, "10", "shared"),
        )
        .unwrap();
        settle(&mut left, &mut right);
        let key = tx_key(&mut left);
        (dir, left, right, key)
    }

    #[test]
    fn a_pulled_edit_keeps_its_remote_stamp() {
        let (_dir, mut left, mut right, key) = shared_transaction();
        // A peer's edit, older than the one the copy holds.
        set_comment(&mut right, "peer edit");
        let older =
            crate::usecases::sync::read::sync_records(&mut right, std::slice::from_ref(&key))
                .unwrap();
        set_comment(&mut right, "copy edit");
        let remote_stamps = right.doc.sync.stamps[&key].clone();

        left.take_external_copy(&file_bytes(&right), "pw").unwrap();
        assert_eq!(comments(&left), ["copy edit"]);
        assert_eq!(left.doc.sync.stamps[&key], remote_stamps);
        // The older edit must not win over the pulled value.
        assert_eq!(left.sync_apply(&older, &peer("third")).unwrap(), None);
        assert_eq!(comments(&left), ["copy edit"]);
    }

    #[test]
    fn a_deletion_in_the_pulled_copy_stays_deleted() {
        let (_dir, mut left, mut right, key) = shared_transaction();
        let alive =
            crate::usecases::sync::read::sync_records(&mut left, std::slice::from_ref(&key))
                .unwrap();
        let id = right.doc.transactions[0].meta().id.clone();
        crate::usecases::money::transactions::delete_transaction(&mut right, &id).unwrap();

        left.take_external_copy(&file_bytes(&right), "pw").unwrap();
        assert!(left.doc.transactions.is_empty());
        assert!(left.doc.sync.tombstones.contains_key(&key));
        // A peer that still offers the old record does not bring it back.
        assert_eq!(left.sync_apply(&alive, &peer("third")).unwrap(), None);
        assert!(left.doc.transactions.is_empty());
    }

    #[test]
    fn a_pull_from_another_vault_is_unmergeable() {
        let (_dir, mut left, _right) = two_devices();
        let (_other_dir, other) = empty_vault();
        assert_eq!(
            left.take_external_copy(&file_bytes(&other), "pw").unwrap(),
            ExternalMerge::Unmergeable
        );
    }

    #[test]
    fn caches_thinned_apart_converge_to_one_thinned_set() {
        use crate::pricing::history::thin_prices;
        let (_dir, mut left, mut right) = two_devices();
        let mut own_left = hourly_prices(500, 0);
        let mut own_right = hourly_prices(500, 1800);
        thin_prices(&mut own_left);
        thin_prices(&mut own_right);
        set_prices(&mut left, own_left);
        set_prices(&mut right, own_right);

        exchange(&mut left, &mut right);
        // The union overshoots a single device's thinning; both cut it back.
        let mut expected = left.doc.price_cache.clone();
        thin_prices(&mut expected);
        assert_eq!(left.doc.price_cache.len(), expected.len());
        assert!(within_caps(&left));
        assert_eq!(price_keys(&left), price_keys(&right));

        // A second diff of the price collections is empty.
        let left_manifest = crate::usecases::sync::read::sync_manifest(&mut left, None).unwrap();
        let right_manifest = crate::usecases::sync::read::sync_manifest(&mut right, None).unwrap();
        let (left_wants, right_wants) = diff(&left_manifest, &right_manifest);
        assert!(left_wants.is_empty() && right_wants.is_empty());
        settle(&mut left, &mut right);
    }

    #[test]
    fn a_peer_that_never_thins_does_not_grow_this_side() {
        let (_dir, mut left, mut right) = two_devices();
        let dense = hourly_prices(800, 0);
        let mut seen = None;
        for _ in 0..4 {
            // The old peer still holds (and offers) every point it ever had.
            set_prices(&mut right, dense.clone());
            exchange(&mut left, &mut right);
            assert!(within_caps(&left));
            let len = left.doc.price_cache.len();
            assert!(len < dense.len(), "left kept {len} of {}", dense.len());
            if let Some(before) = seen {
                assert_eq!(before, len, "the cache grew between exchanges");
            }
            seen = Some(len);
        }
    }

    #[test]
    fn undo_of_a_batch_with_thinned_away_points_does_not_panic() {
        let (_dir, mut left, mut right) = two_devices();
        set_prices(&mut right, hourly_prices(800, 0));
        let (applied, _) = exchange(&mut left, &mut right);
        let batch = applied.expect("left received prices");
        assert!(left.doc.price_cache.len() < 800);
        let report = left.sync_undo(&batch.id).unwrap();
        // Cached points are a union: it has no deletion, so undo lists them
        // as skipped instead of removing what a peer would send back.
        assert_eq!(report.restored, 0);
        assert!(!report.skipped.is_empty());
        assert!(left.doc.price_cache.len() <= 800);
        // The next thinning cuts anything the undo brought back.
        let mut again = left.doc.price_cache.clone();
        crate::pricing::history::thin_prices(&mut again);
        assert!(again.len() <= left.doc.price_cache.len());
    }

    #[test]
    fn edits_on_both_devices_end_up_on_both() {
        let (_dir, mut left, mut right) = two_devices();
        let cash = cash_id(&left);
        crate::usecases::money::transactions::add_transaction(
            &mut left,
            income(&cash, "10", "from left"),
        )
        .unwrap();
        crate::usecases::money::transactions::add_transaction(
            &mut right,
            income(&cash, "20", "from right"),
        )
        .unwrap();

        let (applied, _) = exchange(&mut left, &mut right);
        let summary = applied.expect("left took something").summary;
        assert_eq!(summary.total, 1);
        assert_eq!(summary.collections[0].collection, "transactions");
        settle(&mut left, &mut right);

        assert_eq!(comments(&left), vec!["from left", "from right"]);
        assert_eq!(comments(&right), comments(&left));
        // Holdings are re-derived, not synced.
        assert_eq!(left.doc.holdings.len(), right.doc.holdings.len());
    }

    #[test]
    fn different_fields_of_one_record_both_survive() {
        let (_dir, mut left, mut right) = two_devices();
        left.doc.stores[0].name = "Wallet".into();
        left.persist().unwrap();
        right.doc.stores[0].color = "#123456".into();
        right.persist().unwrap();
        settle(&mut left, &mut right);
        assert_eq!(left.doc.stores[0].name, "Wallet");
        assert_eq!(left.doc.stores[0].color, "#123456");
        assert_eq!(right.doc.stores[0], left.doc.stores[0]);
    }

    #[test]
    fn the_later_edit_of_one_field_wins_and_is_reported() {
        let (_dir, mut left, mut right) = two_devices();
        left.doc.stores[0].name = "Older".into();
        left.persist().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        right.doc.stores[0].name = "Newer".into();
        right.persist().unwrap();
        let (applied, _) = exchange(&mut left, &mut right);
        let summary = applied.expect("left changed").summary;
        assert_eq!(summary.overwritten.len(), 1);
        assert_eq!(summary.overwritten[0].before, "Older");
        assert_eq!(summary.overwritten[0].after, "Newer");
        settle(&mut left, &mut right);
        assert_eq!(right.doc.stores[0].name, "Newer");
    }

    #[test]
    fn a_deletion_reaches_the_other_device() {
        let (_dir, mut left, mut right) = two_devices();
        let cash = cash_id(&left);
        let tx = crate::usecases::money::transactions::add_transaction(
            &mut left,
            income(&cash, "10", "doomed"),
        )
        .unwrap();
        settle(&mut left, &mut right);
        assert_eq!(comments(&right), vec!["doomed"]);
        crate::usecases::money::transactions::delete_transaction(&mut right, &tx.meta().id)
            .unwrap();
        let (applied, _) = exchange(&mut left, &mut right);
        let summary = applied.expect("left changed").summary;
        assert_eq!(summary.collections[0].deleted, 1);
        settle(&mut left, &mut right);
        assert!(left.doc.transactions.is_empty());
    }

    #[test]
    fn undo_puts_records_back_and_the_undo_travels() {
        let (_dir, mut left, mut right) = two_devices();
        let cash = cash_id(&left);
        crate::usecases::money::transactions::add_transaction(
            &mut right,
            income(&cash, "20", "theirs"),
        )
        .unwrap();
        right.doc.stores[0].name = "Renamed".into();
        right.persist().unwrap();
        let (applied, _) = exchange(&mut left, &mut right);
        let batch = applied.expect("left changed");
        assert_eq!(crate::sync::history::history(&left.doc)[0].id, batch.id);
        let line = &crate::usecases::sync::sync_journal::sync_journal(&left)[0];
        assert_eq!(line.batch_id.as_deref(), Some(batch.id.as_str()));
        assert_eq!(line.outcome, SyncJournalOutcome::Applied);

        let report = left.sync_undo(&batch.id).unwrap();
        assert_eq!(report.restored, 2);
        assert!(left.doc.transactions.is_empty());
        assert_eq!(left.doc.stores[0].name, "Cash");
        assert!(
            crate::sync::history::history(&left.doc)[0]
                .undone_at
                .is_some()
        );
        assert!(left.sync_undo(&batch.id).is_err());

        settle(&mut left, &mut right);
        assert!(right.doc.transactions.is_empty());
        assert_eq!(right.doc.stores[0].name, "Cash");
    }

    #[test]
    fn undo_leaves_records_changed_since() {
        let (_dir, mut left, mut right) = two_devices();
        let cash = cash_id(&left);
        crate::usecases::money::transactions::add_transaction(
            &mut right,
            income(&cash, "20", "theirs"),
        )
        .unwrap();
        crate::usecases::money::transactions::add_transaction(
            &mut right,
            income(&cash, "30", "kept"),
        )
        .unwrap();
        let (applied, _) = exchange(&mut left, &mut right);
        let batch = applied.expect("left changed");
        let kept = left
            .doc
            .transactions
            .iter_mut()
            .find(|tx| tx.meta().comment == "kept")
            .unwrap();
        if let Transaction::Topup(topup) = kept {
            topup.meta.comment = "kept and edited".into();
        }
        left.persist().unwrap();
        let report = left.sync_undo(&batch.id).unwrap();
        assert_eq!(report.restored, 1);
        // The readable label of the record: its comment, date and amount.
        assert_eq!(report.skipped.len(), 1);
        assert!(
            report.skipped[0].starts_with("kept and edited"),
            "{:?}",
            report.skipped
        );
        assert_eq!(comments(&left), vec!["kept and edited"]);
    }

    #[test]
    fn an_automation_run_on_both_devices_books_once() {
        let (_dir, mut left, mut right) = two_devices();
        let cash = cash_id(&left);
        let origin = TxOrigin::Automation {
            automation_id: "rent".into(),
            planned_at: "2026-11-01".into(),
        };
        for vault in [&mut left, &mut right] {
            let mut tx = income(&cash, "100", "rent");
            if let Transaction::Topup(topup) = &mut tx {
                topup.meta.origin = origin.clone();
            }
            crate::usecases::money::transactions::add_transaction(vault, tx).unwrap();
        }
        settle(&mut left, &mut right);
        assert_eq!(left.doc.transactions.len(), 1);
        assert_eq!(right.doc.transactions, left.doc.transactions);
    }

    /// An automation's operation for one planned moment; its id.
    fn rent(vault: &mut UnlockedVault, comment: &str) -> String {
        let cash = cash_id(vault);
        let mut tx = income(&cash, "100", comment);
        if let Transaction::Topup(topup) = &mut tx {
            topup.meta.origin = TxOrigin::Automation {
                automation_id: "rent".into(),
                planned_at: "2026-11-01".into(),
            };
        }
        crate::usecases::money::transactions::add_transaction(vault, tx)
            .unwrap()
            .meta()
            .id
            .clone()
    }

    fn edit_comment(vault: &mut UnlockedVault, id: &str, text: &str) {
        for tx in &mut vault.doc.transactions {
            if let Transaction::Topup(topup) = tx
                && topup.meta.id == id
            {
                topup.meta.comment = text.into();
            }
        }
        vault.persist().unwrap();
    }

    fn pause() {
        std::thread::sleep(std::time::Duration::from_millis(5));
    }

    /// [`exchange`] with the right side still running the earlier dedupe.
    fn exchange_with_old_right(left: &mut UnlockedVault, right: &mut UnlockedVault) -> bool {
        let left_manifest = crate::usecases::sync::read::sync_manifest(left, None).unwrap();
        let right_manifest = crate::usecases::sync::read::sync_manifest(right, None).unwrap();
        let (left_wants, right_wants) = diff(&left_manifest, &right_manifest);
        let to_left = crate::usecases::sync::read::sync_records(right, &left_wants).unwrap();
        let to_right = crate::usecases::sync::read::sync_records(left, &right_wants).unwrap();
        let applied_left = left.sync_apply(&to_left, &peer("right")).unwrap();
        let applied_right = right.apply_batch(&to_right, &peer("left"), false).unwrap();
        applied_left.is_some() || applied_right.is_some()
    }

    #[test]
    fn dedupe_keeps_the_later_edit_of_either_copy() {
        // The copy made later is edited: its edit survives in the kept one.
        let (_dir, mut left, mut right) = two_devices();
        rent(&mut left, "rent");
        pause();
        let late = rent(&mut right, "rent");
        pause();
        edit_comment(&mut right, &late, "rent, corrected");
        settle(&mut left, &mut right);
        assert_eq!(comments(&left), vec!["rent, corrected"]);
        assert_eq!(comments(&right), comments(&left));

        // The kept copy is edited after the other one: its edit stays.
        let (_dir, mut left, mut right) = two_devices();
        let early = rent(&mut left, "rent");
        pause();
        let late = rent(&mut right, "rent");
        pause();
        edit_comment(&mut right, &late, "from the later copy");
        pause();
        edit_comment(&mut left, &early, "from the kept copy");
        settle(&mut left, &mut right);
        assert_eq!(comments(&left), vec!["from the kept copy"]);
        assert_eq!(comments(&right), comments(&left));
    }

    #[test]
    fn a_pair_with_the_earlier_dedupe_converges_to_one_record() {
        let (_dir, mut left, mut right) = two_devices();
        rent(&mut left, "rent");
        pause();
        let late = rent(&mut right, "rent");
        pause();
        edit_comment(&mut right, &late, "rent, corrected");
        let mut rounds = 0;
        while exchange_with_old_right(&mut left, &mut right) {
            rounds += 1;
            assert!(rounds <= 3, "the pair keeps deleting and recreating");
        }
        assert_eq!(comments(&left), vec!["rent, corrected"]);
        assert_eq!(comments(&right), comments(&left));
        assert_eq!(
            crate::usecases::sync::read::sync_manifest(&mut left, None).unwrap(),
            crate::usecases::sync::read::sync_manifest(&mut right, None).unwrap()
        );
    }

    #[test]
    fn dedupe_leaves_unstamped_imports_and_automations() {
        let (_dir, mut left, _right) = two_devices();
        let cash = cash_id(&left);
        for (index, origin) in [
            TxOrigin::Import {
                exec_id: "exec-1".into(),
            },
            TxOrigin::Automation {
                automation_id: "rent".into(),
                planned_at: "2026-11-01".into(),
            },
        ]
        .into_iter()
        .enumerate()
        {
            for copy in 0..2 {
                let mut tx = income(&cash, "100", &format!("{index}-{copy}"));
                if let Transaction::Topup(topup) = &mut tx {
                    topup.meta.origin = origin.clone();
                }
                left.doc.transactions.push(tx);
            }
        }
        left.doc.sync.stamps.clear();
        left.dedupe_generated("no-batch", true).unwrap();
        assert_eq!(
            left.doc.transactions.len(),
            4,
            "an unstamped one was deleted"
        );
    }

    #[test]
    fn undo_after_dedupe_restores_both_copies() {
        // Both copies are here when an unrelated batch arrives.
        let (_dir, mut left, mut right) = two_devices();
        rent(&mut left, "first");
        pause();
        let late = rent(&mut left, "second");
        pause();
        edit_comment(&mut left, &late, "second, edited");
        right.doc.stores[0].color = "#123456".into();
        right.persist().unwrap();
        let (applied_left, _) = exchange(&mut left, &mut right);
        let batch = applied_left.expect("left applied the store change");
        assert_eq!(comments(&left), vec!["second, edited"]);
        let report = left.sync_undo(&batch.id).unwrap();
        assert!(report.skipped.is_empty(), "{:?}", report.skipped);
        assert_eq!(comments(&left), vec!["first", "second, edited"]);
        // Dedupe runs again with the next batch, but the undone copy carries
        // the «keep» mark (stage M): it stays, on both devices. Taking the
        // copy off again is «Allow merging» (`keep_tests.rs`).
        right.doc.stores[0].name = "Renamed".into();
        right.persist().unwrap();
        settle(&mut left, &mut right);
        assert_eq!(comments(&left), vec!["first", "second, edited"]);
        assert_eq!(comments(&right), comments(&left));

        // The other copy came in the batch itself: undo leaves the local one.
        let (_dir, mut left, mut right) = two_devices();
        rent(&mut right, "rent");
        pause();
        rent(&mut left, "rent");
        let (applied_left, _) = exchange(&mut left, &mut right);
        let batch = applied_left.expect("left took the earlier copy");
        assert_eq!(left.doc.transactions.len(), 1);
        left.sync_undo(&batch.id).unwrap();
        assert_eq!(left.doc.transactions.len(), 1, "undo left neither copy");
    }

    #[test]
    fn a_dedupe_deletion_is_in_the_batch_collections_but_not_in_the_summary() {
        let (_dir, mut left, mut right) = two_devices();
        rent(&mut left, "first");
        pause();
        rent(&mut left, "second");
        right.doc.stores[0].color = "#123456".into();
        right.persist().unwrap();
        let (applied_left, _) = exchange(&mut left, &mut right);
        let batch = applied_left.expect("left applied the store change");
        assert_eq!(left.doc.transactions.len(), 1, "dedupe removed a copy");
        assert!(
            batch
                .summary
                .collections
                .iter()
                .all(|entry| entry.collection != "transactions"),
            "the summary is taken before the dedupe"
        );
        assert_eq!(batch.summary.total, 1, "only the store change is counted");
        let collections = crate::sync::history::batch_collections(&left.doc, &batch.id);
        assert!(
            collections.contains(&"transactions".to_string()),
            "{collections:?}"
        );
        assert!(
            collections.contains(&"stores".to_string()),
            "{collections:?}"
        );
        let mut sorted = collections.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(collections, sorted);
    }

    /// A workflow transaction of `run`, planned for `moment`.
    fn workflow_tx(vault: &mut UnlockedVault, run: &str, comment: &str) {
        let cash = cash_id(vault);
        let mut tx = income(&cash, "100", comment);
        if let Transaction::Topup(topup) = &mut tx {
            topup.meta.origin = TxOrigin::Workflow {
                workflow_id: "salary".into(),
                run_id: run.into(),
                planned_at: Some("2026-11-01".into()),
                planned_date: None,
                estimated: false,
                run_key: None,
            };
        }
        crate::usecases::money::transactions::add_transaction(vault, tx).unwrap();
    }

    /// Makes both devices apply something, so the dedupe runs on each.
    fn touch_and_settle(left: &mut UnlockedVault, right: &mut UnlockedVault) {
        left.doc.stores[0].name = "Left".into();
        left.persist().unwrap();
        right.doc.stores[0].color = "#123456".into();
        right.persist().unwrap();
        settle(left, right);
    }

    #[test]
    fn one_workflow_run_with_several_operations_for_one_moment_keeps_them_all() {
        let (_dir, mut left, mut right) = two_devices();
        workflow_tx(&mut left, "run-1", "salary");
        workflow_tx(&mut left, "run-1", "tax");
        touch_and_settle(&mut left, &mut right);
        assert_eq!(comments(&left), vec!["salary", "tax"]);
        assert_eq!(comments(&right), comments(&left));
    }

    #[test]
    fn two_runs_on_one_device_for_one_moment_are_both_kept() {
        let (_dir, mut left, mut right) = two_devices();
        workflow_tx(&mut left, "run-1", "first");
        workflow_tx(&mut left, "run-2", "second");
        touch_and_settle(&mut left, &mut right);
        assert_eq!(comments(&left), vec!["first", "second"]);
        assert_eq!(comments(&right), comments(&left));
    }

    #[test]
    fn the_same_moment_run_on_two_devices_keeps_the_earlier_run_whole() {
        let (_dir, mut left, mut right) = two_devices();
        workflow_tx(&mut left, "run-left", "left salary");
        workflow_tx(&mut left, "run-left", "left tax");
        std::thread::sleep(std::time::Duration::from_millis(5));
        workflow_tx(&mut right, "run-right", "right salary");
        workflow_tx(&mut right, "run-right", "right tax");
        settle(&mut left, &mut right);
        assert_eq!(comments(&left), vec!["left salary", "left tax"]);
        assert_eq!(comments(&right), comments(&left));
    }

    #[test]
    fn a_deletion_that_arrives_late_survives_the_compaction_of_the_same_apply() {
        let (_dir, mut left, _right) = two_devices();
        let old = crate::sync::stamp::now_ms() - 200 * 24 * 3600 * 1000;
        // The record is still here (stamped before the deletion), so the
        // deletion is news; one for an unknown key is dropped (see
        // `tombstone_tests`).
        let contact = crate::usecases::household::contacts::create_contact(
            &mut left,
            crate::household::contacts::ContactDraft {
                name: "Gone".into(),
                kind: crate::model::contact::ContactKind::Person,
                note: String::new(),
                accounts: Vec::new(),
                group: String::new(),
            },
        )
        .unwrap();
        let key = format!("contacts:{}", contact.id);
        left.doc.sync.stamps.insert(
            key.clone(),
            crate::sync::RecordStamps::created(Stamp::new(old - 1000, 0, "aaaaaaaa")),
        );
        left.persist().unwrap();
        let incoming: BTreeMap<String, RecordState> = [(
            key.clone(),
            RecordState {
                value: None,
                stamps: None,
                tombstone: Some(Stamp::new(old, 0, "bbbbbbbb")),
            },
        )]
        .into();
        left.sync_apply(&incoming, &peer("right"))
            .unwrap()
            .expect("taken");
        assert!(left.doc.sync.tombstones.contains_key(&key));
        assert!(left.doc.sync.tombstones_seen.contains_key(&key));
        // The bookkeeping is not part of what is described to others.
        assert!(
            crate::usecases::sync::read::sync_manifest(&mut left, None)
                .unwrap()
                .contains_key(&key)
        );
    }

    #[test]
    fn a_compacted_deletion_takes_its_seen_time_with_it() {
        let (_dir, mut left, _right) = two_devices();
        let now = crate::sync::stamp::now_ms();
        let old = now - 400 * 24 * 3600 * 1000;
        let key = "contacts:gone".to_string();
        left.doc
            .sync
            .tombstones
            .insert(key.clone(), Stamp::new(old, 0, "bbbbbbbb"));
        left.doc.sync.tombstones_seen.insert(key.clone(), old);
        left.compact_tombstones();
        assert!(!left.doc.sync.tombstones.contains_key(&key));
        assert!(!left.doc.sync.tombstones_seen.contains_key(&key));
    }

    #[test]
    fn device_settings_never_travel() {
        let (_dir, mut left, mut right) = two_devices();
        left.doc.settings.minimize_to_tray = false;
        left.doc.settings.savings_rate = crate::Amount::parse("15").unwrap();
        left.persist().unwrap();
        settle(&mut left, &mut right);
        assert!(right.doc.settings.minimize_to_tray);
        assert_eq!(
            right.doc.settings.savings_rate,
            left.doc.settings.savings_rate
        );
    }

    fn pin(name: &str) -> crate::model::entity_ref::Pinned {
        crate::model::entity_ref::Pinned::Filter {
            id: format!("pin-{name}"),
            name: name.into(),
            query: String::new(),
        }
    }

    /// The dashboard layout, its period and the pins belong to the device:
    /// neither side takes the other's, in either direction.
    #[test]
    fn dashboard_layout_period_and_pins_never_travel() {
        use crate::model::vault::DashPeriod;
        let (_dir, mut left, mut right) = two_devices();
        left.doc.settings.dashboard_period = DashPeriod::Year;
        left.doc.settings.pinned = vec![pin("left")];
        left.doc.settings.dashboard_widgets = vec![crate::model::vault::DashboardWidgetCfg {
            id: "w-left".into(),
            col: 0,
            row: 0,
            width: 6,
            height: 4,
            pinned: false,
            store_id: None,
            period: None,
            scenario_id: None,
        }];
        left.doc.settings.savings_rate = crate::Amount::parse("15").unwrap();
        left.persist().unwrap();
        right.doc.settings.pinned = vec![pin("right")];
        right.doc.settings.dashboard_period = DashPeriod::Quarter;
        right.persist().unwrap();
        settle(&mut left, &mut right);
        assert_eq!(
            right.doc.settings.savings_rate,
            left.doc.settings.savings_rate
        );
        assert_eq!(left.doc.settings.dashboard_period, DashPeriod::Year);
        assert_eq!(right.doc.settings.dashboard_period, DashPeriod::Quarter);
        assert_eq!(left.doc.settings.pinned, vec![pin("left")]);
        assert_eq!(right.doc.settings.pinned, vec![pin("right")]);
        assert_eq!(left.doc.settings.dashboard_widgets.len(), 1);
        assert!(right.doc.settings.dashboard_widgets.is_empty());
    }

    /// A device from before the change still sends these fields, with a newer
    /// stamp: they change nothing here.
    #[test]
    fn a_settings_record_from_an_older_build_leaves_layout_and_pins_alone() {
        use crate::model::vault::DashPeriod;
        let (_dir, mut left, mut right) = two_devices();
        right.doc.settings.pinned = vec![pin("mine")];
        right.persist().unwrap();
        let mut records =
            crate::usecases::sync::read::sync_records(&mut left, &["settings:".to_string()])
                .unwrap();
        let state = records.get_mut("settings:").unwrap();
        let value = state.value.as_mut().unwrap().as_object_mut().unwrap();
        value.insert("dashboard_period".into(), serde_json::json!("year"));
        value.insert(
            "pinned".into(),
            serde_json::json!([{
                "type": "filter", "id": "theirs", "name": "theirs", "query": ""
            }]),
        );
        value.insert("savings_rate".into(), serde_json::json!("0.33"));
        let later = Stamp::new(u64::MAX / 4, 0, "aaaaaaaa");
        let stamps = state.stamps.as_mut().unwrap();
        for field in ["dashboard_period", "pinned", "savings_rate"] {
            stamps.fields.insert(field.into(), later.clone());
        }
        right.sync_apply(&records, &peer("left")).unwrap();
        assert_eq!(
            right.doc.settings.savings_rate,
            crate::Amount::parse("0.33").unwrap()
        );
        assert_eq!(right.doc.settings.dashboard_period, DashPeriod::default());
        assert_eq!(right.doc.settings.pinned, vec![pin("mine")]);
    }

    #[test]
    fn a_record_under_a_foreign_key_is_refused() {
        let (_dir, mut left, mut right) = two_devices();
        let forged_key = format!("stores:{}", cash_id(&right));
        let mut forged =
            crate::usecases::sync::read::sync_records(&mut right, &[forged_key]).unwrap();
        let state = forged.values().next().unwrap().clone();
        forged.clear();
        forged.insert("stores:someone-else".into(), state);
        assert!(matches!(
            left.sync_apply(&forged, &peer("right")),
            Err(Error::InvalidInput(_))
        ));
        forged.clear();
        forged.insert("holdings:x".into(), RecordState::default());
        assert!(crate::usecases::sync::read::sync_preview(&mut left, &forged).is_err());
    }

    #[test]
    fn a_record_that_does_not_fit_the_schema_changes_nothing() {
        let (_dir, mut left, _right) = two_devices();
        let before = left.doc.clone();
        let broken: BTreeMap<String, RecordState> = [(
            "stores:bad".to_string(),
            RecordState {
                value: Some(serde_json::json!({"id": "bad", "kind": 42})),
                stamps: Some(Default::default()),
                tombstone: None,
            },
        )]
        .into();
        // The record is skipped (told in the journal), not an error that ends
        // the session; the data stays as it was.
        assert!(left.sync_apply(&broken, &peer("right")).unwrap().is_none());
        let mut after = left.doc.clone();
        after.sync = before.sync.clone();
        assert_eq!(after, before);
    }

    #[test]
    fn json_editor_edits_are_stamped_and_keep_the_bookkeeping() {
        let (_dir, mut left, mut right) = two_devices();
        crate::usecases::sync::trust::sync_trust_peer(&mut left, "dev-b", "Phone", "aa").unwrap();
        let mut edited = left.doc.clone();
        edited.stores[0].name = "Edited".into();
        edited.sync = Default::default();
        left.replace_doc(edited).unwrap();
        assert_eq!(left.sync_state().peers.len(), 1);
        settle(&mut left, &mut right);
        assert_eq!(right.doc.stores[0].name, "Edited");
    }

    #[test]
    fn forgetting_a_device_reaches_the_others() {
        let (_dir, mut left, mut right) = two_devices();
        crate::usecases::sync::trust::sync_trust_peer(&mut left, "dev-c", "Tablet", "cc").unwrap();
        right
            .sync_merge_peers(&left.sync_state().peers.clone())
            .unwrap();
        assert!(right.sync_state().trusted("cc").is_some());
        crate::usecases::sync::trust::sync_forget_peer(&mut left, "dev-c").unwrap();
        right
            .sync_merge_peers(&left.sync_state().peers.clone())
            .unwrap();
        assert!(right.sync_state().trusted("cc").is_none());
        assert!(right.sync_state().is_revoked("cc"));
    }

    #[test]
    fn trusting_a_forgotten_device_again_clears_the_revocation() {
        let (_dir, mut left, _right) = two_devices();
        crate::usecases::sync::trust::sync_trust_peer(&mut left, "dev-c", "Tablet", "cc").unwrap();
        crate::usecases::sync::trust::sync_forget_peer(&mut left, "dev-c").unwrap();
        assert!(left.sync_state().is_revoked("cc"));
        crate::usecases::sync::trust::sync_trust_peer(&mut left, "dev-c", "Tablet", "cc").unwrap();
        assert!(!left.sync_state().is_revoked("cc"));
        assert!(left.sync_state().trusted("cc").is_some());
    }

    #[test]
    fn an_older_revoked_entry_under_another_device_id_does_not_revoke_a_new_trust() {
        let (_dir, mut left, _right) = two_devices();
        crate::usecases::sync::trust::sync_trust_peer(&mut left, "dev-old", "Tablet", "cc")
            .unwrap();
        crate::usecases::sync::trust::sync_forget_peer(&mut left, "dev-old").unwrap();
        crate::usecases::sync::trust::sync_trust_peer(&mut left, "dev-new", "Tablet", "cc")
            .unwrap();
        assert!(!left.sync_state().is_revoked("cc"));
        assert_eq!(
            left.sync_state().trusted("cc").unwrap().device_id,
            "dev-new"
        );
    }

    #[test]
    fn a_merged_older_revocation_does_not_beat_a_newer_trust() {
        let (_dir, mut left, mut right) = two_devices();
        crate::usecases::sync::trust::sync_trust_peer(&mut left, "dev-c", "Tablet", "cc").unwrap();
        crate::usecases::sync::trust::sync_forget_peer(&mut left, "dev-c").unwrap();
        let revoked = left.sync_state().peers.clone();
        right.sync_merge_peers(&revoked).unwrap();
        crate::usecases::sync::trust::sync_trust_peer(&mut right, "dev-c2", "Tablet", "cc")
            .unwrap();
        right.sync_merge_peers(&revoked).unwrap();
        assert!(right.sync_state().trusted("cc").is_some());
        assert!(!right.sync_state().is_revoked("cc"));
    }

    #[test]
    fn pairing_after_a_merged_future_revocation_still_sorts_newer() {
        let (_dir, mut left, _right) = two_devices();
        let ahead = crate::sync::stamp::now_ms() + 3_600_000;
        let revoked = TrustedPeer {
            device_id: "dev-c".into(),
            public_key: "cc".into(),
            revoked: Some(Stamp::new(ahead, 0, "remote")),
            stamp: Stamp::new(ahead, 0, "remote"),
            ..TrustedPeer::default()
        };
        assert!(left.sync_merge_peers(&[revoked]).unwrap());
        assert!(left.sync_state().is_revoked("cc"));
        crate::usecases::sync::trust::sync_trust_peer(&mut left, "dev-c", "Tablet", "cc").unwrap();
        assert!(left.sync_state().trusted("cc").is_some());
    }

    #[test]
    fn forgetting_the_newest_entry_still_revokes_the_key() {
        let (_dir, mut left, _right) = two_devices();
        crate::usecases::sync::trust::sync_trust_peer(&mut left, "dev-old", "Tablet", "cc")
            .unwrap();
        crate::usecases::sync::trust::sync_forget_peer(&mut left, "dev-old").unwrap();
        crate::usecases::sync::trust::sync_trust_peer(&mut left, "dev-new", "Tablet", "cc")
            .unwrap();
        crate::usecases::sync::trust::sync_forget_peer(&mut left, "dev-new").unwrap();
        assert!(left.sync_state().is_revoked("cc"));
        assert!(left.sync_state().trusted("cc").is_none());
    }

    #[test]
    fn the_backup_restores_the_vault_as_before_the_sync() {
        let (_dir, mut left, mut right) = two_devices();
        let cash = cash_id(&left);
        crate::usecases::money::transactions::add_transaction(
            &mut right,
            income(&cash, "20", "theirs"),
        )
        .unwrap();
        left.sync_backup().unwrap();
        exchange(&mut left, &mut right);
        assert_eq!(comments(&left), vec!["theirs"]);
        left.sync_restore_backup().unwrap();
        assert!(left.doc.transactions.is_empty());
        settle(&mut left, &mut right);
        assert!(right.doc.transactions.is_empty());
    }

    #[test]
    fn a_vault_from_before_sync_opens_and_starts_stamping() {
        let (dir, mut vault) = empty_vault();
        vault.doc.sync = Default::default();
        vault.persist().unwrap();
        let path = vault.path.clone();
        drop(vault);
        let mut reopened = UnlockedVault::open(path, "pw").unwrap();
        assert!(reopened.sync_state().stamps.is_empty());
        reopened.doc.stores[0].name = "X".into();
        reopened.persist().unwrap();
        assert_eq!(reopened.sync_state().stamps.len(), 1);
        drop(dir);
    }

    #[test]
    fn handing_the_vault_over_needs_its_password_and_leaves_a_trace() {
        let (_dir, mut vault) = empty_vault();
        assert!(vault.check_password("pw").is_ok());
        assert!(matches!(vault.check_password("pw "), Err(Error::Secret)));
        assert!(matches!(vault.check_password(""), Err(Error::Secret)));

        let notice = vault
            .sync_record_transfer("phone", "Сочный Апельсин", "ab", true)
            .unwrap();
        assert_eq!(
            notice.category,
            crate::model::notify::NoticeCategory::Security
        );
        assert!(notice.body.contains("Сочный Апельсин"));
        // Every linked device gets the notice (it is in the synced log)...
        assert!(
            vault
                .doc
                .notification_log
                .iter()
                .any(|record| record.key == notice.key)
        );
        // ...the new device may sync...
        assert!(vault.sync_state().trusted("ab").is_some());
        // ...and the history says where the vault went.
        let history = crate::sync::history::history(&vault.doc);
        assert_eq!(
            history[0].transferred_to.as_deref(),
            Some("Сочный Апельсин")
        );
    }
    #[test]
    fn a_pause_survives_a_reopen_and_is_never_a_peer_field() {
        let (dir, mut vault) = empty_vault();
        assert!(!vault.sync_is_paused("dev-a"));
        vault.sync_pause_peer("dev-a").unwrap();
        // Pausing twice changes nothing.
        vault.sync_pause_peer("dev-a").unwrap();
        assert!(vault.sync_is_paused("dev-a"));
        assert!(!vault.sync_is_paused("dev-b"));
        let reopened = UnlockedVault::open(vault.path.clone(), "pw").unwrap();
        assert!(reopened.sync_is_paused("dev-a"));
        // The list that goes to other devices does not carry it.
        assert!(reopened.sync_state().peers.is_empty());
        let mut reopened = reopened;
        reopened.sync_resume_peer("dev-a").unwrap();
        assert!(!reopened.sync_is_paused("dev-a"));
        let again = UnlockedVault::open(reopened.path.clone(), "pw").unwrap();
        assert!(!again.sync_is_paused("dev-a"));
        drop(dir);
    }

    #[test]
    fn forgetting_a_device_lifts_its_pause() {
        let (_dir, mut vault) = empty_vault();
        crate::usecases::sync::trust::sync_trust_peer(&mut vault, "dev-a", "A", "key-a").unwrap();
        vault.sync_pause_peer("dev-a").unwrap();
        crate::usecases::sync::trust::sync_forget_peer(&mut vault, "dev-a").unwrap();
        assert!(!vault.sync_is_paused("dev-a"));
    }

    #[test]
    fn merging_a_peer_list_leaves_the_pause_alone() {
        let (_dir, mut vault) = empty_vault();
        crate::usecases::sync::trust::sync_trust_peer(&mut vault, "dev-a", "A", "key-a").unwrap();
        vault.sync_pause_peer("dev-a").unwrap();
        let remote = vault.sync_state().peers.clone();
        vault.sync_merge_peers(&remote).unwrap();
        assert!(vault.sync_is_paused("dev-a"));
    }

    #[test]
    fn cut_by_is_saved_once_survives_a_reopen_and_is_never_a_peer_field() {
        let (dir, mut vault) = empty_vault();
        assert_eq!(vault.sync_cut_reason("dev-a"), None);
        assert!(vault.sync_cut_by("dev-a", SyncCutReason::Paused).unwrap());
        // The same news again changes nothing; another reason does.
        assert!(!vault.sync_cut_by("dev-a", SyncCutReason::Paused).unwrap());
        let reopened = UnlockedVault::open(vault.path.clone(), "pw").unwrap();
        assert_eq!(
            reopened.sync_cut_reason("dev-a"),
            Some(SyncCutReason::Paused)
        );
        assert!(reopened.sync_state().peers.is_empty());
        let mut reopened = reopened;
        assert!(
            reopened
                .sync_cut_by("dev-a", SyncCutReason::Forgotten)
                .unwrap()
        );
        // A merged list leaves it alone.
        let remote = reopened.sync_state().peers.clone();
        reopened.sync_merge_peers(&remote).unwrap();
        assert_eq!(
            reopened.sync_cut_reason("dev-a"),
            Some(SyncCutReason::Forgotten)
        );
        assert!(reopened.sync_clear_cut("dev-a").unwrap());
        assert!(!reopened.sync_clear_cut("dev-a").unwrap());
        drop(dir);
    }

    #[test]
    fn pairing_or_forgetting_a_device_clears_what_it_told_us() {
        let (_dir, mut vault) = empty_vault();
        vault
            .sync_cut_by("dev-a", SyncCutReason::Forgotten)
            .unwrap();
        crate::usecases::sync::trust::sync_trust_peer(&mut vault, "dev-a", "A", "key-a").unwrap();
        assert_eq!(vault.sync_cut_reason("dev-a"), None);
        vault.sync_cut_by("dev-a", SyncCutReason::Paused).unwrap();
        crate::usecases::sync::trust::sync_forget_peer(&mut vault, "dev-a").unwrap();
        assert_eq!(vault.sync_cut_reason("dev-a"), None);
    }
}

/// The security level in synchronisation (spec E6, decision R3): a device
/// below level 3 still receives everything that is not the crypto module's,
/// and the module's records never leave the sender for it.
#[cfg(test)]
mod level_pair_tests {
    use super::tests::two_devices;
    use super::*;
    use crate::error::Error;
    use crate::model::chain::Chain;
    use crate::model::cryptotrack::CryptoAccount;
    use crate::model::store::StoreKind;

    const ADDRESS: &str = "0x00000000000000000000000000000000000000aa";

    fn peer(name: &str) -> PeerRef {
        PeerRef {
            device_id: format!("{name}-id"),
            name: name.into(),
        }
    }

    fn crypto_keys<'key>(keys: impl IntoIterator<Item = &'key String>) -> Vec<String> {
        keys.into_iter()
            .filter(|key| is_crypto_key(key))
            .cloned()
            .collect()
    }

    /// A level-3 `left` that follows an address and has a wallet of its own,
    /// and a level-2 `right` with another wallet.
    fn pair() -> (tempfile::TempDir, UnlockedVault, UnlockedVault) {
        let (dir, mut left, mut right) = two_devices();
        left.raise_to_level3_for_tests();
        assert_eq!(left.security_level(), SecurityLevel::Crypto);
        assert_ne!(right.security_level(), SecurityLevel::Crypto);
        crate::usecases::money::stores::create_store(
            &mut left,
            "FromLeft",
            StoreKind::Crypto,
            "#111",
            "",
            None,
        )
        .unwrap();
        crate::usecases::money::stores::create_store(
            &mut right,
            "FromRight",
            StoreKind::Crypto,
            "#222",
            "",
            None,
        )
        .unwrap();
        // Ordinary operations on the shared cash wallet, one from each side.
        for (vault, comment) in [(&mut left, "from left"), (&mut right, "from right")] {
            let cash = vault
                .doc
                .stores
                .iter()
                .find(|store| store.kind == StoreKind::Cash && store.parent_id.is_none())
                .map(|store| store.id.clone())
                .unwrap();
            crate::usecases::money::transactions::add_transaction(
                vault,
                super::super::test_support::income(&cash, "100", comment),
            )
            .unwrap();
        }
        left.doc.crypto_accounts.push(CryptoAccount::new(
            "acc-1",
            "Main",
            Chain::Ethereum,
            ADDRESS,
            "2026-01-01T00:00:00Z",
        ));
        (dir, left, right)
    }

    fn has_store(vault: &UnlockedVault, name: &str) -> bool {
        vault.doc.stores.iter().any(|store| store.name == name)
    }

    #[test]
    fn a_level_two_device_gets_everything_but_the_crypto_records() {
        let (_dir, mut left, mut right) = pair();
        let right_level = right.security_level();
        let left_level = left.security_level();

        // What each side puts on the wire, as the session builds it.
        let (to_right_manifest, withheld) =
            crate::usecases::sync::read::sync_manifest_for(&mut left, None, right_level).unwrap();
        let (to_left_manifest, none) =
            crate::usecases::sync::read::sync_manifest_for(&mut right, None, left_level).unwrap();
        assert!(withheld > 0, "the sender must know it held something back");
        assert_eq!(none, 0);
        // The full manifest does carry the module, so the filter is what hides it.
        assert!(
            !crypto_keys(
                crate::usecases::sync::read::sync_manifest(&mut left, None)
                    .unwrap()
                    .keys()
            )
            .is_empty()
        );
        assert!(crypto_keys(to_right_manifest.keys()).is_empty());

        let (right_wants, _) =
            crate::usecases::sync::read::sync_diff(&right, &to_left_manifest, &to_right_manifest);
        let (left_wants, _) =
            crate::usecases::sync::read::sync_diff(&left, &to_right_manifest, &to_left_manifest);
        // A peer that asks for the module's keys anyway gets nothing for them.
        let mut greedy = right_wants.clone();
        greedy.extend(crypto_keys(
            crate::usecases::sync::read::sync_manifest(&mut left, None)
                .unwrap()
                .keys(),
        ));
        let to_right =
            crate::usecases::sync::read::sync_records_for(&mut left, &greedy, right_level).unwrap();
        let to_left =
            crate::usecases::sync::read::sync_records_for(&mut right, &left_wants, left_level)
                .unwrap();
        assert!(crypto_keys(to_right.keys()).is_empty());
        assert!(to_right.keys().any(|key| key.starts_with("stores:")));

        let (kept, dropped) = right.sync_admit(to_right);
        assert_eq!(dropped, 0, "nothing should arrive that has to be dropped");
        right.sync_apply(&kept, &peer("left")).unwrap();
        left.sync_apply(&to_left, &peer("right")).unwrap();

        assert!(has_store(&right, "FromLeft"));
        assert!(has_store(&left, "FromRight"));
        // The ordinary operations of both sides reached both.
        assert_eq!(left.doc.transactions.len(), 2);
        assert_eq!(right.doc.transactions.len(), 2);
        assert!(right.doc.crypto_accounts.is_empty());
        assert_eq!(left.doc.crypto_accounts.len(), 1);
    }

    #[test]
    fn a_level_three_pair_still_exchanges_the_module() {
        let (_dir, mut left, mut right) = pair();
        right.raise_to_level3_for_tests();
        let (manifest, withheld) =
            crate::usecases::sync::read::sync_manifest_for(&mut left, None, SecurityLevel::Crypto)
                .unwrap();
        assert_eq!(withheld, 0);
        assert!(!crypto_keys(manifest.keys()).is_empty());
        let (wants, _) =
            crate::usecases::sync::read::sync_diff(&right, &Manifest::new(), &manifest);
        let records =
            crate::usecases::sync::read::sync_records_for(&mut left, &wants, SecurityLevel::Crypto)
                .unwrap();
        let (kept, dropped) = right.sync_admit(records);
        assert_eq!(dropped, 0);
        right.sync_apply(&kept, &peer("left")).unwrap();
        assert_eq!(right.doc.crypto_accounts.len(), 1);
    }

    /// The receiver does not rely on the sender: a peer that sends the module
    /// anyway has it dropped and the rest applied, and the fuse behind it still
    /// refuses a batch that was not filtered.
    #[test]
    fn a_hostile_sender_loses_only_the_crypto_records() {
        let (_dir, mut left, mut right) = pair();
        let all: Vec<String> = crate::usecases::sync::read::sync_manifest(&mut left, None)
            .unwrap()
            .keys()
            .cloned()
            .collect();
        let sent = crate::usecases::sync::read::sync_records(&mut left, &all).unwrap();
        assert!(!crypto_keys(sent.keys()).is_empty());

        assert!(matches!(
            right.sync_apply(&sent, &peer("left")),
            Err(Error::SecurityLevelRequired)
        ));
        let (kept, dropped) = right.sync_admit(sent);
        assert!(dropped > 0);
        assert!(crypto_keys(kept.keys()).is_empty());
        right.sync_apply(&kept, &peer("left")).unwrap();
        assert!(has_store(&right, "FromLeft"));
        assert!(right.doc.crypto_accounts.is_empty());
    }

    #[test]
    fn a_level_three_receiver_drops_nothing() {
        let (_dir, mut left, mut right) = pair();
        right.raise_to_level3_for_tests();
        let all: Vec<String> = crate::usecases::sync::read::sync_manifest(&mut left, None)
            .unwrap()
            .keys()
            .cloned()
            .collect();
        let sent = crate::usecases::sync::read::sync_records(&mut left, &all).unwrap();
        let count = sent.len();
        let (kept, dropped) = right.sync_admit(sent);
        assert_eq!((kept.len(), dropped), (count, 0));
    }
}
