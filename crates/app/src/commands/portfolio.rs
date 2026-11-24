use tauri::{AppHandle, State};

use mwt_base::Amount;
use mwt_base::calendar::now_secs;
use mwt_money::asset_holdings::AssetHoldings;
use mwt_money::investments::Investments;
use mwt_money::portfolio::{HistoryPoint, PortfolioSummary, RealizedPnl};
use mwt_money::store_months::StoreMonthRow;
use mwt_money::valuation::{PropertyValue, ValuationInput};
use mwt_money::worth::NetWorth;
use mwt_money::{asset_holdings, investments, portfolio, valuation as valuation_reads};
use mwt_planning::allocation as allocation_reads;
use mwt_planning::allocation::{AssetClassTarget, AssetClassesView, TopupAdvice};
use mwt_usecases::ipc_contract::SetManualPriceInput;
use mwt_usecases::model::ids::StoreId;
use mwt_usecases::model::planning::AssetClass;
use mwt_usecases::model::store::AssetRef;
use mwt_usecases::model::vault::ManualPrice;
use mwt_usecases::model::vault::Valuation;
use mwt_usecases::usecases::household::store_months;
use mwt_usecases::usecases::money::{capital, valuation};
use mwt_usecases::usecases::planning::allocation;

use crate::commands::AppState;
use crate::commands::app_settings::current_zone;
use crate::commands::{with_vault, with_vault_mut};
use crate::ipc::IpcError;
use crate::ipc::commands::ipc_commands;

#[tauri::command]
pub async fn get_summary(state: State<'_, AppState>) -> Result<PortfolioSummary, IpcError> {
    with_vault(&state, |vault| {
        Ok(capital::portfolio_summary(&vault.books()))
    })
}

