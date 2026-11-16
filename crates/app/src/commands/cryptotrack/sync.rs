//! The background tracking of the followed accounts.
//!
//! Owner: spec E6, with the tick in `crates/app/src/cryptotrack/jobs.rs` and
//! `service/cryptotrack/apply.rs`.

use tauri::AppHandle;
use trade_core::ipc_contract::cryptotrack::sync::CryptoSyncStatus;
use trade_core::model::vault::now_iso;

use crate::commands::app_settings::{config_dir, current_settings, save};
use crate::commands::err_to_string;
use crate::cryptotrack::jobs;

/// What each account's last read did, and what the module is reading now.
#[tauri::command]
pub async fn crypto_sync_status(app: AppHandle) -> Result<CryptoSyncStatus, String> {
    jobs::status(&app).map_err(err_to_string)
}

/// Read the followed accounts now, without waiting for the tick. Refused
/// without the user's consent, and on a device that does not run the jobs.
#[tauri::command]
pub async fn crypto_sync_now(app: AppHandle) -> Result<(), String> {
    jobs::read_now(&app).map_err(err_to_string)
}

/// The one-time consent to talk to the providers, after the pattern of
/// `enrichment_opt_in`: without it the module makes no network call at all,
/// and withdrawing it stops them again (a running pass ends before its next
/// request). A yes starts the catch-up at once.
#[tauri::command]
pub async fn crypto_set_tracking_consent(app: AppHandle, consent: bool) -> Result<(), String> {
    let dir = config_dir(&app);
    let mut settings = current_settings(&app);
    settings.crypto_tracking_consent = Some(consent);
    // A yes keeps its date (and a repeated yes keeps the first one); a no
    // clears it, so the next yes records its own.
    settings.crypto_tracking_consent_at = if consent {
        settings
            .crypto_tracking_consent_at
            .or_else(|| Some(now_iso()))
    } else {
        None
    };
    save(&dir, &settings).map_err(err_to_string)?;
    if consent {
        jobs::consent_changed(&app);
    }
    Ok(())
}
