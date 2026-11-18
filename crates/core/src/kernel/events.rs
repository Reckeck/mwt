//! Domain events and the bus that delivers them. A transaction collects the
//! events it emits; the commit publishes them, synchronously and in
//! subscription order, on the caller's thread. A subscriber gets the event
//! only, never the vault, so it cannot re-enter the transaction.

use std::sync::{Arc, Mutex, PoisonError, Weak};

/// Declares the payload structs and the `DomainEvent` enum that wraps them.
/// Every variant has the name of its payload struct, and `From<Payload>` for
/// `DomainEvent` is generated.
///
/// ```ignore
/// domain_events! {
///     Committed { revision: u64 },
///     KeysChanged { keys: Vec<String> },
/// }
///
/// let event: DomainEvent = Committed { revision: 3 }.into();
/// ```
macro_rules! domain_events {
    ($($name:ident { $($field:ident : $ty:ty),* $(,)? }),* $(,)?) => {
        $(
            #[derive(Debug, Clone, PartialEq)]
            pub struct $name {
                $(pub $field: $ty),*
            }

            impl From<$name> for DomainEvent {
                fn from(payload: $name) -> Self {
                    DomainEvent::$name(payload)
                }
            }
        )*

        /// Everything a transaction or a lock path can announce.
        #[derive(Debug, Clone, PartialEq)]
        pub enum DomainEvent {
            $($name($name)),*
        }
    };
}

/// Why a vault was closed. Every close path of the app names one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockReason {
    /// The user locked it.
    Manual,
    /// Idle, sleep or screen lock.
    Autolock,
    /// Leaving the sandbox.
    SandboxLeave,
    /// The same file is opened again while it is open.
    Reopen,
    /// Another vault is opened or created while one is open.
    Replace,
}

domain_events! {
    VaultLocked { reason: LockReason, closed: bool },
    Committed { revision: u64 },
    KeysChanged { keys: Vec<String> },
    AutomationDeleted { id: String },
    AutomationRetried { id: String, zone: String, at: i64 },
    GoalPaused { id: String, workflow_ids: Vec<String>, at: i64 },
    GoalResumed { id: String, workflow_ids: Vec<String>, at: i64 },
}

/// Told the keys of every local save, so a live session can send them on.
/// Every `SyncTap` is one (the blanket adapter lives next to that trait).
pub trait ChangeSink: Send + Sync {
    fn changed(&self, keys: &[String]);
}

type Handler = Arc<dyn Fn(&DomainEvent) + Send + Sync>;

#[derive(Default)]
struct Registry {
    next_id: u64,
    handlers: Vec<(u64, Handler)>,
}

/// The subscribers of one bus. Clones share the same subscribers.
#[derive(Clone, Default)]
pub struct EventBus {
    registry: Arc<Mutex<Registry>>,
}

/// Proof of one subscription; `unsubscribe` removes the handler. Dropping the
/// token keeps the handler.
pub struct Subscription {
    id: u64,
    registry: Weak<Mutex<Registry>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a handler after the existing ones.
    pub fn subscribe(
        &self,
        handler: impl Fn(&DomainEvent) + Send + Sync + 'static,
    ) -> Subscription {
        let mut registry = lock(&self.registry);
        let id = registry.next_id;
        registry.next_id += 1;
        registry.handlers.push((id, Arc::new(handler)));
        Subscription {
            id,
            registry: Arc::downgrade(&self.registry),
        }
    }

    /// Pass the keys of every `KeysChanged` event to `sink`.
    pub fn subscribe_changes<S: ChangeSink + ?Sized + 'static>(
        &self,
        sink: Arc<S>,
    ) -> Subscription {
        self.subscribe(move |event| {
            if let DomainEvent::KeysChanged(KeysChanged { keys }) = event {
                sink.changed(keys);
            }
        })
    }

    /// Call every subscriber with the event, in subscription order, here. The
    /// subscriber list is copied first, so a handler may unsubscribe.
    pub fn publish(&self, event: &DomainEvent) {
        let handlers: Vec<Handler> = lock(&self.registry)
            .handlers
            .iter()
            .map(|(_, handler)| handler.clone())
            .collect();
        for handler in handlers {
            handler(event);
        }
    }
}

impl Subscription {
    /// Remove the handler; later events do not reach it.
    pub fn unsubscribe(self) {
        if let Some(registry) = self.registry.upgrade() {
            lock(&registry).handlers.retain(|(id, _)| *id != self.id);
        }
    }
}

fn lock(registry: &Mutex<Registry>) -> std::sync::MutexGuard<'_, Registry> {
    registry.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recorder(bus: &EventBus, tag: &'static str, log: &Arc<Mutex<Vec<String>>>) -> Subscription {
        let log = log.clone();
        bus.subscribe(move |event| {
            log.lock().unwrap().push(format!("{tag}:{event:?}"));
        })
    }

    fn committed(revision: u64) -> DomainEvent {
        Committed { revision }.into()
    }

    #[test]
    fn subscribers_run_in_order() {
        let bus = EventBus::new();
        let log = Arc::new(Mutex::new(Vec::new()));
        recorder(&bus, "first", &log);
        recorder(&bus, "second", &log);
        bus.publish(&committed(7));
        let seen = log.lock().unwrap().clone();
        assert_eq!(seen.len(), 2);
        assert!(seen[0].starts_with("first:"));
        assert!(seen[1].starts_with("second:"));
    }

    #[test]
    fn an_unsubscribed_handler_gets_nothing() {
        let bus = EventBus::new();
        let log = Arc::new(Mutex::new(Vec::new()));
        let gone = recorder(&bus, "gone", &log);
        recorder(&bus, "kept", &log);
        gone.unsubscribe();
        bus.publish(&committed(1));
        let seen = log.lock().unwrap().clone();
        assert_eq!(seen.len(), 1);
        assert!(seen[0].starts_with("kept:"));
    }

    #[test]
    fn a_vault_locked_event_reaches_a_subscriber_with_its_reason() {
        let bus = EventBus::new();
        let log = Arc::new(Mutex::new(Vec::new()));
        recorder(&bus, "lock", &log);
        let locked = VaultLocked {
            reason: LockReason::Autolock,
            closed: false,
        };
        bus.publish(&locked.clone().into());
        let seen = log.lock().unwrap().clone();
        assert_eq!(
            seen,
            [format!("lock:{:?}", DomainEvent::VaultLocked(locked))]
        );
    }

    #[test]
    fn planning_events_convert_from_their_payloads() {
        let id = || "item-1".to_string();
        assert_eq!(
            DomainEvent::from(AutomationDeleted { id: id() }),
            DomainEvent::AutomationDeleted(AutomationDeleted { id: id() })
        );
        assert_eq!(
            DomainEvent::from(AutomationRetried {
                id: id(),
                zone: "UTC".into(),
                at: 7
            }),
            DomainEvent::AutomationRetried(AutomationRetried {
                id: id(),
                zone: "UTC".into(),
                at: 7
            })
        );
        assert_eq!(
            DomainEvent::from(GoalPaused {
                id: id(),
                workflow_ids: vec![],
                at: 7
            }),
            DomainEvent::GoalPaused(GoalPaused {
                id: id(),
                workflow_ids: vec![],
                at: 7
            })
        );
        assert_eq!(
            DomainEvent::from(GoalResumed {
                id: id(),
                workflow_ids: vec![],
                at: 7
            }),
            DomainEvent::GoalResumed(GoalResumed {
                id: id(),
                workflow_ids: vec![],
                at: 7
            })
        );
    }
}