#[tauri::command]
pub async fn get_history(
    base_currency: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<HistoryPoint>, IpcError> {
    with_vault(&state, |vault| {
        let base = base_currency.unwrap_or_else(|| vault.base_currency().to_string());
        Ok(portfolio::history_series(&vault.books(), &base))
    })
}

/// The same asset on every wallet (the «elsewhere» block of the asset sheet).
#[tauri::command]
pub async fn get_asset_holdings(
    asset: AssetRef,
    state: State<'_, AppState>,
) -> Result<AssetHoldings, IpcError> {
    with_vault(&state, |vault| {
        Ok(asset_holdings::asset_holdings(&vault.books(), &asset))
    })
}

/// "Investments": coins and futures margin, at cost and today (Money 2 · 13).
#[tauri::command]
pub async fn get_investments(
    store_ids: Option<Vec<String>>,
    state: State<'_, AppState>,
) -> Result<Investments, IpcError> {
    with_vault(&state, |vault| {
        Ok(investments::investments(
            &vault.books(),
            &store_ids.unwrap_or_default(),
        ))
    })
}

/// Money in and out of a wallet per local month, the last `months` (default 6).
#[tauri::command]
pub async fn get_store_months(
    app: AppHandle,
    store_id: String,
    include_children: Option<bool>,
    months: Option<u32>,
    state: State<'_, AppState>,
) -> Result<Vec<StoreMonthRow>, IpcError> {
    let zone = current_zone(&app);
    with_vault(&state, |vault| {
        Ok(store_months::store_months(
            vault,
            &StoreId::from(store_id),
            include_children.unwrap_or(true),
            months.unwrap_or(6).clamp(1, 36),
            now_secs(),
            &zone,
        ))
    })
}

#[tauri::command]
pub async fn get_realized_pnl(state: State<'_, AppState>) -> Result<RealizedPnl, IpcError> {
    with_vault(&state, |vault| Ok(portfolio::realized_pnl(&vault.books())))
}

/// Value of a single store over time (wallet page chart); with its
/// sub-accounts unless `include_children` is `false`.
#[tauri::command]
pub async fn get_store_history(
    store_id: String,
    include_children: Option<bool>,
    base_currency: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<HistoryPoint>, IpcError> {
    with_vault(&state, |vault| {
        let base = base_currency.unwrap_or_else(|| vault.base_currency().to_string());
        Ok(portfolio::store_history(
            &vault.books(),
            &store_id,
            include_children.unwrap_or(true),
            &base,
        ))
    })
}

/// Realized PnL booked on a single store (and, by default, its sub-accounts).
#[tauri::command]
pub async fn get_store_pnl(
    store_id: String,
    include_children: Option<bool>,
    state: State<'_, AppState>,
) -> Result<RealizedPnl, IpcError> {
    with_vault(&state, |vault| {
        Ok(portfolio::store_pnl(
            &vault.books(),
            &store_id,
            include_children.unwrap_or(true),
        ))
    })
}

/// Assets, debts and what people owe, in the base currency.
#[tauri::command]
pub async fn get_net_worth(state: State<'_, AppState>) -> Result<NetWorth, IpcError> {
    with_vault(&state, |vault| Ok(capital::net_worth(&vault.books())))
}

/// Hand-set value of one unit of a property asset (a flat, a car).
#[tauri::command]
pub async fn set_manual_price(
    input: SetManualPriceInput,
    state: State<'_, AppState>,
) -> Result<ManualPrice, IpcError> {
    with_vault_mut(&state, |vault| {
        capital::set_manual_price(vault, &input.asset_key, input.unit_price, &input.currency)
    })
}

#[tauri::command]
pub async fn clear_manual_price(
    asset_key: String,
    state: State<'_, AppState>,
) -> Result<(), IpcError> {
    with_vault_mut(&state, |vault| {
        capital::clear_manual_price(vault, &asset_key)
    })
}

/// Portfolio baskets with target shares; the preset until saved (plan F).
#[tauri::command]
pub async fn asset_classes_get(state: State<'_, AppState>) -> Result<AssetClassesView, IpcError> {
    with_vault(&state, |vault| {
        Ok(allocation_reads::asset_classes(&vault.doc))
    })
}

/// Replace the baskets; shares must add up to 100, an empty list = preset.
#[tauri::command]
pub async fn asset_classes_set(
    classes: Vec<AssetClass>,
    state: State<'_, AppState>,
) -> Result<AssetClassesView, IpcError> {
    with_vault_mut(&state, |vault| {
        allocation::set_asset_classes(vault, classes)
    })
}

/// Put an asset or a store in a basket, or take it out with `None`.
#[tauri::command]
pub async fn asset_class_assign(
    target: AssetClassTarget,
    class_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<AssetClassesView, IpcError> {
    with_vault_mut(&state, |vault| {
        allocation::assign_asset_class(vault, target, class_id)
    })
}

/// Where the free money should go to bring the shares back to target.
#[tauri::command]
pub async fn topup_advice(
    app: AppHandle,
    amount: Option<Amount>,
    state: State<'_, AppState>,
) -> Result<TopupAdvice, IpcError> {
    let zone = current_zone(&app);
    with_vault(&state, |vault| {
        Ok(allocation_reads::topup_advice(
            &vault.books(),
            amount,
            now_secs(),
            &zone,
        ))
    })
}

/// The value story of a property's assets: purchase, dated valuations, the
/// rule and today's change, nominal and real.
#[tauri::command]
pub async fn property_valuations(
    store_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<PropertyValue>, IpcError> {
    let zone = current_zone(&app);
    with_vault(&state, |vault| {
        valuation_reads::property_values(&vault.books(), &StoreId::from(store_id), &zone)
    })
}

/// A dated valuation of a property asset; one on the same day replaces it.
#[tauri::command]
pub async fn property_valuation_add(
    input: ValuationInput,
    state: State<'_, AppState>,
) -> Result<Valuation, IpcError> {
    with_vault_mut(&state, |vault| valuation::add_valuation(vault, input))
}

/// Remove a valuation; the removed one comes back for "Undo".
#[tauri::command]
pub async fn property_valuation_delete(
    id: String,
    state: State<'_, AppState>,
) -> Result<Valuation, IpcError> {
    with_vault_mut(&state, |vault| valuation::delete_valuation(vault, &id))
}

ipc_commands!(portfolio_commands, PORTFOLIO_NAMES, commands::portfolio: [get_summary, get_history, get_asset_holdings, get_investments, get_store_months, get_realized_pnl, get_store_history, get_store_pnl, get_net_worth, set_manual_price, clear_manual_price, asset_classes_get, asset_classes_set, asset_class_assign, topup_advice, property_valuations, property_valuation_add, property_valuation_delete]);
