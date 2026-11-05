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
}
