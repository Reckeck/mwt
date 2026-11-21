//! The sync state lives in [`mwt_model`]; this path stays for callers.

pub use mwt_model::{
    BATCH_DAYS, BatchEntry, CutBy, DEVICE_TAG_LEN, MAX_BATCHES, MAX_FUTURE_MS, MAX_TRANSFERS,
    REVOKED_GATE_DAYS, RecordStamps, RecordState, Stamp, SyncBatch, SyncCutReason, SyncState,
    TOMBSTONE_DAYS, TransferRecord, TrustedPeer, acked_through, device_tag, is_own_device,
};

#[cfg(test)]
mod tests {
    /// The moved types stay reachable at the paths callers already use.
    #[test]
    fn sync_state_keeps_its_old_paths() {
        let cut = crate::state::CutBy {
            reason: mwt_model::SyncCutReason::Paused,
            at: String::new(),
        };
        let mut state = crate::SyncState::default();
        state.cut_by.insert("dev".into(), cut);
        let stamp = crate::Stamp::new(1, 0, "dev");
        assert_eq!(stamp.wall, 1);
        assert_eq!(state.cut_by.len(), 1);
    }
}
