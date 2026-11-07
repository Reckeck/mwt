//! When the vault locks after the app has been in the background (Android).
//! Pure, so it is tested on every platform; only `background.rs` decides.
#![cfg_attr(not(target_os = "android"), allow(dead_code))]

use std::time::{Duration, Instant, SystemTime};

/// Where the app went to the background: both clocks, because the monotonic
/// one stops in deep sleep and the wall clock can jump.
#[derive(Clone, Copy)]
pub struct Since {
    pub mono: Instant,
    pub wall: SystemTime,
}

impl Since {
    pub fn now() -> Self {
        Self {
            mono: Instant::now(),
            wall: SystemTime::now(),
        }
    }

    /// Time spent in the background: the larger of the two clocks, so a deep
    /// sleep counts; a clock that went backwards counts as zero.
    pub fn elapsed(&self) -> Duration {
        let mono = Instant::now().saturating_duration_since(self.mono);
        let wall = SystemTime::now()
            .duration_since(self.wall)
            .unwrap_or_default();
        mono.max(wall)
    }
}

/// Lock now? `idle_for` is the time since the last sync activity (`None`
/// when there was none); activity newer than the start of the background
/// period shortens the idle time. `minutes == 0` never locks.
pub fn lock_due(background_for: Duration, idle_for: Option<Duration>, minutes: u32) -> bool {
    if minutes == 0 {
        return false;
    }
    let idle = idle_for.map_or(background_for, |idle| idle.min(background_for));
    idle >= Duration::from_secs(u64::from(minutes) * 60)
}

/// What the vault does when the app is in the background.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackgroundLimit {
    /// Stays open.
    Never,
    /// Locks at once.
    Now,
    /// Locks after this many minutes (`lock_due`).
    After(u32),
}

/// Everything the background decision reads.
#[derive(Debug, Clone, Copy)]
pub struct BackgroundInput {
    /// Background sync runs.
    pub sync_armed: bool,
    /// "Stop" was pressed on the sync notification.
    pub sync_paused: bool,
    /// `SyncPrefs::background_lock_min`; `0` = never.
    pub sync_minutes: u32,
    /// The autolock minutes while the autolock is on, `None` while it is off.
    pub autolock_minutes: Option<u32>,
    /// The device holds exchange keys (the autolock cannot be off then).
    pub has_device_keys: bool,
}

/// The decision for the time in the background. With the autolock off it is
/// the old sync rule, untouched. With it on: no running sync means nothing to
/// protect by staying open, so lock at once; a running sync decides by its own
/// limit so it is not cut off, but with keys on the device never later than
/// the autolock minutes.
pub fn background_limit(input: BackgroundInput) -> BackgroundLimit {
    let sync_limit = match input.sync_minutes {
        0 => BackgroundLimit::Never,
        minutes => BackgroundLimit::After(minutes),
    };
    let Some(autolock) = input.autolock_minutes else {
        return if input.sync_armed || input.sync_paused {
            sync_limit
        } else {
            BackgroundLimit::Never
        };
    };
    if !input.sync_armed {
        return BackgroundLimit::Now;
    }
    match (sync_limit, input.has_device_keys) {
        (limit, false) => limit,
        (BackgroundLimit::After(minutes), true) => BackgroundLimit::After(minutes.min(autolock)),
        (_, true) => BackgroundLimit::After(autolock),
    }
}

