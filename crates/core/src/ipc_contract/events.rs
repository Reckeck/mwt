//! Payloads of the backend-to-frontend events that used to be inline
//! `serde_json::json!` literals. Each struct serializes to the JSON the
//! literal built, so the frontend sees no change.

use serde::Serialize;
use ts_rs::TS;

use crate::kernel::events::LockReason;

/// Payload of `bybit-import-pending`: how many records the poll recorded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct BybitImportPendingPayload {
    pub staged: u32,
    pub recorded: u32,
}

/// Payload of `crypto-updated`: the account whose records changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct CryptoUpdatedPayload {
    pub account_id: String,
}

/// Payload of `automation-created`: the transactions the run created.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct AutomationCreatedPayload {
    pub tx_ids: Vec<String>,
}

/// Payload of `automation-skipped`: how many runs were skipped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct AutomationSkippedPayload {
    pub count: usize,
}

/// Payload of `workflows-ran`: empty after a manual run, the counts after a
/// scheduled pass.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
pub struct WorkflowsRanPayload {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub runs: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub queued: Option<u32>,
}

/// Payload of `price-alert`: the alert that fired.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct PriceAlertPayload {
    pub alert_id: String,
}

/// Payload of `reminder-due` and `reminders-changed`: nothing but the signal.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
pub struct EmptyPayload {}

/// Payload of `enrichment-ready`: the transaction that was enriched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct EnrichmentReadyPayload {
    pub tx_id: String,
}

/// Payload of `vault-locked`: why the vault was locked and whether it
/// closed (`false` when the flush failed and the vault stayed open).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct VaultLockedPayload {
    pub reason: VaultLockedReason,
    pub closed: bool,
}

/// Why the vault was locked, as the `vault-locked` event names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum VaultLockedReason {
    Manual,
    Autolock,
    SandboxLeave,
    Reopen,
    Replace,
}

impl From<LockReason> for VaultLockedReason {
    fn from(reason: LockReason) -> Self {
        match reason {
            LockReason::Manual => Self::Manual,
            LockReason::Autolock => Self::Autolock,
            LockReason::SandboxLeave => Self::SandboxLeave,
            LockReason::Reopen => Self::Reopen,
            LockReason::Replace => Self::Replace,
        }
    }
}

/// Payload of `vault-save-failed`: the message, or the reason with
/// `exiting` when the app was closing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(untagged)]
pub enum VaultSaveFailedPayload {
    Message(String),
    Exiting { reason: String, exiting: bool },
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn as_json<P: Serialize>(payload: &P) -> serde_json::Value {
        serde_json::to_value(payload).unwrap()
    }

    #[test]
    fn each_lock_reason_keeps_its_wire_string() {
        let wire = [
            (LockReason::Manual, "manual"),
            (LockReason::Autolock, "autolock"),
            (LockReason::SandboxLeave, "sandbox-leave"),
            (LockReason::Reopen, "reopen"),
            (LockReason::Replace, "replace"),
        ];
        for (reason, name) in wire {
            assert_eq!(as_json(&VaultLockedReason::from(reason)), json!(name));
        }
    }

    #[test]
    fn the_payload_json_is_unchanged_bybit_import_pending() {
        let payload = BybitImportPendingPayload {
            staged: 0,
            recorded: 3,
        };
        assert_eq!(as_json(&payload), json!({ "staged": 0, "recorded": 3 }));
    }

    #[test]
    fn the_payload_json_is_unchanged_crypto_updated() {
        let payload = CryptoUpdatedPayload {
            account_id: "acc".into(),
        };
        assert_eq!(as_json(&payload), json!({ "account_id": "acc" }));
    }

    #[test]
    fn the_payload_json_is_unchanged_automation_created() {
        let ids = vec!["a".to_string(), "b".to_string()];
        let payload = AutomationCreatedPayload {
            tx_ids: ids.clone(),
        };
        assert_eq!(as_json(&payload), json!({ "tx_ids": ids }));
        let none = AutomationCreatedPayload { tx_ids: vec![] };
        assert_eq!(as_json(&none), json!({ "tx_ids": [] }));
    }

    #[test]
    fn the_payload_json_is_unchanged_automation_skipped() {
        let payload = AutomationSkippedPayload { count: 2 };
        assert_eq!(as_json(&payload), json!({ "count": 2 }));
    }

    #[test]
    fn the_payload_json_is_unchanged_workflows_ran() {
        let counts = WorkflowsRanPayload {
            runs: Some(2),
            queued: Some(1),
        };
        assert_eq!(as_json(&counts), json!({ "runs": 2, "queued": 1 }));
        assert_eq!(as_json(&WorkflowsRanPayload::default()), json!({}));
    }

    #[test]
    fn the_payload_json_is_unchanged_price_alert() {
        let payload = PriceAlertPayload {
            alert_id: "alert".into(),
        };
        assert_eq!(as_json(&payload), json!({ "alert_id": "alert" }));
    }

    #[test]
    fn the_payload_json_is_unchanged_reminder_due() {
        assert_eq!(as_json(&EmptyPayload {}), json!({}));
    }

    #[test]
    fn the_payload_json_is_unchanged_reminders_changed() {
        assert_eq!(as_json(&EmptyPayload::default()), json!({}));
    }

    #[test]
    fn the_payload_json_is_unchanged_enrichment_ready() {
        let payload = EnrichmentReadyPayload { tx_id: "tx".into() };
        assert_eq!(as_json(&payload), json!({ "tx_id": "tx" }));
    }

    #[test]
    fn the_payload_json_is_unchanged_vault_save_failed() {
        let message = VaultSaveFailedPayload::Message("disk full".into());
        assert_eq!(as_json(&message), json!("disk full"));
        let exiting = VaultSaveFailedPayload::Exiting {
            reason: "disk full".into(),
            exiting: true,
        };
        assert_eq!(
            as_json(&exiting),
            json!({ "reason": "disk full", "exiting": true })
        );
    }
}
