//! The DeFi positions an address holds inside a protocol.
//!
//! Owner: spec D0 (stage 3), with `service/cryptotrack/positions.rs`. The
//! stored shape and its sync rule exist from E0, so DeFi needs no further
//! change of the vault schema.
//!
//! The command puts three things together: the stored positions and their
//! operations (the vault), what this device's last pass read
//! (`cryptotrack::jobs::live_positions`) and the clock. Everything money is
//! folded here and now — nothing derived is stored.

use tauri::State;

use trade_core::ipc_contract::cryptotrack::positions::PositionView;
use trade_core::model::vault::now_iso;

use crate::commands::AppState;
use crate::commands::planning::with_vault;
use crate::cryptotrack::jobs;

/// The positions of the followed accounts, open ones first. Read only.
///
/// A figure the stored document cannot answer for — a pool's uncollected
/// fees, the adapter's `approximate` — is absent on a device that has only
/// synced, never a zero.
#[tauri::command]
pub async fn crypto_list_positions(
    state: State<'_, AppState>,
) -> Result<Vec<PositionView>, String> {
    let live = jobs::live_positions();
    let at = now_iso();
    with_vault(&state, |vault| Ok(vault.crypto_list_positions(&live, &at)))
}
