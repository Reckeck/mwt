//! The addresses and exchanges the module follows.
//!
//! Owner: spec E9, with `service/cryptotrack/accounts.rs` and
//! `ipc_contract/cryptotrack/accounts.rs`. An account is shown by an ordinary
//! wallet whose `Store::crypto_account_id` points at it (decision P5), so
//! these commands feed the wallet page and not a section of their own.
//!
//! There is no command that takes a seed phrase or a private key, and a
//! string that looks like one is refused in the service before it is stored.

use tauri::State;

use trade_core::ipc_contract::cryptotrack::accounts::{
    ConnectAddressInput, ConnectAddressResult, CryptoAccountRefInput, CryptoAccountView,
    UpdateAccountInput,
};

use crate::commands::AppState;
use crate::commands::planning::{with_vault, with_vault_mut};
use crate::ipc::commands::ipc_commands;

/// The accounts the module follows, with the wallet that shows each and the
/// tokens of it that wait in quarantine.
#[tauri::command]
pub async fn crypto_list_accounts(
    state: State<'_, AppState>,
) -> Result<Vec<CryptoAccountView>, String> {
    with_vault(&state, |vault| Ok(vault.crypto_list_accounts()))
}

/// Follow a public address: the last step of the wallet wizard for the
/// `Crypto` kind. A string that looks like a seed phrase or a private key is
/// refused and never stored.
#[tauri::command]
pub async fn crypto_connect_address(
    input: ConnectAddressInput,
    state: State<'_, AppState>,
) -> Result<ConnectAddressResult, String> {
    with_vault_mut(&state, |vault| vault.crypto_connect_address(&input))
}

/// Rename an account, change its note or stop counting it.
#[tauri::command]
pub async fn crypto_update_account(
    input: UpdateAccountInput,
    state: State<'_, AppState>,
) -> Result<(), String> {
    with_vault_mut(&state, |vault| vault.crypto_update_account(&input))
}

/// Stop following an address. Its events stay until the module is wiped
/// (spec E12).
#[tauri::command]
pub async fn crypto_forget_account(
    input: CryptoAccountRefInput,
    state: State<'_, AppState>,
) -> Result<(), String> {
    with_vault_mut(&state, |vault| {
        vault.crypto_forget_account(&input.account_id)
    })
}

ipc_commands!(cryptotrack_accounts_commands, CRYPTOTRACK_ACCOUNTS_NAMES, commands::cryptotrack::accounts: [crypto_list_accounts, crypto_connect_address, crypto_update_account, crypto_forget_account]);
