//! The tokens the module has met, their quarantine and the user's verdict.
//!
//! Owner: spec E2 (its browser mock comes with E9), with
//! `service/cryptotrack/tokens.rs` and `cryptotrack/tokens/`.

use tauri::State;

use trade_core::cryptotrack::tokens::TokenStatus;
use trade_core::ipc_contract::cryptotrack::tokens::{SetTokenBasisInput, TokenRefInput, TokenView};
use trade_core::model::cryptotrack::TokenId;

use crate::commands::AppState;
use crate::commands::planning::{with_vault, with_vault_mut};
use crate::ipc::IpcError;
use crate::ipc::commands::ipc_commands;
use trade_core::usecases::cryptotrack::tokens;

/// The known and the quarantined tokens of the followed accounts.
#[tauri::command]
pub async fn crypto_list_tokens(state: State<'_, AppState>) -> Result<Vec<TokenView>, IpcError> {
    with_vault(&state, |vault| Ok(tokens::crypto_tokens(&vault.doc)))
}

/// Take a quarantined token into the totals. The verdict is a correction, so
/// it survives a refetch of the events.
#[tauri::command]
pub async fn crypto_confirm_token(
    input: TokenRefInput,
    state: State<'_, AppState>,
) -> Result<TokenStatus, IpcError> {
    let token = TokenId::new(input.chain, &input.contract);
    with_vault_mut(&state, |vault| tokens::crypto_confirm_token(vault, &token))
}

/// Hide a token: a scam airdrop is shown nowhere and priced nowhere.
#[tauri::command]
pub async fn crypto_reject_token(
    input: TokenRefInput,
    state: State<'_, AppState>,
) -> Result<TokenStatus, IpcError> {
    let token = TokenId::new(input.chain, &input.contract);
    with_vault_mut(&state, |vault| tokens::crypto_reject_token(vault, &token))
}

/// Take back a recognition or a hiding: the token is judged by the built-in
/// list and the quarantine again, so it returns to the quarantine list and
/// stops counting towards the capital.
#[tauri::command]
pub async fn crypto_clear_token_verdict(
    input: TokenRefInput,
    state: State<'_, AppState>,
) -> Result<TokenStatus, IpcError> {
    let token = TokenId::new(input.chain, &input.contract);
    with_vault_mut(&state, |vault| {
        tokens::crypto_clear_token_verdict(vault, &token)
    })
}

/// The cost basis of what was bought before the module started following the
/// address, as an attribute of the token.
#[tauri::command]
pub async fn crypto_set_token_basis(
    input: SetTokenBasisInput,
    state: State<'_, AppState>,
) -> Result<(), IpcError> {
    let token = TokenId::new(input.chain, &input.contract);
    with_vault_mut(&state, |vault| {
        tokens::crypto_set_token_basis(vault, &token, input.basis_usd)
    })
}

ipc_commands!(cryptotrack_tokens_commands, CRYPTOTRACK_TOKENS_NAMES, commands::cryptotrack::tokens: [crypto_list_tokens, crypto_confirm_token, crypto_reject_token, crypto_clear_token_verdict, crypto_set_token_basis]);
