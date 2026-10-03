use tauri::{AppHandle, Emitter, State};

use trade_core::ipc_contract::{AssetPriceInput, CachedPrice, RefreshResult, SetTxPricesInput};
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
    let g = require_vault(&state).map_err(err_to_string)?;
    let v = g.as_ref().unwrap();
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
    let hit = v
        .doc
        .price_cache
        .iter()
        .filter(|p| p.asset_key == key)
        .max_by(|a, b| a.ts.cmp(&b.ts));
    Ok(match hit {
        Some(p) => CachedPrice {
            price_usd: p.value_usd,
            ts: p.ts.clone(),
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
        let mut g = require_vault(&state).map_err(err_to_string)?;
        let v = g.as_mut().unwrap();

        let assets: Vec<trade_core::model::store::AssetRef> = {
            let mut seen = std::collections::HashSet::new();
            v.doc
                .holdings
                .iter()
                .filter(|h| h.asset.is_crypto())
                .map(|h| h.asset.clone())
                .filter(|a| seen.insert(a.key()))
                .collect()
        };
        let base = v.doc.settings.base_currency.clone();
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

    let mut g = require_vault(&state).map_err(err_to_string)?;
    let v = g.as_mut().unwrap();
    trade_core::pricing::PricingService::apply_prices(&mut v.doc, &prices);
    trade_core::pricing::PricingService::apply_fx(&mut v.doc, &fx);
    let updated_at = trade_core::model::vault::now_iso();
    let _ = v.persist();

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

/// Stamp price/fx overrides on an historical transaction.
#[tauri::command]
pub async fn set_tx_prices(
    input: SetTxPricesInput,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut g = require_vault(&state).map_err(err_to_string)?;
    let v = g.as_mut().unwrap();
    v.set_prices(&input.transaction_id, input.price_usd, input.fx_override)
        .map_err(crate::commands::err_to_string)
}
