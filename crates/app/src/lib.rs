//! Tauri entrypoint for the Multi Wallet Tracker shell. All business logic and
//! data lives in `trade-core`; this crate only wires IPC + windows.

mod commands;
#[cfg(desktop)]
mod legacy;
mod notify_store;
mod quick_add_socket;
mod scheduler;
mod shortcut;
#[cfg(desktop)]
mod tray;

use commands::{
    AppState, analytics, app_settings, auth, calc, categories, contacts, enrichment, exchange,
    losses, notifications, planning, portfolio, pricing, settings, stores, transactions,
    vault_recents, workflows,
};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // `--quick-add` with an instance running: hand it over, start nothing.
    if quick_add_socket::forward_if_running() {
        return;
    }
    // Once, from the TradeApp name: before any window opens its storage.
    #[cfg(desktop)]
    legacy::migrate_dirs();
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init());
    // Autostart, global keys and window geometry exist on desktops only.
    #[cfg(desktop)]
    let builder = builder
        // Off until the user turns it on; a start by the system comes hidden.
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]),
        ))
        .plugin(shortcut::plugin())
        .plugin(tauri_plugin_window_state::Builder::default().build());
    builder
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
            stores::get_wallet_catalog,
            stores::create_store,
            stores::plan_store_change,
            stores::apply_store_change,
            stores::list_envelopes,
            stores::create_envelope,
            stores::update_envelope,
            stores::fund_envelope,
            stores::move_between_envelopes,
            stores::delete_envelope,
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
            transactions::add_linked_transactions,
            transactions::preview_transaction,
            transactions::update_transaction,
            transactions::delete_transaction,
            transactions::get_adjustment_draft,
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
            contacts::list_debt_terms,
            contacts::set_debt_terms,
            // exchange (read-only prefill)
            exchange::inspect_tx,
            exchange::bybit_executions,
            exchange::exchange_key_status,
            exchange::set_exchange_key,
            exchange::set_exchange_targets,
            exchange::set_import_interval,
            exchange::get_terminal_state,
            exchange::update_terminal_state,
            exchange::save_terminal_drawings,
            exchange::clear_exchange_key,
            exchange::import_bybit_history,
            exchange::get_import_settings,
            exchange::preview_bybit_import,
            exchange::set_import_settings,
            exchange::list_import_runs,
            exchange::list_imported_transactions,
            exchange::create_bybit_accounts,
            exchange::bybit_balances,
            exchange::list_pending_imports,
            exchange::confirm_pending_imports,
            exchange::discard_pending_imports,
            // pricing
            pricing::search_assets,
            pricing::refresh_all,
            pricing::ensure_fx,
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
            app_settings::get_shortcut_support,
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
            workflows::workflow_node_catalog,
            workflows::validate_workflow,
            workflows::list_workflows,
            workflows::get_workflow,
            workflows::save_workflow,
            workflows::set_workflow_active,
            workflows::delete_workflow,
            workflows::restore_workflow_version,
            workflows::dry_run_workflow,
            workflows::list_workflow_waits,
            workflows::save_workflow_draft,
            workflows::discard_workflow_draft,
            workflows::answer_workflow_wait,
            workflows::run_workflow_now,
            workflows::list_workflow_runs,
            workflows::list_workflow_queue,
            workflows::confirm_workflow_queued,
            workflows::discard_workflow_queued,
            workflows::list_workflow_vars,
            workflows::set_workflow_var,
            workflows::delete_workflow_var,
            workflows::export_workflow,
            workflows::import_workflow,
            workflows::save_workflow_test,
            workflows::delete_workflow_test,
            workflows::run_workflow_tests,
            planning::list_automations,
            planning::create_automation,
            planning::update_automation,
            planning::delete_automation,
            planning::run_automation,
            planning::simulate_automations,
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
            planning::finish_reminder_series,
            planning::snooze_reminder,
            notifications::list_notifications,
            notifications::mark_notifications_read,
            notifications::send_test_notification,
            notifications::notification_permission,
            notifications::get_autostart,
            notifications::set_autostart,
            notifications::notification_storage,
            planning::list_price_alerts,
            planning::create_price_alert,
            planning::update_price_alert,
            planning::delete_price_alert,
            planning::get_upcoming,
            planning::set_loyalty_info,
            planning::adjust_points,
            planning::get_cashback_report,
            planning::set_deposit_terms,
            planning::get_deposit_report,
            planning::get_interest_tax,
            planning::refresh_key_rates,
            planning::list_utility_items,
            planning::create_utility_item,
            planning::update_utility_item,
            planning::delete_utility_item,
            planning::add_meter_reading,
            planning::delete_meter_reading,
            planning::update_meter_reading,
            planning::get_utility_bill,
            planning::pay_utility_bill,
            planning::preview_utility_formula,
            planning::get_meter_trend,
            planning::get_property_summary,
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

            // What was scheduled to be told while the vault is locked survives a
            // restart (encrypted with a key from the OS keyring).
            if let Ok(mut notify) = state.notify.lock() {
                *notify = notify_store::load(app.handle());
            }

            // Background jobs: Bybit import, automations, alerts, reminders (plan §7.7)
            scheduler::spawn(app.handle().clone());

            // The «+ Операция» key, if the user chose one. A clash only costs
            // the shortcut; the settings screen shows it as not registered.
            let stored = app_settings::current_settings(app.handle()).quick_add_shortcut;
            if let Err(err) = shortcut::apply(app.handle(), stored.as_deref()) {
                eprintln!("quick-add shortcut: {err}");
            }

            // Create system tray with context menu
            #[cfg(desktop)]
            tray::create(app)?;
            #[cfg(desktop)]
            legacy::migrate_autostart(app.handle());
            // Started with the system: stay in the tray, remind from there.
            if std::env::args().any(|arg| arg == "--hidden")
                && let Some(window) = app.get_webview_window("main")
            {
                let _ = window.hide();
            }
            quick_add_socket::listen(app.handle().clone());
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
        .expect("error while running Multi Wallet Tracker");
}
