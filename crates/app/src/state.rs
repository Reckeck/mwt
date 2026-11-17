//! Process-wide state that is not the vault: the bus of lock events (spec D5).

use tauri::Manager;
use trade_core::kernel::events::{DomainEvent, EventBus, VaultLocked};

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

/// The one registration of the lock subscribers, at startup.
pub fn register_lock_subscribers(app: &tauri::AppHandle) {
    let bus = &app.state::<LockBus>().0;
    crate::commands::attachments::wipe_on_lock(bus, app.clone());
    crate::cryptotrack::jobs::forget_positions_on_lock(bus);
}
