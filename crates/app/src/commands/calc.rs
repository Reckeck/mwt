use tauri::State;

use trade_core::calc::{self, CalcInput, CalcResult};

use crate::commands::AppState;

/// Pure calculator preview (converter / position). Needs no vault state —
/// the whole computation happens in `trade_core::calc`.
#[tauri::command]
pub async fn calc_preview(
    input: CalcInput,
    _state: State<'_, AppState>,
) -> Result<CalcResult, String> {
    Ok(calc::preview(&input))
}
