//! The sync state lives in [`crate::model::sync_state`]; this path stays for callers.

pub use crate::model::sync_state::*;

#[cfg(test)]
mod tests {
    /// The moved types stay reachable at the paths callers already use.
    #[test]
    fn sync_state_keeps_its_old_paths() {
        let cut = crate::sync::state::CutBy {
            reason: crate::ipc_contract::SyncCutReason::Paused,
            at: String::new(),
        };
        let mut state = crate::sync::SyncState::default();
        state.cut_by.insert("dev".into(), cut);
        let stamp = crate::sync::Stamp::new(1, 0, "dev");
        assert_eq!(stamp.wall, 1);
        assert_eq!(state.cut_by.len(), 1);
    }
}
