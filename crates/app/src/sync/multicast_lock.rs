//! The Wi-Fi multicast lock is one lock on the Android side (the plugin is not
//! reference counted: any release drops it), but three things need it: sync,
//! the receiving screen and the nearby-devices search. Each takes and gives
//! back its own share here; the plugin hears only about the first take and the
//! last release. On other systems there is no lock, only the count.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use tauri::AppHandle;

static HOLDERS: AtomicUsize = AtomicUsize::new(0);
static SYNC_HOLDS: AtomicBool = AtomicBool::new(false);

/// Count one more holder (`on`) or one less. `Some(state)` is the change the
/// lock itself must make; `None` means it stays as it is. Releasing with no
/// holder left is ignored (the count never goes below zero).
pub(super) fn step(holders: &AtomicUsize, on: bool) -> Option<bool> {
    if on {
        (holders.fetch_add(1, Ordering::SeqCst) == 0).then_some(true)
    } else {
        let previous = holders
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |count| {
                count.checked_sub(1)
            })
            .ok()?;
        (previous == 1).then_some(false)
    }
}

/// One holder takes (`on`) or gives back its share of the lock.
pub(crate) fn hold(app: &AppHandle, on: bool) {
    let Some(change) = step(&HOLDERS, on) else {
        return;
    };
    #[cfg(target_os = "android")]
    crate::platform_net::multicast(app, change);
    #[cfg(not(target_os = "android"))]
    let _ = (app, change);
}

/// The sync run's share. Idempotent: a second take, or a release without a
/// take, changes nothing, so restarts cannot leak or underflow it.
pub(crate) fn hold_for_sync(app: &AppHandle, on: bool) {
    if SYNC_HOLDS.swap(on, Ordering::SeqCst) != on {
        hold(app, on);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lock_changes_only_on_the_first_take_and_the_last_release() {
        let holders = AtomicUsize::new(0);
        assert_eq!(step(&holders, true), Some(true));
        assert_eq!(step(&holders, true), None);
        assert_eq!(step(&holders, false), None);
        assert_eq!(step(&holders, false), Some(false));
    }

    #[test]
    fn a_release_without_a_take_never_underflows() {
        let holders = AtomicUsize::new(0);
        assert_eq!(step(&holders, false), None);
        assert_eq!(holders.load(Ordering::SeqCst), 0);
        assert_eq!(step(&holders, true), Some(true));
    }
}
