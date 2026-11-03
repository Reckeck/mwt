//! Hybrid logical clock: every change a device makes gets a [`Stamp`] that
//! orders it against the changes of every other device.
//!
//! A stamp is wall-clock milliseconds, a counter for changes within the same
//! millisecond (or while the wall clock lags behind a stamp already seen) and
//! a short device tag that breaks the last ties. The derived `Ord` compares
//! them in that order, so two devices always agree which change is later.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Characters of the device id kept in a stamp: enough to break ties, short
/// enough that thousands of stamps do not bloat the vault.
pub const DEVICE_TAG_LEN: usize = 8;

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Stamp {
    /// Unix milliseconds.
    #[serde(rename = "w")]
    pub wall: u64,
    #[serde(rename = "c")]
    pub count: u32,
    /// [`device_tag`] of the device that made the change.
    #[serde(rename = "d")]
    pub device: String,
}

impl Stamp {
    pub fn new(wall: u64, count: u32, device: &str) -> Self {
        Self {
            wall,
            count,
            device: device.to_string(),
        }
    }
}

/// How far ahead of the wall clock a stamp may be and still move a clock, and
/// the skew at which devices refuse to sync (`session::SKEW_REFUSE_MS` is this).
/// A record stamped beyond it is stored as it is, but never drags a clock along.
pub const MAX_FUTURE_MS: u64 = 24 * 3600 * 1000;

/// The part of a device id a stamp carries.
pub fn device_tag(device_id: &str) -> String {
    device_id
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(DEVICE_TAG_LEN)
        .collect()
}

/// Unix milliseconds now; a clock before 1970 reads as 0.
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

/// The clock of one device for one open vault.
#[derive(Debug, Clone, Default)]
pub struct Clock {
    wall: u64,
    count: u32,
    device: String,
}

impl Clock {
    pub fn new(device_id: &str) -> Self {
        Self {
            device: device_tag(device_id),
            ..Self::default()
        }
    }

    pub fn set_device(&mut self, device_id: &str) {
        self.device = device_tag(device_id);
    }

    pub fn device(&self) -> &str {
        &self.device
    }

    /// A stamp for a change made now.
    pub fn tick(&mut self) -> Stamp {
        self.tick_at(now_ms())
    }

    /// A stamp for a change made at `now` (ms); never below one already
    /// issued or observed, even when the wall clock went back.
    pub fn tick_at(&mut self, now: u64) -> Stamp {
        if now > self.wall {
            self.wall = now;
            self.count = 0;
        } else {
            self.count = self.count.saturating_add(1);
        }
        Stamp::new(self.wall, self.count, &self.device)
    }

    /// Take in a stamp from elsewhere, so the next local change sorts after it.
    /// A stamp further ahead of the wall clock than [`MAX_FUTURE_MS`] is not
    /// followed: one device with a wrong date must not poison every other.
    pub fn observe(&mut self, stamp: &Stamp) {
        self.observe_at(stamp, now_ms());
    }

    /// [`Clock::observe`] with an explicit `now` (ms).
    pub fn observe_at(&mut self, stamp: &Stamp, now: u64) {
        if stamp.wall > now.saturating_add(MAX_FUTURE_MS) {
            return;
        }
        if (stamp.wall, stamp.count) > (self.wall, self.count) {
            self.wall = stamp.wall;
            self.count = stamp.count;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamps_order_by_time_then_counter_then_device() {
        assert!(Stamp::new(1, 9, "b") < Stamp::new(2, 0, "a"));
        assert!(Stamp::new(2, 0, "b") < Stamp::new(2, 1, "a"));
        assert!(Stamp::new(2, 1, "a") < Stamp::new(2, 1, "b"));
    }

    #[test]
    fn clock_never_goes_back() {
        let mut clock = Clock::new("aaaaaaaa-1111");
        let first = clock.tick_at(100);
        let same_ms = clock.tick_at(100);
        let behind = clock.tick_at(50);
        assert!(first < same_ms && same_ms < behind);
        assert_eq!(behind.wall, 100);
        assert_eq!(first.device, "aaaaaaaa");
    }

    #[test]
    fn observed_stamps_push_the_clock_forward() {
        let mut clock = Clock::new("a");
        clock.observe(&Stamp::new(500, 3, "zz"));
        let next = clock.tick_at(10);
        assert!(next > Stamp::new(500, 3, "zz"));
    }

    #[test]
    fn a_stamp_beyond_the_limit_is_not_followed() {
        let mut clock = Clock::new("a");
        clock.observe_at(&Stamp::new(1_000 + MAX_FUTURE_MS + 1, 0, "zz"), 1_000);
        let next = clock.tick_at(1_000);
        assert_eq!(next.wall, 1_000);
    }

    #[test]
    fn a_stamp_at_the_limit_is_followed() {
        let mut clock = Clock::new("a");
        clock.observe_at(&Stamp::new(1_000 + MAX_FUTURE_MS, 0, "zz"), 1_000);
        assert_eq!(clock.tick_at(1_000).wall, 1_000 + MAX_FUTURE_MS);
    }
}
