//! Entering the module, and the archive of what was kept by hand.
//!
//! Owner: spec E11, with `service/cryptotrack/transition.rs`. The step is
//! idempotent: a failure between the re-encryption and the archiving is
//! repaired at the next open, and no record is ever deleted.

use tauri::State;

use trade_core::ipc_contract::cryptotrack::transition::{
    CryptoArchiveReport, CryptoTransitionPlan,
};

use crate::commands::AppState;
use crate::commands::planning::{with_vault, with_vault_mut};

/// What entering the module would change: the hand-kept crypto wallets the
/// move archives and how much capital leaves the totals with them. This is
/// the number the wizard warns with, counted on demand and stored nowhere, so
/// the warning can be shown at any level.
#[tauri::command]
pub async fn crypto_transition_plan(
    state: State<'_, AppState>,
) -> Result<CryptoTransitionPlan, String> {
    with_vault(&state, |vault| Ok(vault.crypto_transition_plan()))
}

/// Archive the crypto wallets kept by hand, once the vault is at level 3. A
/// wallet the module reads stays — a followed address, and an exchange of a
/// provider the module talks to, whose records spec E7 converts rather than
/// retires — and so does an empty wallet, which has no record to retire.
/// Nothing is deleted, and a second call reports `changed: false`.
#[tauri::command]
pub async fn crypto_archive_manual_wallets(
    state: State<'_, AppState>,
) -> Result<CryptoArchiveReport, String> {
    with_vault_mut(&state, |vault| vault.crypto_archive_manual_wallets())
}
