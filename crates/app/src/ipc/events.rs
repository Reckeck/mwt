//! Backend-to-frontend events: one declaration per event name, with its
//! payload type. [`app_events!`] generates a marker type per event (the name
//! constant and a typed `emit`, so a wrong payload does not compile) and the
//! [`EVENTS`] table the TypeScript generator reads. Names and payload JSON
//! are the ones the app already sends.

use serde::Serialize;
use tauri::{Emitter, Runtime};
use ts_rs::TS;

pub use trade_core::ipc_contract::events::VaultLockedPayload;

/// One event of the registry, as the generators see it.
pub struct EventMeta {
    pub name: &'static str,
    /// The TypeScript type of the payload.
    pub payload_ts: fn() -> String,
}

/// A backend-to-frontend event. Implemented by the types `app_events!` makes.
pub trait AppEvent {
    const NAME: &'static str;
    type Payload: Serialize + Clone + TS;

    /// Sends the event to every window.
    fn emit<R: Runtime>(app: &impl Emitter<R>, payload: &Self::Payload) -> tauri::Result<()> {
        app.emit(Self::NAME, payload)
    }
}

/// The TypeScript name of a payload type, for the [`EVENTS`] table.
pub fn payload_ts<P: TS>() -> String {
    P::name(&ts_rs::Config::new())
}

/// Declares the events: `"name" => Marker(PayloadType)`.
macro_rules! app_events {
    ( $( $name:literal => $marker:ident($payload:ty) ),* $(,)? ) => {
        $(
            #[doc = concat!("The `", $name, "` event.")]
            pub struct $marker;

            impl $crate::ipc::events::AppEvent for $marker {
                const NAME: &'static str = $name;
                type Payload = $payload;
            }
        )*

        /// Every event the backend sends, with its payload type.
        pub const EVENTS: &[$crate::ipc::events::EventMeta] = &[
            $( $crate::ipc::events::EventMeta {
                name: <$marker as $crate::ipc::events::AppEvent>::NAME,
                payload_ts: $crate::ipc::events::payload_ts::<$payload>,
            }, )*
        ];
    };
}

