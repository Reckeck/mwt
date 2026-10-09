//! Tauri entrypoint for the TradeApp desktop shell. All business logic and
//! data lives in `trade-core`; this crate only wires IPC + windows.

mod commands;

use commands::{
    AppState, app_settings, auth, calc, categories, err_to_string, exchange, portfolio, pricing,
    settings, stores, transactions, vault_recents,
};
use tauri::{
    Emitter, Manager,
    menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
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
            transactions::transaction_kinds_for_store_kind,
            transactions::add_income,
            transactions::add_expense,
            transactions::add_payment,
            transactions::add_fee,
            transactions::add_bank_transfer,
            transactions::add_card_payment,
            transactions::add_wallet_transfer,
            transactions::add_defi_action,
            transactions::add_cash_flow,
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
            exchange::import_bybit_history,
            exchange::list_pending_imports,
            exchange::confirm_pending_imports,
            exchange::discard_pending_imports,
            // pricing
            pricing::search_assets,
            pricing::refresh_all,
            pricing::open_explorer,
            pricing::set_tx_prices,
            pricing::get_cached_price,
            pricing::get_candles,
            // portfolio
            portfolio::get_summary,
            portfolio::get_history,
            portfolio::get_realized_pnl,
            portfolio::get_store_history,
            portfolio::get_store_pnl,
            // calculator
            calc::calc_preview,
            // app settings (outside the vault)
            app_settings::get_app_settings,
            app_settings::update_app_settings,
            app_settings::get_supported_currencies,
            // settings
            settings::get_settings,
            settings::update_settings,
            settings::dev_dump_vault,
            settings::vault_schema,
            settings::dev_export_vault,
            settings::dev_apply_vault_json,
            settings::dev_open_vault_json_in_editor,
            settings::dev_reload_vault_json_from_editor,
            // vault recents
            vault_recents::recent_vaults,
            vault_recents::forget_recent_vault,
            vault_recents::pick_vault_file,
        ])
        .setup(|app| {
            // Resolve the default vault location once (XDG/AppData).
            let data_dir = app.path().app_data_dir().expect("app data dir");
            let default_path = data_dir.join("vault.wallet");
            let state = app.state::<AppState>();
            *state.vault_path.lock().expect("vault_path lock") = default_path;

            // Spawn background Bybit import scheduler (D2.4)
            spawn_bybit_import_scheduler(app.handle().clone());

            // Create system tray with context menu
            create_tray(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            use tauri::WindowEvent;
            if let WindowEvent::CloseRequested { api, .. } = event {
                // Check if minimize_to_tray is enabled in settings
                // For now, always minimize to tray instead of closing
                let state = window.state::<AppState>();
                let minimize_to_tray = state
                    .vault
                    .lock()
                    .ok()
                    .and_then(|guard| {
                        guard
                            .as_ref()
                            .map(|vault| vault.doc.settings.minimize_to_tray)
                    })
                    .unwrap_or(true);
                if minimize_to_tray {
                    window.hide().ok();
                    api.prevent_close();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running TradeApp");
}

fn create_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let app_handle = app.handle().clone();

    // Create tray menu items
    let show_hide = MenuItemBuilder::new("Показать / Скрыть")
        .id("show_hide")
        .accelerator("CmdOrCtrl+Shift+H")
        .build(&app_handle)?;

    let separator1 = PredefinedMenuItem::separator(&app_handle)?;

    let dashboard = MenuItemBuilder::new("Дашборд")
        .id("nav_dashboard")
        .build(&app_handle)?;

    let wallets = MenuItemBuilder::new("Кошельки")
        .id("nav_stores")
        .build(&app_handle)?;

    let transactions = MenuItemBuilder::new("Операции")
        .id("nav_transactions")
        .build(&app_handle)?;

    let charts = MenuItemBuilder::new("Графики")
        .id("nav_charts")
        .build(&app_handle)?;

    let trading = MenuItemBuilder::new("Терминал")
        .id("nav_trading")
        .build(&app_handle)?;

    let strategy_lab = MenuItemBuilder::new("Лаборатория")
        .id("nav_strategy_lab")
        .build(&app_handle)?;

    let settings = MenuItemBuilder::new("Настройки")
        .id("nav_settings")
        .build(&app_handle)?;

    let separator2 = PredefinedMenuItem::separator(&app_handle)?;

    let refresh_prices = MenuItemBuilder::new("Обновить цены")
        .id("refresh_prices")
        .accelerator("CmdOrCtrl+R")
        .build(&app_handle)?;

    let separator3 = PredefinedMenuItem::separator(&app_handle)?;

    let lock_unlock = MenuItemBuilder::new("Заблокировать")
        .id("lock_unlock")
        .build(&app_handle)?;

    let separator4 = PredefinedMenuItem::separator(&app_handle)?;

    let quit = MenuItemBuilder::new("Выйти")
        .id("quit")
        .accelerator("CmdOrCtrl+Q")
        .build(&app_handle)?;

    let menu = MenuBuilder::new(&app_handle)
        .items(&[
            &show_hide,
            &separator1,
            &dashboard,
            &wallets,
            &transactions,
            &charts,
            &trading,
            &strategy_lab,
            &settings,
            &separator2,
            &refresh_prices,
            &separator3,
            &lock_unlock,
            &separator4,
            &quit,
        ])
        .build()?;

    // Build tray icon
    let app_handle_tray = app_handle.clone();
    let app_handle_menu = app_handle.clone();
    let _tray = TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(move |_tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                ..
            } = event
            {
                let window = app_handle_tray.get_webview_window("main").unwrap();
                let _ = if window.is_visible().unwrap_or(false) {
                    window.hide()
                } else {
                    window.show().and_then(|_| window.set_focus())
                };
            }
        })
        .on_menu_event(move |_tray, event| {
            match event.id.as_ref() {
                "show_hide" => {
                    let window = app_handle_menu.get_webview_window("main").unwrap();
                    let _ = if window.is_visible().unwrap_or(false) {
                        window.hide()
                    } else {
                        window.show().and_then(|_| window.set_focus())
                    };
                }
                "nav_dashboard" => {
                    let window = app_handle_menu.get_webview_window("main").unwrap();
                    let _ = window.show().and_then(|_| window.set_focus());
                    let _ = window.eval("window.location.href = '/'");
                }
                "nav_stores" => {
                    let window = app_handle_menu.get_webview_window("main").unwrap();
                    let _ = window.show().and_then(|_| window.set_focus());
                    let _ = window.eval("window.location.href = '/stores'");
                }
                "nav_transactions" => {
                    let window = app_handle_menu.get_webview_window("main").unwrap();
                    let _ = window.show().and_then(|_| window.set_focus());
                    let _ = window.eval("window.location.href = '/transactions'");
                }
                "nav_charts" => {
                    let window = app_handle_menu.get_webview_window("main").unwrap();
                    let _ = window.show().and_then(|_| window.set_focus());
                    let _ = window.eval("window.location.href = '/charts'");
                }
                "nav_trading" => {
                    let window = app_handle_menu.get_webview_window("main").unwrap();
                    let _ = window.show().and_then(|_| window.set_focus());
                    let _ = window.eval("window.location.href = '/trading'");
                }
                "nav_strategy_lab" => {
                    let window = app_handle_menu.get_webview_window("main").unwrap();
                    let _ = window.show().and_then(|_| window.set_focus());
                    let _ = window.eval("window.location.href = '/strategy-lab'");
                }
                "nav_settings" => {
                    let window = app_handle_menu.get_webview_window("main").unwrap();
                    let _ = window.show().and_then(|_| window.set_focus());
                    let _ = window.eval("window.location.href = '/settings'");
                }
                "refresh_prices" => {
                    // Emit event to frontend to trigger refresh
                    let _ = app_handle_menu.emit("refresh-prices", ());
                }
                "lock_unlock" => {
                    let _ = app_handle_menu.emit("toggle-lock", ());
                }
                "quit" => {
                    std::process::exit(0);
                }
                _ => {}
            }
        })
        .build(&app_handle)?;

    Ok(())
}

/// How often the scheduler wakes up to re-read the interval setting.
const SCHEDULER_TICK_SEC: u64 = 30;

/// Poll Bybit on the interval from `settings.bybit_import_interval_sec`
/// (`0` disables it) and stage any new fills for the user to confirm.
///
/// Nothing is ever written to `transactions` from here: fills land in
/// `pending_imports` and the user confirms them explicitly.
fn spawn_bybit_import_scheduler(app_handle: tauri::AppHandle) {
    std::thread::spawn(move || {
        let mut last_run: Option<std::time::Instant> = None;
        loop {
            std::thread::sleep(std::time::Duration::from_secs(SCHEDULER_TICK_SEC));

            // Snapshot the settings under the lock, then drop the guard: the
            // guard must never live across `.await` (AGENTS.md rule 3).
            let (interval_sec, configured) = {
                let state = app_handle.state::<AppState>();
                let Ok(guard) = state.vault.lock() else {
                    continue;
                };
                let Some(vault) = guard.as_ref() else {
                    continue; // locked: nothing to poll for
                };
                let has_key = !vault.doc.exchanges.bybit.is_empty();
                (vault.doc.settings.bybit_import_interval_sec, has_key)
            };
            if interval_sec == 0 || !configured {
                continue;
            }

            let interval = std::time::Duration::from_secs(interval_sec as u64);
            if last_run.is_some_and(|previous| previous.elapsed() < interval) {
                continue;
            }
            last_run = Some(std::time::Instant::now());

            // Soft-fail by design: a poll that cannot run leaves the staging
            // area untouched and simply retries on the next tick.
            let Ok(staged) = tauri::async_runtime::block_on(poll_bybit_once(&app_handle)) else {
                continue;
            };
            if staged > 0 {
                let _ = app_handle.emit(
                    "bybit-import-pending",
                    serde_json::json!({ "staged": staged }),
                );
            }
        }
    });
}

/// One polling pass over every store that has a Bybit key. Returns how many
/// fills were newly staged.
async fn poll_bybit_once(app_handle: &tauri::AppHandle) -> Result<u32, String> {
    let state = app_handle.state::<AppState>();
    let keys: Vec<(String, String, String)> = {
        let guard = state.vault.lock().map_err(|err| err.to_string())?;
        let vault = guard.as_ref().ok_or("REQUIRES_UNLOCK: vault is locked")?;
        vault
            .doc
            .exchanges
            .bybit
            .iter()
            .filter(|entry| !entry.api_key.is_empty() && !entry.api_secret.is_empty())
            .map(|entry| {
                (
                    entry.store_id.clone(),
                    entry.api_key.clone(),
                    entry.api_secret.clone(),
                )
            })
            .collect()
    };

    let client = trade_core::exchange::BybitClient::default();
    let mut staged_total = 0u32;

    for (store_id, api_key, api_secret) in keys {
        // Network call happens with no guard held.
        let execs = client
            .all_executions(&api_key, &api_secret, 500)
            .await
            .map_err(err_to_string)?;

        let imports: Vec<trade_core::model::vault::PendingImport> = execs
            .iter()
            .filter_map(|execution| {
                trade_core::model::vault::PendingImport::from_execution(
                    &store_id,
                    execution.category_hint(),
                    execution,
                )
            })
            .collect();

        let mut guard = state.vault.lock().map_err(|err| err.to_string())?;
        let Some(vault) = guard.as_mut() else {
            return Err("REQUIRES_UNLOCK: vault was locked mid-poll".into());
        };
        staged_total += vault.stage_imports(imports).map_err(err_to_string)?;
    }

    Ok(staged_total)
}
