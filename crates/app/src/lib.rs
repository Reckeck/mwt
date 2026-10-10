//! Tauri entrypoint for the TradeApp desktop shell. All business logic and
//! data lives in `trade-core`; this crate only wires IPC + windows.

mod commands;
mod scheduler;
mod shortcut;

use commands::{
    AppState, analytics, app_settings, auth, calc, categories, contacts, enrichment, exchange,
    losses, planning, portfolio, pricing, settings, stores, transactions, vault_recents,
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
        .plugin(tauri_plugin_notification::init())
        .plugin(shortcut::plugin())
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
            stores::get_store_tree,
            stores::move_store,
            stores::store_move_targets,
            // transactions
            transactions::list_transactions,
            transactions::get_tx_graph,
            transactions::transaction_kinds_for_store_kind,
            transactions::add_transaction,
            transactions::update_transaction,
            transactions::delete_transaction,
            // categories and rules
            categories::list_categories,
            categories::create_category,
            categories::update_category,
            categories::archive_category,
            categories::delete_category,
            categories::list_categorization_rules,
            categories::create_categorization_rule,
            categories::update_categorization_rule,
            categories::delete_categorization_rule,
            categories::reorder_categorization_rules,
            // contacts
            contacts::list_contacts,
            contacts::create_contact,
            contacts::update_contact,
            contacts::archive_contact,
            contacts::delete_contact,
            contacts::get_contact_turnover,
            contacts::get_contact_balances,
            // exchange (read-only prefill)
            exchange::inspect_tx,
            exchange::bybit_executions,
            exchange::exchange_key_status,
            exchange::set_exchange_key,
            exchange::set_exchange_targets,
            exchange::clear_exchange_key,
            exchange::import_bybit_history,
            exchange::list_pending_imports,
            exchange::confirm_pending_imports,
            exchange::discard_pending_imports,
            // pricing
            pricing::search_assets,
            pricing::refresh_all,
            pricing::open_explorer,
            pricing::get_cached_price,
            pricing::convert_amount,
            pricing::get_candles,
            // portfolio
            portfolio::get_summary,
            portfolio::get_history,
            portfolio::get_realized_pnl,
            portfolio::get_store_history,
            portfolio::get_store_pnl,
            portfolio::get_net_worth,
            portfolio::set_manual_price,
            portfolio::clear_manual_price,
            // calculator
            calc::calc_preview,
            // app settings (outside the vault)
            app_settings::get_app_settings,
            app_settings::update_app_settings,
            app_settings::set_quick_add_shortcut,
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
            // losses, chains, rates
            losses::get_tx_loss,
            losses::get_tx_losses,
            losses::list_flows,
            losses::link_transactions,
            losses::unlink_transaction,
            losses::get_fx_table,
            // enrichment
            enrichment::get_enrichments,
            enrichment::apply_enrichment,
            enrichment::reject_enrichment,
            enrichment::re_enrich,
            // analytics
            analytics::get_report,
            analytics::export_report_csv,
            analytics::get_calendar,
            // planning
            planning::list_automations,
            planning::create_automation,
            planning::update_automation,
            planning::delete_automation,
            planning::run_automation,
            planning::list_automation_runs,
            planning::retry_automation_run,
            planning::skip_automation_run,
            planning::list_budgets,
            planning::create_budget,
            planning::update_budget,
            planning::delete_budget,
            planning::get_budget_status,
            planning::get_daily_budget,
            planning::get_month_summary,
            planning::list_goals,
            planning::create_goal,
            planning::update_goal,
            planning::delete_goal,
            planning::get_goal_progress,
            planning::get_goal_calendar,
            planning::list_reminders,
            planning::create_reminder,
            planning::update_reminder,
            planning::delete_reminder,
            planning::set_reminder_done,
            planning::list_price_alerts,
            planning::create_price_alert,
            planning::update_price_alert,
            planning::delete_price_alert,
            planning::get_upcoming,
            planning::list_utility_items,
            planning::create_utility_item,
            planning::update_utility_item,
            planning::delete_utility_item,
            planning::add_meter_reading,
            planning::delete_meter_reading,
            planning::get_meter_trend,
            planning::get_utility_payment_draft,
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

            // Background jobs: Bybit import, automations, alerts, reminders (plan §7.7)
            scheduler::spawn(app.handle().clone());

            // The «+ Операция» key, if the user chose one. A clash only costs
            // the shortcut; the settings screen shows it as not registered.
            let stored = app_settings::current_settings(app.handle()).quick_add_shortcut;
            if let Err(err) = shortcut::apply(app.handle(), stored.as_deref()) {
                eprintln!("quick-add shortcut: {err}");
            }

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
