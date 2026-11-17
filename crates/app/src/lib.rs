//! Tauri entrypoint for the Multi Wallet Tracker shell. All business logic and
//! data lives in `trade-core`; this crate only wires IPC + windows.

mod app_id;
mod autolock;
#[cfg(desktop)]
mod autostart;
mod biometric;
mod commands;
mod cryptotrack;
mod ipc;
#[cfg(desktop)]
mod legacy;
mod notify_store;
mod platform_fs;
#[cfg(target_os = "android")]
mod platform_net;
mod quick_add_socket;
mod scheduler;
mod shortcut;
mod state;
mod sync;
#[cfg(desktop)]
mod tray;
mod updates;
mod vault_writer;

use commands::{AppState, app_settings, sandbox};
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
        .plugin(ipc::opener::Opener::plugin())
        .plugin(tauri_plugin_dialog::init())
        // Rust-side file access for document URIs (`platform_fs`); no JS permission.
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_notification::init());
    #[cfg(target_os = "android")]
    let builder = builder
        .plugin(tauri_plugin_saf::init())
        .plugin(tauri_plugin_net::init())
        .plugin(tauri_plugin_insets::init())
        .plugin(tauri_plugin_biometric::init());
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
        .manage(state::LockBus::default())
        .manage(sync::SyncHub::default())
        .manage(sync::handover::Handover::default())
        .manage(updates::UpdateState::default())
        .invoke_handler(commands::analytics::analytics_commands!(
            [
                commands::app_settings::app_settings_commands,
                commands::attachments::attachments_commands,
                commands::auth::auth_commands,
                commands::calc::calc_commands,
                commands::categories::categories_commands,
                commands::contacts::contacts_commands,
                commands::cryptotrack::accounts::cryptotrack_accounts_commands,
                commands::cryptotrack::links::cryptotrack_links_commands,
                commands::cryptotrack::network::cryptotrack_network_commands,
                commands::cryptotrack::operations::cryptotrack_operations_commands,
                commands::cryptotrack::positions::cryptotrack_positions_commands,
                commands::cryptotrack::relations::cryptotrack_relations_commands,
                commands::cryptotrack::sync::cryptotrack_sync_commands,
                commands::cryptotrack::tokens::cryptotrack_tokens_commands,
                commands::cryptotrack::transition::cryptotrack_transition_commands,
                commands::cryptotrack::wipe::cryptotrack_wipe_commands,
                commands::enrichment::enrichment_commands,
                commands::exchange::exchange_commands,
                commands::losses::losses_commands,
                commands::mobile::mobile_commands,
                commands::notifications::notifications_commands,
                commands::planning::planning_commands,
                commands::portfolio::portfolio_commands,
                commands::pricing::pricing_commands,
                commands::rates::rates_commands,
                commands::sandbox::sandbox_commands,
                commands::security::autolock::security_autolock_commands,
                commands::security::biometric::security_biometric_commands,
                commands::security::keys::security_keys_commands,
                commands::security::raise::security_raise_commands,
                commands::security::state::security_state_commands,
                commands::settings::settings_commands,
                commands::shared_events::shared_events_commands,
                commands::sources::sources_commands,
                commands::stores::stores_commands,
                commands::sync::sync_commands,
                commands::transactions_bulk::transactions_bulk_commands,
                commands::transactions::transactions_commands,
                commands::transfer::transfer_commands,
                commands::updates::updates_commands,
                commands::vault_backups::vault_backups_commands,
                commands::vault_link::vault_link_commands,
                commands::vault_recents::vault_recents_commands,
                commands::workflows::workflows_commands,
                ipc::commands::generate_handler,
            ];
        ))
        .setup(|app| {
            // Before any vault is opened: what a crashed run left of the sandbox goes.
            sandbox::erase_on_start(app.handle());
            // Whatever must follow a lock listens on the lock bus, once.
            state::register_lock_subscribers(app.handle());
            // Decrypted attachments a crashed run left for other programs go too.
            commands::attachments::wipe_opened(app.handle());
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
            // Idle, sleep and screen-lock locking (H4).
            autolock::spawn(app.handle().clone());

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
                commands::attachments::wipe_opened(app);
            }
            _ => {}
        });
}
