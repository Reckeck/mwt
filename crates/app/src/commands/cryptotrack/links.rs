//! Pointing a fiat record at a crypto operation, and back (spec E13).
//!
//! The pointer is a visible reference stored on the crypto side
//! (`CryptoOverride::fiat_tx_id`). It joins no chain, changes no cost basis
//! and no sum: the chain of money is a different thing, written by
//! `crypto_set_override` (spec E10) and by nothing in this file. The
//! application never links by itself, so a candidate is only ever suggested.

use tauri::State;

use trade_core::ipc_contract::cryptotrack::links::{
    CryptoLinkCandidatesInput, CryptoLinkInput, CryptoLinkOption, CryptoPair, CryptoPairsInput,
    CryptoUnlinkInput,
};

use crate::commands::AppState;
use crate::commands::{with_vault, with_vault_mut};
use crate::ipc::IpcError;
use crate::ipc::commands::ipc_commands;
use trade_core::usecases::cryptotrack::links;

/// The crypto operations that look like the same movement of money as a fiat
/// record: close in time, and matching in value at the rate of that day. Read
/// only.
#[tauri::command]
pub async fn crypto_link_candidates(
    input: CryptoLinkCandidatesInput,
    state: State<'_, AppState>,
) -> Result<Vec<CryptoLinkOption>, IpcError> {
    with_vault(&state, |vault| {
        Ok(links::crypto_link_candidates(&vault.books(), &input))
    })
}

/// Point a crypto operation at a fiat record, so each shows the other. Sets
/// `fiat_tx_id` and nothing else: it does not join a chain and changes no
/// amount. The record must be saved first.
#[tauri::command]
pub async fn crypto_link_operation(
    input: CryptoLinkInput,
    state: State<'_, AppState>,
) -> Result<(), IpcError> {
    with_vault_mut(&state, |vault| links::crypto_link_fiat(vault, &input))
}

/// Clear the pointer again (`fiat_tx_id`). Reversible: the notification
/// carries an undo that sets it back (rule 15). The chain is untouched.
#[tauri::command]
pub async fn crypto_unlink_operation(
    input: CryptoUnlinkInput,
    state: State<'_, AppState>,
) -> Result<(), IpcError> {
    with_vault_mut(&state, |vault| links::crypto_unlink_fiat(vault, &input))
}

/// The pointers of one operation or of one fiat record, both ends resolved.
/// An end that is gone comes back empty, not as an error. Read only.
#[tauri::command]
pub async fn crypto_link_pairs(
    input: CryptoPairsInput,
    state: State<'_, AppState>,
) -> Result<Vec<CryptoPair>, IpcError> {
    with_vault(&state, |vault| {
        Ok(links::crypto_pairs(&vault.books(), &input))
    })
}

ipc_commands!(cryptotrack_links_commands, CRYPTOTRACK_LINKS_NAMES, commands::cryptotrack::links: [crypto_link_candidates, crypto_link_operation, crypto_unlink_operation, crypto_link_pairs]);
