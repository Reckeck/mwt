//! Tauri entrypoint for the TradeApp desktop shell. All business logic and
//! data lives in `trade-core`; this crate only wires IPC + windows.

mod commands;

use commands::{
    AppState, auth, calc, categories, exchange, portfolio, pricing, settings, sim, stores,
    transactions,
};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            // auth
            auth::vault_status,
            auth::create_vault,
            auth::unlock_vault,
            auth::lock_vault_cmd,
            auth::change_password,
            // stores
            stores::list_stores,
            stores::create_store,
            stores::update_store,
            stores::archive_store,
            stores::delete_store,
            stores::get_store_overview,
            // transactions
            transactions::list_transactions,
            transactions::add_transaction,
            transactions::add_transfer,
            transactions::add_trade,
            transactions::delete_transaction,
            transactions::update_comment,
            // categories
            categories::add_category,
            categories::list_categories,
            // exchange (read-only prefill)
            exchange::inspect_tx,
            exchange::bybit_executions,
            exchange::exchange_key_status,
            exchange::set_exchange_key,
            exchange::clear_exchange_key,
            // pricing
            pricing::search_assets,
            pricing::refresh_all,
            pricing::open_explorer,
            pricing::set_tx_prices,
            pricing::get_cached_price,
            // portfolio
            portfolio::get_summary,
            portfolio::get_history,
            portfolio::get_realized_pnl,
            portfolio::get_store_history,
            portfolio::get_store_pnl,
            // calculator
            calc::calc_preview,
            // settings
            settings::get_settings,
            settings::update_settings,
            // sim (strategy lab)
            sim::save_strategy,
            sim::list_strategies,
            sim::delete_strategy,
            sim::list_runs,
            sim::run_strategy,
        ])
        .setup(|app| {
            // Resolve the default vault location once (XDG/AppData).
            let data_dir = app.path().app_data_dir().expect("app data dir");
            let default_path = data_dir.join("vault.wallet");
            let state = app.state::<AppState>();
            *state.vault_path.lock().expect("vault_path lock") = default_path;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running TradeApp");
}
