use tauri::{AppHandle, Emitter, State};

use trade_core::ipc_contract::{
    AssetPriceInput, CachedPrice, CandlePoint, GetCandlesInput, GetCandlesOutput, RefreshResult,
};
use trade_core::model::store::AssetRef;
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

/// Last cached USD quote for an asset. Cache-only: no network call, so it is
/// safe to hit on every keystroke of a price field. Used to prefill the
/// transaction form and the wallet calculator.
#[tauri::command]
pub async fn get_cached_price(
    input: AssetPriceInput,
    state: State<'_, AppState>,
) -> Result<CachedPrice, String> {
    let guard = require_vault(&state).map_err(err_to_string)?;
    let vault = guard.as_ref().unwrap();
    let symbol = input.symbol.trim().to_uppercase();
    if symbol.is_empty() {
        return Ok(CachedPrice::default());
    }
    let asset = AssetRef {
        quote: symbol.clone(),
        name: symbol.clone(),
        symbol,
        coingecko_id: input.coingecko_id,
        chain: input.chain,
        contract: input.contract,
    };
    let key = asset.key();
    let hit = vault
        .doc
        .price_cache
        .iter()
        .filter(|point| point.asset_key == key)
        .max_by(|left, right| left.ts.cmp(&right.ts));
    Ok(match hit {
        Some(point) => CachedPrice {
            price_usd: point.value_usd,
            ts: point.ts.clone(),
        },
        None => CachedPrice::default(),
    })
}

/// Refresh crypto prices + FX for everything currently held, emit event.
#[tauri::command]
pub async fn refresh_all(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<RefreshResult, String> {
    let (assets, base) = {
        let mut guard = require_vault(&state).map_err(err_to_string)?;
        let vault = guard.as_mut().unwrap();

        let assets: Vec<trade_core::model::store::AssetRef> = {
            let mut seen = std::collections::HashSet::new();
            vault
                .doc
                .holdings
                .iter()
                .filter(|hit| hit.asset.is_crypto())
                .map(|hit| hit.asset.clone())
                .filter(|asset| seen.insert(asset.key()))
                .collect()
        };
        let base = vault.base_currency().to_string();
        (assets, base)
    };

    let prices = state
        .pricing
        .fetch_prices(&assets)
        .await
        .map_err(crate::commands::err_to_string)?;
    let fx = state
        .pricing
        .fetch_fx(&base)
        .await
        .map_err(crate::commands::err_to_string)?;

    let mut guard = require_vault(&state).map_err(err_to_string)?;
    let vault = guard.as_mut().unwrap();
    trade_core::pricing::PricingService::apply_prices(&mut vault.doc, &prices);
    trade_core::pricing::PricingService::apply_fx(&mut vault.doc, &fx);
    let updated_at = trade_core::model::vault::now_iso();
    let _ = vault.persist();

    let result = RefreshResult {
        prices: prices.len(),
        fx: fx.len(),
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