/// Lock now? `idle_for` as in [`lock_due`].
pub fn background_lock_due(
    limit: BackgroundLimit,
    background_for: Duration,
    idle_for: Option<Duration>,
) -> bool {
    match limit {
        BackgroundLimit::Never => false,
        BackgroundLimit::Now => true,
        BackgroundLimit::After(minutes) => lock_due(background_for, idle_for, minutes),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINUTE: Duration = Duration::from_secs(60);

    #[test]
    fn zero_minutes_never_locks() {
        assert!(!lock_due(MINUTE * 1000, None, 0));
    }

    #[test]
    fn locks_after_the_limit_and_not_before() {
        assert!(!lock_due(MINUTE * 4, None, 5));
        assert!(lock_due(MINUTE * 5, None, 5));
    }

    #[test]
    fn recent_activity_resets_the_idle_time() {
        assert!(!lock_due(MINUTE * 10, Some(MINUTE), 5));
        assert!(lock_due(MINUTE * 10, Some(MINUTE * 6), 5));
    }

    #[test]
    fn activity_older_than_the_background_period_is_ignored() {
        assert!(!lock_due(MINUTE, Some(MINUTE * 60), 5));
    }

    #[test]
    fn wall_clock_counts_a_deep_sleep() {
        let since = Since {
            mono: Instant::now(),
            wall: SystemTime::now() - MINUTE * 30,
        };
        assert!(since.elapsed() >= MINUTE * 30);
        assert!(lock_due(since.elapsed(), None, 5));
    }

    #[test]
    fn activity_before_a_deep_sleep_counts_the_sleep_as_idle() {
        let went_away = Since {
            mono: Instant::now(),
            wall: SystemTime::now() - MINUTE * 40,
        };
        let last_activity = Since {
            mono: Instant::now(),
            wall: SystemTime::now() - MINUTE * 30,
        };
        assert!(lock_due(
            went_away.elapsed(),
            Some(last_activity.elapsed()),
            5
        ));
    }

    #[test]
    fn clock_moved_backwards_does_not_panic() {
        let since = Since {
            mono: Instant::now(),
            wall: SystemTime::now() + MINUTE * 30,
        };
        assert!(since.elapsed() < MINUTE);
        assert!(!lock_due(since.elapsed(), None, 5));
    }

    fn input(armed: bool, paused: bool, autolock: Option<u32>, keys: bool) -> BackgroundInput {
        BackgroundInput {
            sync_armed: armed,
            sync_paused: paused,
            sync_minutes: 30,
            autolock_minutes: autolock,
            has_device_keys: keys,
        }
    }

    #[test]
    fn background_without_sync_locks_at_once() {
        assert_eq!(
            background_limit(input(false, false, Some(15), false)),
            BackgroundLimit::Now
        );
        // The sync was stopped from the notification: nothing to keep open for.
        assert_eq!(
            background_limit(input(false, true, Some(15), true)),
            BackgroundLimit::Now
        );
        assert!(background_lock_due(
            BackgroundLimit::Now,
            Duration::ZERO,
            None
        ));
    }

    #[test]
    fn running_sync_decides_by_the_sync_limit() {
        assert_eq!(
            background_limit(input(true, false, Some(15), false)),
            BackgroundLimit::After(30)
        );
        let mut never = input(true, false, Some(15), false);
        never.sync_minutes = 0;
        assert_eq!(background_limit(never), BackgroundLimit::Never);
    }

    #[test]
    fn keys_cap_the_sync_limit_by_the_autolock_minutes() {
        assert_eq!(
            background_limit(input(true, false, Some(15), true)),
            BackgroundLimit::After(15)
        );
        // A shorter sync limit stays.
        assert_eq!(
            background_limit(input(true, false, Some(45), true)),
            BackgroundLimit::After(30)
        );
        // "Never" cannot outlive the cap.
        let mut never = input(true, false, Some(20), true);
        never.sync_minutes = 0;
        assert_eq!(background_limit(never), BackgroundLimit::After(20));
    }

    #[test]
    fn autolock_off_keeps_the_old_sync_rule() {
        assert_eq!(
            background_limit(input(true, false, None, false)),
            BackgroundLimit::After(30)
        );
        assert_eq!(
            background_limit(input(false, true, None, false)),
            BackgroundLimit::After(30)
        );
        assert_eq!(
            background_limit(input(false, false, None, false)),
            BackgroundLimit::Never
        );
    }

    #[test]
    fn limits_compare_against_the_time_in_the_background() {
        assert!(!background_lock_due(
            BackgroundLimit::After(15),
            MINUTE * 14,
            None
        ));
        assert!(background_lock_due(
            BackgroundLimit::After(15),
            MINUTE * 15,
            None
        ));
        assert!(!background_lock_due(
            BackgroundLimit::Never,
            MINUTE * 1000,
            None
        ));
    }
}
