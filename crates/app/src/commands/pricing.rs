use tauri::{AppHandle, Emitter, State};

use trade_core::ipc_contract::{
    AssetPriceInput, CachedPrice, CandlePoint, ConvertAmountInput, ConvertedAmount,
    GetCandlesInput, GetCandlesOutput, RefreshResult,
};
use trade_core::pricing::AssetSearchHit;

use crate::commands::{AppState, err_to_string, require_vault};

/// Search crypto assets for the UI comboboxes.
#[tauri::command]
pub async fn search_assets(
    query: String,
    state: State<'_, AppState>,
) -> Result<Vec<AssetSearchHit>, String> {
    state
        .pricing
        .search_assets(&query, 25)
        .await
        .map_err(crate::commands::err_to_string)
}

/// Cached USD quote of an asset nearest a moment (now when `at` is empty).
/// Cache-only: no network call, so it is safe to hit on every keystroke of a
/// price field. Used to prefill the transaction form and the wallet
/// calculator, and to show which day's rate a new operation will record.
#[tauri::command]
pub async fn get_cached_price(
    input: AssetPriceInput,
    state: State<'_, AppState>,
) -> Result<CachedPrice, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    let vault = guard.as_ref().unwrap();
    if input.symbol.trim().is_empty() {
        return Ok(CachedPrice::default());
    }
    Ok(vault.cached_quote(&input.to_asset(), input.at.as_deref().unwrap_or_default()))
}

/// Convert an amount between two currencies at the rate of a moment.
#[tauri::command]
pub async fn convert_amount(
    input: ConvertAmountInput,
    state: State<'_, AppState>,
) -> Result<ConvertedAmount, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    let vault = guard.as_ref().unwrap();
    Ok(ConvertedAmount {
        amount: vault.convert_at(
            input.amount,
            &input.from,
            &input.to,
            input.at.as_deref().unwrap_or_default(),
        ),
    })
}

/// Refresh crypto prices and the whole FX table, then price what was saved
/// without a price (plan §6.1–§6.2), and emit `prices-updated`.
#[tauri::command]
pub async fn refresh_all(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<RefreshResult, String> {
    let assets = {
        let guard = require_vault(&state).map_err(err_to_string)?;
        let vault = guard.as_ref().unwrap();
        let mut seen = std::collections::HashSet::new();
        vault
            .doc
            .holdings
            .iter()
            .filter(|hit| hit.asset.is_crypto() && !hit.asset.is_derivative())
            .map(|hit| hit.asset.clone())
            .filter(|asset| seen.insert(asset.key()))
            .collect::<Vec<_>>()
    };

    let prices = state
        .pricing
        .fetch_prices(&assets)
        .await
        .map_err(crate::commands::err_to_string)?;
    let fx = state
        .pricing
        .fetch_fx()
        .await
        .map_err(crate::commands::err_to_string)?;

    let mut guard = require_vault(&state).map_err(err_to_string)?;
    let vault = guard.as_mut().unwrap();
    vault.apply_prices(&prices);
    vault.apply_fx(&fx);
    let restamped = vault.restamp_missing();
    let updated_at = trade_core::model::vault::now_iso();
    let _ = vault.persist();

    let result = RefreshResult {
        prices: prices.len(),
        fx: fx.len(),
        restamped,
        updated_at,
    };
    let _ = app.emit("prices-updated", &result);
    Ok(result)
}

/// Open a URL (typically an explorer tx link) in the default browser.
#[tauri::command]
pub async fn open_explorer(_app: AppHandle, url: String) -> Result<(), String> {
    tauri_plugin_opener::open_url(&url, None::<&str>)
        .map_err(|_| format!("failed to open {url}"))?;
    Ok(())
}

/// Fetch OHLCV candles from Binance public API for charting.
/// No vault access required — pure public data.
#[tauri::command]
pub async fn get_candles(input: GetCandlesInput) -> Result<GetCandlesOutput, String> {
    use reqwest::Client;
    use trade_core::market::fetch_candles;

    let symbol = input.symbol.trim().to_uppercase();
    if symbol.is_empty() {
        return Ok(GetCandlesOutput { candles: vec![] });
    }
    let limit = input.limit.clamp(1, 1000);
    let interval_min = input.interval_min;

    let client = Client::new();
    let candles = fetch_candles(&symbol, interval_min, limit, &client)
        .await
        .map_err(crate::commands::err_to_string)?;

    let candles = candles
        .into_iter()
        .map(|candle| CandlePoint {
            ts: candle.ts,
            open: candle.open.to_string(),
            high: candle.high.to_string(),
            low: candle.low.to_string(),
            close: candle.close.to_string(),
            volume: candle.volume.to_string(),
        })
        .collect();

    Ok(GetCandlesOutput { candles })
}