app_events! {
    "prices-updated" => PricesUpdated(Option<trade_core::ipc_contract::RefreshResult>),
    "rates-updated" => RatesUpdated(()),
    "sources-updated" => SourcesUpdated(()),
    "bybit-import-pending" => BybitImportPending(trade_core::ipc_contract::events::BybitImportPendingPayload),
    "crypto-updated" => CryptoUpdated(trade_core::ipc_contract::events::CryptoUpdatedPayload),
    "automation-created" => AutomationCreated(trade_core::ipc_contract::events::AutomationCreatedPayload),
    "automation-rejected" => AutomationRejected(trade_core::ipc_contract::AutomationRejectedEvent),
    "automation-skipped" => AutomationSkipped(trade_core::ipc_contract::events::AutomationSkippedPayload),
    "workflows-ran" => WorkflowsRan(trade_core::ipc_contract::events::WorkflowsRanPayload),
    "price-alert" => PriceAlert(trade_core::ipc_contract::events::PriceAlertPayload),
    "reminder-due" => ReminderDue(trade_core::ipc_contract::events::EmptyPayload),
    "reminders-changed" => RemindersChanged(trade_core::ipc_contract::events::EmptyPayload),
    "notification" => Notification(trade_core::model::notify::NoticeRecord),
    "enrichment-ready" => EnrichmentReady(trade_core::ipc_contract::events::EnrichmentReadyPayload),
    "quick-add" => QuickAdd(()),
    "refresh-prices" => RefreshPrices(()),
    "toggle-lock" => ToggleLock(()),
    "tray-navigate" => TrayNavigate(String),
    "vault-changed" => VaultChanged(trade_core::ipc_contract::VaultStatus),
    "vault-locked" => VaultLocked(VaultLockedPayload),
    // `SAVE_FAILED_EVENT`: a string, or `{ reason, exiting }` on exit.
    "vault-save-failed" => VaultSaveFailed(trade_core::ipc_contract::events::VaultSaveFailedPayload),
    // `SAVED_EVENT`
    "vault-saved" => VaultSaved(()),
    // `MIRROR_FAILED_EVENT`
    "vault-mirror-failed" => VaultMirrorFailed(String),
    // `STATE_EVENT`
    "update-state" => UpdateState(trade_core::ipc_contract::UpdateStatusDto),
    "security-raise-progress" => SecurityRaiseProgress(trade_core::ipc_contract::security::RaiseProgress),
    "sync-status" => SyncStatusChanged(trade_core::ipc_contract::SyncStatus),
    "sync-offer" => SyncOffer(trade_core::ipc_contract::SyncOfferEvent),
    "sync-pairing" => SyncPairing(trade_core::ipc_contract::SyncPairingEvent),
    "sync-plan-ready" => SyncPlanReady(trade_core::ipc_contract::SyncPlanEvent),
    "vault-synced" => VaultSynced(trade_core::ipc_contract::VaultSyncedEvent),
    "sync-clock" => SyncClock(trade_core::ipc_contract::SyncClockEvent),
    "sync-error" => SyncError(trade_core::ipc_contract::SyncErrorEvent),
    "transfer-receive" => TransferReceive(trade_core::ipc_contract::TransferReceiveView),
    "transfer-send" => TransferSend(trade_core::ipc_contract::TransferSendView),
    "transfer-nearby" => TransferNearby(Vec<trade_core::ipc_contract::TransferNearby>),
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    /// The names `docs/ipc.md` and the spec list, as the app sends them today.
    const OLD_NAMES: &[&str] = &[
        "prices-updated",
        "rates-updated",
        "sources-updated",
        "automation-created",
        "automation-rejected",
        "workflows-ran",
        "price-alert",
        "reminder-due",
        "reminders-changed",
        "notification",
        "enrichment-ready",
        "vault-changed",
        "vault-locked",
        "quick-add",
        "refresh-prices",
        "toggle-lock",
        "tray-navigate",
        "vault-saved",
        "vault-save-failed",
        "update-state",
        "vault-mirror-failed",
        "security-raise-progress",
        "bybit-import-pending",
        "crypto-updated",
        "automation-skipped",
        "sync-status",
        "sync-offer",
        "sync-pairing",
        "sync-plan-ready",
        "vault-synced",
        "sync-clock",
        "sync-error",
        "transfer-receive",
        "transfer-send",
        "transfer-nearby",
    ];

    #[test]
    fn an_event_name_is_unique() {
        let mut seen = HashSet::new();
        for event in EVENTS {
            assert!(seen.insert(event.name), "{} is declared twice", event.name);
        }
    }

    #[test]
    fn every_old_event_name_is_in_the_registry() {
        let registered: HashSet<_> = EVENTS.iter().map(|event| event.name).collect();
        for name in OLD_NAMES {
            assert!(registered.contains(name), "{name} is not in EVENTS");
        }
    }

    #[test]
    fn the_registry_names_match_the_constants() {
        assert_eq!(PricesUpdated::NAME, "prices-updated");
        assert_eq!(VaultSaved::NAME, "vault-saved");
        assert_eq!(VaultSaveFailed::NAME, "vault-save-failed");
        assert_eq!(UpdateState::NAME, "update-state");
        assert_eq!(VaultMirrorFailed::NAME, "vault-mirror-failed");
        assert_eq!(TransferReceive::NAME, "transfer-receive");
        assert_eq!(TransferSend::NAME, "transfer-send");
        assert_eq!(TransferNearby::NAME, "transfer-nearby");
    }

    #[test]
    fn vault_changed_still_fires() {
        assert_eq!(VaultChanged::NAME, "vault-changed");
        assert_eq!(VaultLocked::NAME, "vault-locked");
    }

    #[test]
    fn the_registry_pins_every_pub_event_constant() {
        use crate::sync::{
            CLOCK_EVENT, ERROR_EVENT, OFFER_EVENT, PAIRING_EVENT, PLAN_EVENT, STATUS_EVENT,
            SYNCED_EVENT,
        };

        let pairs = [
            (STATUS_EVENT, SyncStatusChanged::NAME),
            (OFFER_EVENT, SyncOffer::NAME),
            (PAIRING_EVENT, SyncPairing::NAME),
            (PLAN_EVENT, SyncPlanReady::NAME),
            (SYNCED_EVENT, VaultSynced::NAME),
            (ERROR_EVENT, SyncError::NAME),
            (CLOCK_EVENT, SyncClock::NAME),
        ];
        for (constant, registered) in pairs {
            assert_eq!(constant, registered);
        }
    }

    /// Never called: it compiles only if typed `emit` accepts a real handle.
    fn emits_typed<R: Runtime>(app: &tauri::AppHandle<R>) {
        let _ = PricesUpdated::emit(app, &None);
    }

    #[test]
    fn typed_emit_compiles_with_a_real_app_handle() {
        let _ = emits_typed::<tauri::Wry>;
    }

    #[test]
    fn the_table_carries_the_payload_type() {
        let find = |name: &str| EVENTS.iter().find(|event| event.name == name).unwrap();
        assert_eq!(
            (find("prices-updated").payload_ts)(),
            "RefreshResult | null"
        );
        assert_eq!((find("rates-updated").payload_ts)(), "null");
        assert_eq!((find("price-alert").payload_ts)(), "PriceAlertPayload");
    }
}
