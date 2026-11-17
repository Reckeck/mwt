//! Process-wide state that is not the vault: the bus of lock events (spec D5).

use tauri::Manager;
use trade_core::kernel::events::{DomainEvent, EventBus, LockReason, VaultLocked};

use crate::ipc::events::{AppEvent, VaultLockedPayload};

/// The bus the app's one close path publishes `VaultLocked` on. Managed by
/// Tauri; what must follow a lock subscribes once at startup.
#[derive(Default)]
pub struct LockBus(pub EventBus);

/// Call `action` on every `VaultLocked`, whatever the reason and whether or
/// not the vault closed.
pub fn on_vault_locked(bus: &EventBus, action: impl Fn(&VaultLocked) + Send + Sync + 'static) {
    bus.subscribe(move |event| {
        if let DomainEvent::VaultLocked(locked) = event {
            action(locked);
        }
    });
}

/// The name of a lock reason on the `vault-locked` event.
fn reason_name(reason: LockReason) -> &'static str {
    match reason {
        LockReason::Manual => "manual",
        LockReason::Autolock => "autolock",
        LockReason::SandboxLeave => "sandbox-leave",
        LockReason::Reopen => "reopen",
        LockReason::Replace => "replace",
    }
}

/// Hand `send` the `vault-locked` payload of every `VaultLocked`, also when
/// the flush failed and the vault stayed open (`closed = false`).
pub(crate) fn announce_lock(
    bus: &EventBus,
    send: impl Fn(VaultLockedPayload) + Send + Sync + 'static,
) {
    on_vault_locked(bus, move |locked| {
        send(VaultLockedPayload {
            reason: reason_name(locked.reason).to_owned(),
            closed: locked.closed,
        });
    });
}

/// The one registration of the lock subscribers, at startup.
pub fn register_lock_subscribers(app: &tauri::AppHandle) {
    let bus = &app.state::<LockBus>().0;
    crate::commands::attachments::wipe_on_lock(bus, app.clone());
    crate::cryptotrack::jobs::forget_positions_on_lock(bus);
    let handle = app.clone();
    announce_lock(bus, move |payload| {
        if let Err(error) = crate::ipc::events::VaultLocked::emit(&handle, &payload) {
            eprintln!("vault-locked emit failed: {error}");
        }
    });
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    fn recorded(bus: &EventBus) -> Arc<Mutex<Vec<VaultLockedPayload>>> {
        let seen = Arc::new(Mutex::new(vec![]));
        let sink = Arc::clone(&seen);
        announce_lock(bus, move |payload| sink.lock().unwrap().push(payload));
        seen
    }

    #[test]
    fn a_lock_emits_vault_locked_with_reason_and_closed() {
        let bus = EventBus::new();
        let seen = recorded(&bus);
        bus.publish(
            &VaultLocked {
                reason: LockReason::Autolock,
                closed: true,
            }
            .into(),
        );
        let expected = VaultLockedPayload {
            reason: "autolock".into(),
            closed: true,
        };
        assert_eq!(*seen.lock().unwrap(), vec![expected]);
    }

    #[test]
    fn a_failed_flush_emits_closed_false() {
        let bus = EventBus::new();
        let seen = recorded(&bus);
        bus.publish(
            &VaultLocked {
                reason: LockReason::Manual,
                closed: false,
            }
            .into(),
        );
        assert!(!seen.lock().unwrap()[0].closed);
    }
}
