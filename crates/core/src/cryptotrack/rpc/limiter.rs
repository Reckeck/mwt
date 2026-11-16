use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use tokio::time::Instant;

#[derive(Debug)]
struct HostState {
    /// Earliest moment the next request to this host may start.
    next_free: Instant,
    /// Who the last request on this host was for.
    last_subject: Option<String>,
}

/// Spaces requests per host.
///
/// Every request reserves a slot: the slot is at least `min_interval` after
/// the previous one, and when the subject differs from the previous request's
/// subject a random extra gap is added, so two addresses of one owner are not
/// sent to a provider back to back.
#[derive(Debug)]
pub struct HostLimiter {
    min_interval: Duration,
    gap_min: Duration,
    gap_max: Duration,
    hosts: Mutex<HashMap<String, HostState>>,
}

impl HostLimiter {
    pub fn new(min_interval: Duration, gap_min: Duration, gap_max: Duration) -> Self {
        Self {
            min_interval,
            gap_min,
            gap_max: gap_max.max(gap_min),
            hosts: Mutex::new(HashMap::new()),
        }
    }

    /// Reserve the next slot for `subject` on `host` and wait for it.
    pub async fn acquire(&self, host: &str, subject: &str) {
        let start = self.reserve(host, subject, Instant::now());
        tokio::time::sleep_until(start).await;
    }

    /// Push a host's next slot out (a `Retry-After` from the server).
    pub fn delay_host(&self, host: &str, until: Instant) {
        if let Ok(mut hosts) = self.hosts.lock()
            && let Some(state) = hosts.get_mut(host)
        {
            state.next_free = state.next_free.max(until);
        }
    }

    fn reserve(&self, host: &str, subject: &str, now: Instant) -> Instant {
        let Ok(mut hosts) = self.hosts.lock() else {
            return now;
        };
        let state = hosts.entry(host.to_owned()).or_insert_with(|| HostState {
            next_free: now,
            last_subject: None,
        });
        let mut start = state.next_free.max(now);
        if state
            .last_subject
            .as_deref()
            .is_some_and(|last| last != subject)
        {
            start += self.random_gap();
        }
        state.last_subject = Some(subject.to_owned());
        state.next_free = start + self.min_interval;
        start
    }

    fn random_gap(&self) -> Duration {
        let span = self.gap_max.saturating_sub(self.gap_min).as_millis() as u64;
        let extra = if span == 0 {
            0
        } else {
            rand::random::<u64>() % (span + 1)
        };
        self.gap_min + Duration::from_millis(extra)
    }
}
