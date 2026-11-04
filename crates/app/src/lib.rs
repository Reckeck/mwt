//! Tauri entrypoint for the Multi Wallet Tracker shell. All business logic and
//! data lives in `trade-core`; this crate only wires IPC + windows.

mod app_id;
#[cfg(desktop)]
mod autostart;
mod commands;
#[cfg(desktop)]
mod legacy;
mod notify_store;
mod platform_fs;
#[cfg(target_os = "android")]
mod platform_net;
mod quick_add_socket;
mod scheduler;
mod shortcut;
mod sync;
#[cfg(desktop)]
mod tray;
mod updates;
mod vault_writer;

use commands::{
    AppState, analytics, app_settings, attachments, auth, calc, categories, contacts, enrichment,
    exchange, losses, mobile, notifications, planning, portfolio, pricing, rates, sandbox,
    settings, shared_events, sources, stores, transactions, transactions_bulk,
    updates as update_commands, vault_backups, vault_link, vault_recents, workflows,
};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // The channel's identifier (dev.mwt.desktop or mwt.desktop): the keyring
    // and the quick-add socket follow it, as Tauri's directories do.
    let context = tauri::generate_context!();
    app_id::set(&context.config().identifier);
    // `--quick-add` with an instance running: hand it over, start nothing.
    if quick_add_socket::forward_if_running() {
        return;
    }
    // Once, from the TradeApp name (dev channel): before any window opens its storage.
    #[cfg(desktop)]
    legacy::migrate_dirs();
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        // Rust-side file access for document URIs (`platform_fs`); no JS permission.
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_notification::init());
    #[cfg(target_os = "android")]
    let builder = builder
        .plugin(tauri_plugin_saf::init())
        .plugin(tauri_plugin_net::init())
        .plugin(tauri_plugin_insets::init());
    // Autostart, global keys and window geometry exist on desktops only.
    #[cfg(desktop)]
    let builder = builder
        // Off until the user turns it on; a start by the system comes hidden.
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]),
        ))
        .plugin(shortcut::plugin())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_updater::Builder::new().build());
    builder
        .manage(AppState::new())
        .manage(sync::SyncHub::default())
        .manage(sync::handover::Handover::default())
        .manage(updates::UpdateState::default())
        .invoke_handler(tauri::generate_handler![
            // auth
            auth::vault_status,
            auth::create_vault,
            auth::unlock_vault,
            auth::lock_vault_cmd,
            auth::flush_vault,
            auth::exit_without_saving,
            auth::change_password,
            auth::set_vault_password,
            auth::remove_vault_password,
            // sandbox (demo data, erased on exit)
            sandbox::open_sandbox,
            sandbox::reset_sandbox,
            sandbox::close_sandbox,
            sandbox::is_sandbox,
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
            stores::fund_goal,
            stores::move_between_envelopes,
            stores::delete_envelope,
            stores::update_store,
            stores::vehicle_update_mileage,
            stores::vehicle_service_done,
            stores::property_set_revalue,
            stores::archive_store,
            stores::delete_store,
            stores::get_store_overview,
            stores::get_store_tree,
            stores::move_store,
            stores::store_move_targets,
            // transactions
            transactions::list_transactions,
            transactions::list_transactions_page,
            transactions::export_transactions_csv,
            transactions::get_tx_graph,
            transactions::transaction_kinds_for_store_kind,
            transactions::add_transaction,
            transactions::add_linked_transactions,
            transactions::preview_transaction,
            transactions::update_transaction,
            transactions::tx_property_links,
            transactions::delete_transaction,
            transactions_bulk::delete_transactions,
            transactions_bulk::restore_transactions,
            transactions_bulk::set_transaction_categories,
            transactions_bulk::move_transactions,
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
            attachments::list_attachments,
            attachments::add_attachment_link,
            attachments::add_attachment_file,
            attachments::open_attachment,
            attachments::remove_attachment,
            attachments::restore_attachment,
            attachments::rename_attachment,
            attachments::save_attachment_as,
            contacts::list_contacts,
            contacts::create_contact,
            contacts::update_contact,
            contacts::archive_contact,
            contacts::delete_contact,
            contacts::get_contact_turnover,
            contacts::get_contact_balances,
            contacts::list_contact_summaries,
            contacts::list_debt_terms,
            contacts::set_debt_terms,
            // shared events (shared sit-down events)
            shared_events::list_shared_events,
            shared_events::get_shared_event,
            shared_events::create_shared_event,
            shared_events::update_shared_event,
            shared_events::delete_shared_event,
            shared_events::archive_shared_event,
            shared_events::add_shared_participant,
            shared_events::update_shared_participant,
            shared_events::remove_shared_participant,
            shared_events::make_shared_participant_contact,
            shared_events::add_shared_item,
            shared_events::update_shared_item,
            shared_events::delete_shared_item,
            shared_events::set_shared_item_split,
            shared_events::add_shared_purchase,
            shared_events::update_shared_purchase,
            shared_events::delete_shared_purchase,
            shared_events::attach_payment_to_shared_event,
            shared_events::set_shared_share_override,
            shared_events::settle_shared,
            shared_events::write_off_shared,
            shared_events::reassign_shared,
            shared_events::delete_shared_settlement,
            shared_events::set_shared_due,
            shared_events::get_shared_event_share_text,
            shared_events::get_contact_shared_events,
            shared_events::list_shared_tx_links,
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
            portfolio::get_asset_holdings,
            portfolio::get_investments,
            portfolio::get_store_months,
            portfolio::get_store_history,
            portfolio::get_store_pnl,
            portfolio::get_net_worth,
            portfolio::set_manual_price,
            portfolio::clear_manual_price,
            portfolio::property_valuations,
            portfolio::property_valuation_add,
            portfolio::property_valuation_delete,
            portfolio::asset_classes_get,
            portfolio::asset_classes_set,
            portfolio::asset_class_assign,
            portfolio::topup_advice,
            // calculator
            calc::calc_preview,
            calc::calc_invest_simulate,
            calc::calc_invest_solve,
            calc::calc_invest_compare,
            calc::calc_invest_safe_withdrawal,
            calc::calc_scenarios_list,
            calc::calc_scenario_save,
            calc::calc_scenario_delete,
            calc::calc_scenario_restore,
            calc::calc_scenario_prefill,
            // app settings (outside the vault)
            app_settings::get_app_settings,
            app_settings::app_platform,
            mobile::set_system_bars,
            mobile::move_to_back,
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
            analytics::get_report_overview,
            analytics::capital_health,
            analytics::health_settings_set,
            analytics::export_report_csv,
            analytics::get_calendar,
            analytics::get_month_balance,
            analytics::set_carry_over,
            analytics::get_spending_overview,
            analytics::get_month_pace,
            analytics::get_net_worth_trend,
            analytics::get_dashboard_today,
            analytics::get_scenario_status,
            analytics::get_entity_card,
            analytics::get_search_index,
            analytics::search_transactions,
            analytics::get_pinned,
            analytics::set_pinned,
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
            workflows::preview_workflow_import,
            workflows::workflow_templates,
            workflows::workflow_from_template,
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
            planning::confirm_estimated_amount,
            planning::detach_automation,
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
            planning::restore_goal,
            planning::get_goal_progress,
            planning::get_goal_calendar,
            planning::goal_from_scenario,
            planning::emergency_fund_get,
            planning::emergency_fund_set,
            planning::emergency_fund_toggle,
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
            planning::list_my_automations,
            planning::set_my_automation_active,
            planning::get_automation_card,
            planning::preview_automation_dates,
            planning::list_automation_journal,
            planning::set_loyalty_info,
            planning::create_loyalty_program,
            planning::adjust_points,
            planning::get_cashback_report,
            planning::set_deposit_terms,
            planning::get_deposit_report,
            planning::get_interest_tax,
            rates::refresh_rates,
            rates::get_inflation_overview,
            rates::set_inflation_mode,
            sources::datasource_preview,
            sources::datasource_test,
            sources::datasources_list,
            sources::datasource_save,
            sources::datasource_delete,
            sources::datasource_refresh,
            sources::datasource_confirm,
            sources::set_inflation_source,
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
            update_commands::update_status,
            update_commands::update_check_now,
            update_commands::update_restart_now,
            vault_recents::recent_vaults,
            vault_recents::forget_recent_vault,
            vault_recents::restore_recent_vault,
            vault_recents::relocate_recent_vault,
            vault_recents::delete_vault_file,
            vault_recents::export_vault_copy,
            vault_recents::pick_vault_file,
            // external copy of a vault
            vault_backups::list_vault_backups,
            vault_backups::create_vault_backup,
            vault_backups::restore_vault_backup,
            vault_backups::reveal_vault_folder,
            vault_link::vault_link_status,
            vault_link::link_vault_external,
            vault_link::unlink_vault_external,
            vault_link::resolve_vault_conflict,
            // device-to-device sync
            commands::sync::sync_status,
            commands::sync::sync_set_settings,
            commands::sync::sync_pair,
            commands::sync::sync_dismiss_offer,
            commands::sync::sync_confirm_pairing,
            commands::sync::sync_decide_plan,
            commands::sync::sync_peers,
            commands::sync::sync_forget_peer,
            commands::sync::sync_resume_peer,
            commands::sync::sync_history,
            commands::sync::sync_journal,
            commands::sync::sync_undo,
            commands::sync::allow_merging,
            commands::sync::sync_restore_backup,
            commands::sync::sync_open_firewall,
            commands::transfer::transfer_receive_start,
            commands::transfer::transfer_receive_stop,
            commands::transfer::transfer_receive_decide,
            commands::transfer::transfer_browse_start,
            commands::transfer::transfer_browse_stop,
            commands::transfer::transfer_send,
            commands::transfer::transfer_send_code,
            commands::transfer::transfer_send_cancel,
        ])
        .setup(|app| {
            // Before any vault is opened: what a crashed run left of the sandbox goes.
            sandbox::erase_on_start(app.handle());
            // Resolve the default vault location once (`platform_fs::vaults_dir`).
            let default_path = platform_fs::default_vault_path(app.handle());
            let state = app.state::<AppState>();
            *state.vault_path.lock().expect("vault_path lock") = default_path;

            // What was scheduled to be told while the vault is locked survives a
            // restart (encrypted with a key from the OS keyring).
            if let Ok(mut notify) = state.notify.lock() {
                *notify = notify_store::load(app.handle());
            }

            // Batched vault writes: one write per burst of changes, off the vault lock.
            vault_writer::spawn(app.handle().clone());
            // Background jobs: Bybit import, automations, alerts, reminders (plan §7.7)
            scheduler::spawn(app.handle().clone());

            // The "+ Transaction" key, if the user chose one. A clash only costs
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
            #[cfg(desktop)]
            autostart::guard_entry(app.handle());
            // Until the page paints, the window shows its own background: the
            // saved theme's, so a dark theme does not open white.
            #[cfg(desktop)]
            if let Some(window) = app.get_webview_window("main") {
                use trade_core::appconfig::Theme;
                let dark = match app_settings::current_settings(app.handle()).theme {
                    Theme::Dark => true,
                    Theme::Light => false,
                    Theme::Auto => matches!(window.theme(), Ok(tauri::Theme::Dark)),
                };
                let color = if dark {
                    tauri::window::Color(9, 9, 11, 255)
                } else {
                    tauri::window::Color(255, 255, 255, 255)
                };
                let _ = window.set_background_color(Some(color));
            }
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
            // Whose window has focus decides where a notification shows.
            if let WindowEvent::Focused(focused) = event {
                sync::set_focus(window.app_handle(), *focused);
            }
            if let WindowEvent::CloseRequested { api, .. } = event {
                // A device setting: kept in the app config, not in the vault.
                let minimize_to_tray =
                    commands::app_settings::current_settings(window.app_handle())
                        .minimize_to_tray();
                if minimize_to_tray {
                    window.hide().ok();
                    api.prevent_close();
                    // Hidden to the tray: what was batched goes to disk now, off the UI thread.
                    let app = window.app_handle().clone();
                    std::thread::spawn(move || {
                        if let Err(err) = vault_writer::flush_now(&app) {
                            eprintln!("vault: save on hide failed: {err}");
                        }
                    });
                }
            }
        })
        .build(context)
        .expect("error while building Multi Wallet Tracker")
        .run(|app, event| match event {
            // Quitting (window closed, tray «Quit», session end) writes the batched
            // changes first; if that fails, the app stays and asks.
            tauri::RunEvent::ExitRequested { api, .. } => {
                if let Err(err) = vault_writer::flush_now(app)
                    && !vault_writer::exit_unsaved_allowed()
                {
                    api.prevent_exit();
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                    let _ = tauri::Emitter::emit(
                        app,
                        vault_writer::SAVE_FAILED_EVENT,
                        serde_json::json!({ "reason": err.to_string(), "exiting": true }),
                    );
                }
            }
            tauri::RunEvent::Exit => {
                if let Err(err) = vault_writer::flush_now(app) {
                    eprintln!("vault: unsaved changes lost on exit: {err}");
                }
                // A downloaded update goes in only on a real exit, after the save.
                if let Err(err) = updates::apply_pending(app) {
                    eprintln!("updates: install on exit failed: {err}");
                }
                // The demo data does not outlive the run.
                sandbox::erase_on_exit(app);
            }
            _ => {}
        });
}
