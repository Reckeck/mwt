//! The endpoints of each network: RPC, Blockscout, the explorer.
//!
//! Owner: spec E17, with `service/cryptotrack/network.rs`; the screen is a
//! subsection of Settings (decision P5 of 2026-11-11). Requests go out of
//! Rust, never out of the webview, so the CSP is not widened for them.
//!
//! The settings, and the optional Blockscout key (decision R8), live in the
//! device secrets file; the key is never returned, logged or put in an error.

use tauri::{AppHandle, State};

use trade_core::error::Error;
use trade_core::ipc_contract::cryptotrack::network::{
    NetworkSettings, NetworkTestResult, SetNetworkSettingsInput, TestNetworkInput,
};
use trade_core::service::{NetworkProbe, test_endpoint};

use crate::commands::app_settings::current_settings;
use crate::commands::{AppState, sandbox};
use crate::commands::{with_vault, with_vault_mut};
use crate::ipc::IpcError;
use crate::ipc::commands::ipc_commands;
use trade_core::usecases::cryptotrack::network;

/// The endpoints of every network, with the presets to fall back to. Below
/// protection level 3 they are the presets and `editable` is false.
#[tauri::command]
pub async fn crypto_network_settings(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<NetworkSettings, IpcError> {
    let mut settings = with_vault::<_, IpcError>(&state, |vault| {
        Ok(network::crypto_network_settings(&vault.books()))
    })?;
    // The consent is this device's own answer and lives in the app settings.
    let app_settings = current_settings(&app);
    if app_settings.crypto_tracking_allowed() {
        settings.consent_at = app_settings.crypto_tracking_consent_at.clone();
        settings.consent_device = app_settings.device_name.clone();
    }
    Ok(settings)
}

/// Save the endpoints of one network. Only `https` is accepted, apart from
/// `localhost` for the user's own node; an empty address is the preset.
#[tauri::command]
pub async fn crypto_set_network_settings(
    input: SetNetworkSettingsInput,
    state: State<'_, AppState>,
) -> Result<(), IpcError> {
    with_vault_mut(&state, |vault| {
        network::crypto_set_network_settings(vault, &input)
    })
}

/// Ask one endpoint whether it answers, on the user's button. A network call,
/// so it needs the same consent as the tracking and is refused without it
/// (and in the sandbox). The vault lock is let go before the request.
#[tauri::command]
pub async fn crypto_test_network(
    app: AppHandle,
    input: TestNetworkInput,
    state: State<'_, AppState>,
) -> Result<NetworkTestResult, IpcError> {
    let consent = current_settings(&app).crypto_tracking_allowed();
    if sandbox::is_active(&state) {
        return Err(IpcError::from(Error::Sandbox(
            "the sandbox talks to no network".into(),
        )));
    }
    let (url, key) = with_vault::<_, IpcError>(&state, |vault| {
        network::crypto_test_target(&vault.books(), &input)
    })?;
    let probe = NetworkProbe::new()?;
    test_endpoint(&probe, consent, input.chain, input.kind, &url, key.as_ref())
        .await
        .map_err(IpcError::from)
}

ipc_commands!(cryptotrack_network_commands, CRYPTOTRACK_NETWORK_NAMES, commands::cryptotrack::network: [crypto_network_settings, crypto_set_network_settings, crypto_test_network]);
