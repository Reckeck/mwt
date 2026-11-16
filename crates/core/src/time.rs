//! Time port: a source of "now" that tests can pin.

/// A local calendar date; the key an automation occurrence is identified by.
pub type LocalDate = chrono::NaiveDate;

/// Source of the current moment, in unix seconds.
pub trait Clock {
    fn now_unix(&self) -> i64;
}

/// The wall clock.
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_unix(&self) -> i64 {
        crate::calendar::now_secs()
    }
}

/// A clock stuck at one moment.
pub struct FixedClock(pub i64);

impl Clock for FixedClock {
    fn now_unix(&self) -> i64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fixed_clock_returns_its_moment() {
        assert_eq!(FixedClock(42).now_unix(), 42);
    }

    #[test]
    fn the_system_clock_is_after_2020() {
        assert!(SystemClock.now_unix() > 1_577_836_800);
    }
}
