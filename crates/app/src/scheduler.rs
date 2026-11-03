//! Background jobs (plan §7.7): one thread, a tick every
//! [`SCHEDULER_TICK_SEC`] seconds, only while a vault is unlocked. Each job
//! soft-fails — a pass that cannot run leaves everything as it was and the
//! next tick tries again.
//!
//! 1. Bybit import, each exchange on its own
//!    `ExchangeProfile.import_interval_sec` — fills are staged, never written
//!    as transactions.
//! 2. Automations on every tick (`run_due_automations`).
//! 3. Price alerts: prices of their assets refreshed at most every
//!    [`ALERT_REFRESH_SEC`], then `check_alerts`.
//! 4. Reminders: each due reminder is announced once per session.
//!
//! The vault guard is never held across `.await` (AGENTS.md rule 3).

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

use trade_core::appconfig::{Locale, load_settings};
use trade_core::calendar::now_secs;
use trade_core::ipc_contract::{AutomationRejectedEvent, SyncRole};
use trade_core::model::notify::{NoticeCategory, NoticeRecord};
use trade_core::model::planning::{AlertCondition, RejectReason};
use trade_core::model::transaction::Transaction;
use trade_core::service::{AlertBase, UnlockedVault, notice, notice_title};
use trade_core::workflow::RunOutcome;

use crate::commands::AppState;
use crate::commands::app_settings::current_zone;

/// How often the scheduler wakes up.
const SCHEDULER_TICK_SEC: u64 = 30;
/// Least time between two price refreshes for alerts.
const ALERT_REFRESH_SEC: u64 = 5 * 60;
/// Inflation and the key rate: once a day, an hour after a failure.
const RATES_REFRESH_SEC: u64 = 24 * 60 * 60;
const RATES_RETRY_SEC: u64 = 60 * 60;
/// Automatic update check: the first one on the first tick (~30 s after start), then every 6 hours.
const UPDATE_CHECK_SEC: u64 = 6 * 60 * 60;

/// What the scheduler remembers between ticks.
#[derive(Default)]
struct Memory {
    /// Last import poll per exchange store.
    last_bybit: HashMap<String, Instant>,
    last_alert_refresh: Option<Instant>,
    /// Last automatic price refresh (the interval chosen on this device).
    last_price_refresh: Option<Instant>,
    /// When inflation and the key rate are fetched next (`None`: now).
    next_rates: Option<Instant>,
    /// When the next automatic update check runs (`None`: this tick).
    next_update_check: Option<Instant>,
}

/// How often the snapshot for a locked vault is taken while it is open.
const SNAPSHOT_REFRESH_SEC: u64 = 60;

pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || {
        let mut memory = Memory::default();
        loop {
            std::thread::sleep(Duration::from_secs(SCHEDULER_TICK_SEC));
            // Needs no vault: the lock screen may sit open for days.
            updates_tick(&app, &mut memory);
            if !is_unlocked(&app) {
                locked_tick(&app);
                continue;
            }
            // First take in what was told while locked, so nothing repeats
            // (a client drops it: see `merge_findings`).
            merge_locked(&app);
            // In a sync group only the main device runs the jobs; the
            // others get their results by sync (docs/sync.md).
            if crate::sync::runs_jobs(&app) {
                // The sandbox talks to no exchange and fetches no user source.
                let sandbox = crate::commands::sandbox::is_active_app(&app);
                if !sandbox {
                    bybit_tick(&app, &mut memory);
                }
                prices_tick(&app, &mut memory);
                rates_tick(&app, &mut memory);
                if !sandbox {
                    sources_tick(&app);
                }
                workflows_tick(&app);
                alerts_tick(&app, &mut memory);
                reminders_tick(&app, &mut memory);
            }
            snapshot_tick(&app, false);
        }
    });
}

/// Look for a new version when the user allows it (Settings → About).
/// The network part runs on the async runtime, not on this thread.
fn updates_tick(app: &AppHandle, memory: &mut Memory) {
    if memory
        .next_update_check
        .is_some_and(|at| Instant::now() < at)
    {
        return;
    }
    memory.next_update_check = Some(Instant::now() + Duration::from_secs(UPDATE_CHECK_SEC));
    if !crate::commands::app_settings::current_settings(app).auto_update()
        || !crate::updates::auto_check_due(app)
    {
        return;
    }
    tauri::async_runtime::spawn(crate::updates::run_check(app.clone(), false));
}

/// Whether quotes are worth fetching now: the window is on screen, or this is
/// the main device and the user works on another device of the group.
fn should_refresh_prices(visible: bool, role: SyncRole, any_link_focused: bool) -> bool {
    visible || (role == SyncRole::Main && any_link_focused)
}

/// Refresh prices and rates every «Price refresh interval» (Settings → This
/// device; 0 = off) while the window is on screen: a window hidden in the tray
/// does not need fresh quotes, and the next look refreshes them anyway.
fn prices_tick(app: &AppHandle, memory: &mut Memory) {
    let state = app.state::<AppState>();
    let vault_interval = state.vault.lock().ok().and_then(|guard| {
        guard
            .as_ref()
            .map(|vault| vault.doc.settings.fx_refresh_interval_sec)
    });
    let interval = crate::commands::app_settings::current_settings(app)
        .fx_refresh_interval_sec
        .or(vault_interval)
        .unwrap_or(300);
    if interval == 0 {
        return;
    }
    if memory
        .last_price_refresh
        .is_some_and(|last| last.elapsed() < Duration::from_secs(u64::from(interval)))
    {
        return;
    }
    let visible = app
        .get_webview_window("main")
        .is_some_and(|window| window.is_visible().unwrap_or(true));
    let role = crate::sync::current_role(app).unwrap_or(SyncRole::Main);
    if !should_refresh_prices(visible, role, crate::sync::any_link_focused(app)) {
        return;
    }
    memory.last_price_refresh = Some(Instant::now());
    // Offline is fine: the cached prices stay, labelled with their date.
    let _ = tauri::async_runtime::block_on(crate::commands::pricing::refresh_all(
        app.clone(),
        app.state::<AppState>(),
    ));
}

/// Inflation and the key rate from the statistics offices; offline just
/// tries again later.
fn rates_tick(app: &AppHandle, memory: &mut Memory) {
    if memory.next_rates.is_some_and(|next| Instant::now() < next) {
        return;
    }
    let fetched = tauri::async_runtime::block_on(crate::commands::rates::refresh_rates_now(app));
    let wait = if fetched.is_ok() {
        RATES_REFRESH_SEC
    } else {
        RATES_RETRY_SEC
    };
    memory.next_rates = Some(Instant::now() + Duration::from_secs(wait));
}

/// The user's data sources whose interval has passed (each keeps its own
/// `checked_at`, so nothing is remembered here).
fn sources_tick(app: &AppHandle) {
    tauri::async_runtime::block_on(crate::commands::sources::refresh_due_sources(app));
}

fn is_unlocked(app: &AppHandle) -> bool {
    let state = app.state::<AppState>();
    state.vault.lock().is_ok_and(|guard| guard.is_some())
}

/// The text for the interface language: `ru_text` when `ru`, else `en_text`.
/// `ru` comes from [`russian`]; every text shown outside the window goes through here.
pub fn pick(ru: bool, ru_text: &str, en_text: &str) -> String {
    if ru { ru_text } else { en_text }.to_string()
}

/// Whether texts shown outside the window should be in Russian.
pub fn russian(app: &AppHandle) -> bool {
    let Ok(dir) = app.path().app_config_dir() else {
        return false;
    };
    match load_settings(&dir).locale {
        Locale::Ru => true,
        Locale::En => false,
        Locale::Auto => system_language(app).starts_with("ru"),
    }
}

/// The system's language: Android has no `LANG`, the plugin asks the OS.
#[cfg(target_os = "android")]
fn system_language(app: &AppHandle) -> String {
    crate::platform_net::language(app)
}

#[cfg(not(target_os = "android"))]
fn system_language(_app: &AppHandle) -> String {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .find_map(|var| std::env::var(var).ok().filter(|value| !value.is_empty()))
        .unwrap_or_default()
}

/// Show notices: each goes to the window as a `notification` event (the bell
/// and a toast) and, when the user wants them, to the system as well.
/// `force` shows a system notice even when those are off (the test button).
pub fn deliver(app: &AppHandle, records: Vec<NoticeRecord>, force: bool) {
    if records.is_empty() {
        return;
    }
    // In a sync group the device whose window has focus shows it (all of
    // them when none has).
    // The sandbox shows its notices in the window only, never to the system.
    let system = !crate::commands::sandbox::is_active_app(app)
        && (force
            || crate::sync::show_system_here(app) && {
                let state = app.state::<AppState>();
                state.vault.lock().ok().is_none_or(|guard| {
                    guard
                        .as_ref()
                        .is_none_or(|vault| vault.doc.settings.notifications.system)
                })
            });
    for record in records {
        let _ = app.emit("notification", &record);
        if system {
            let _ = app
                .notification()
                .builder()
                .title(&record.title)
                .body(&record.body)
                .show();
        }
    }
}

/// Log notices shown by a job (so they are never repeated) and show the new ones.
pub fn record_and_deliver(app: &AppHandle, records: Vec<NoticeRecord>) {
    let fresh = {
        let state = app.state::<AppState>();
        let Ok(mut guard) = state.vault.lock() else {
            return;
        };
        let Some(vault) = guard.as_mut() else {
            return;
        };
        vault.log_notices(records).unwrap_or_default()
    };
    deliver(app, fresh, false);
}

// -- 0. Notices while the vault is locked ---------------------------------------

/// Take a fresh snapshot of the notices ahead (every minute, or now with
/// `force`) and keep it, encrypted.
pub fn snapshot_tick(app: &AppHandle, force: bool) {
    let state = app.state::<AppState>();
    // The sandbox's notices are never kept for the locked screen.
    if crate::commands::sandbox::is_active(&state) {
        return;
    }
    let due = state.notify.lock().is_ok_and(|notify| {
        force
            || notify
                .last_snapshot
                .is_none_or(|last| last.elapsed() >= Duration::from_secs(SNAPSHOT_REFRESH_SEC))
    });
    if !due {
        return;
    }
    let ru = russian(app);
    let zone = current_zone(app);
    let snapshot = {
        let Ok(guard) = state.vault.lock() else {
            return;
        };
        let Some(vault) = guard.as_ref() else {
            return;
        };
        let mut snapshot = vault.notification_snapshot(now_secs(), &zone, ru);
        snapshot.role_client = matches!(crate::sync::current_role(app), Some(SyncRole::Client));
        snapshot
    };
    if let Ok(mut notify) = state.notify.lock() {
        // The same notices as a minute ago: no keyring call, no encrypted write.
        let unchanged = notify.snapshot.as_ref().is_some_and(|kept| {
            trade_core::service::NotificationSnapshot {
                created_at: String::new(),
                ..kept.clone()
            } == trade_core::service::NotificationSnapshot {
                created_at: String::new(),
                ..snapshot.clone()
            }
        });
        notify.snapshot = Some(snapshot);
        notify.last_snapshot = Some(Instant::now());
        if force || !unchanged {
            crate::notify_store::save(app, &mut notify);
        }
    }
}

/// What the locked app told and fired, taken out of the memory.
struct LockedFindings {
    vault_id: String,
    shown: Vec<NoticeRecord>,
    fired: Vec<(String, String)>,
    bases: HashMap<String, AlertBase>,
}

/// Empty the locked memory into findings; `None` when nothing was told.
fn take_locked(notify: &mut crate::notify_store::NotifyMemory) -> Option<LockedFindings> {
    if notify.shown.is_empty() && notify.fired_alerts.is_empty() {
        return None;
    }
    Some(LockedFindings {
        vault_id: notify
            .snapshot
            .as_ref()
            .map(|snapshot| snapshot.vault_id.clone())
            .unwrap_or_default(),
        shown: std::mem::take(&mut notify.shown),
        fired: std::mem::take(&mut notify.fired_alerts),
        bases: std::mem::take(&mut notify.fired_bases),
    })
}

/// Put the findings into the vault. Only a device that runs the jobs does:
/// on a client the main device owns alerts and the log, and what a stale
/// locked snapshot saw would only fight what came by sync. The findings are
/// dropped there, not kept for later. Returns whether they were applied.
fn merge_findings(vault: &mut UnlockedVault, found: LockedFindings, runs_jobs: bool) -> bool {
    if !runs_jobs || vault.doc.id != found.vault_id {
        return false;
    }
    let _ = vault.apply_locked_deliveries_seen(found.shown, &found.fired, &found.bases);
    true
}

/// Merge what was shown while locked into the vault's log (same vault only).
fn merge_locked(app: &AppHandle) {
    let state = app.state::<AppState>();
    let Some(found) = state
        .notify
        .lock()
        .ok()
        .and_then(|mut notify| take_locked(&mut notify))
    else {
        return;
    };
    let runs_jobs = crate::sync::runs_jobs(app);
    let applied = {
        let Ok(mut guard) = state.vault.lock() else {
            return;
        };
        let Some(vault) = guard.as_mut() else {
            return;
        };
        merge_findings(vault, found, runs_jobs)
    };
    if let Ok(mut notify) = state.notify.lock() {
        crate::notify_store::save(app, &mut notify);
    }
    if applied {
        let _ = app.emit("reminders-changed", serde_json::json!({}));
    }
}

/// The vault is locked: remind from the snapshot, check alerts against fresh
/// prices every few minutes. Nothing here can read the vault.
fn locked_tick(app: &AppHandle) {
    let state = app.state::<AppState>();
    let ru = russian(app);
    let now = now_secs();
    let (fresh, system, alert_assets) = {
        let Ok(mut notify) = state.notify.lock() else {
            return;
        };
        let Some(snapshot) = notify.snapshot.clone() else {
            return;
        };
        let shown: HashSet<String> = notify
            .shown
            .iter()
            .map(|record| record.key.clone())
            .collect();
        let fresh = snapshot.due(now, &shown);
        notify.shown.extend(fresh.iter().cloned());
        let check_alerts = !snapshot.alerts.is_empty()
            && !snapshot.role_client
            && notify
                .last_alert_check
                .is_none_or(|last| last.elapsed() >= Duration::from_secs(ALERT_REFRESH_SEC));
        if check_alerts {
            notify.last_alert_check = Some(Instant::now());
        }
        let assets: Vec<_> = if check_alerts {
            snapshot
                .alerts
                .iter()
                .map(|alert| alert.asset.clone())
                .collect()
        } else {
            Vec::new()
        };
        if !fresh.is_empty() {
            crate::notify_store::save(app, &mut notify);
        }
        (fresh, snapshot.system, assets)
    };
    show_locked(app, fresh, system);
    if alert_assets.is_empty() {
        return;
    }
    let Ok(prices) = tauri::async_runtime::block_on(state.pricing.fetch_prices(&alert_assets))
    else {
        return;
    };
    let usd: HashMap<String, trade_core::Amount> = prices
        .into_iter()
        .map(|point| (point.asset_key, point.value_usd))
        .collect();
    let hits = {
        let Ok(mut notify) = state.notify.lock() else {
            return;
        };
        let Some(snapshot) = notify.snapshot.clone() else {
            return;
        };
        let fired: HashSet<String> = notify
            .fired_alerts
            .iter()
            .map(|(id, _)| id.clone())
            .collect();
        let hits = snapshot.alert_hits(&usd, &fired, ru);
        for (alert_id, record) in &hits {
            notify
                .fired_alerts
                .push((alert_id.clone(), record.at.clone()));
            if let Some(alert) = snapshot
                .alerts
                .iter()
                .find(|alert| &alert.alert_id == alert_id)
            {
                notify.fired_bases.insert(alert_id.clone(), alert.base());
            }
            notify.shown.push(record.clone());
        }
        if !hits.is_empty() {
            crate::notify_store::save(app, &mut notify);
        }
        hits
    };
    show_locked(
        app,
        hits.into_iter().map(|(_, record)| record).collect(),
        system,
    );
}

/// Show notices without the vault: the window gets them, the system too
/// unless the user turned system notices off.
fn show_locked(app: &AppHandle, records: Vec<NoticeRecord>, system: bool) {
    for record in records {
        let _ = app.emit("notification", &record);
        if system {
            let _ = app
                .notification()
                .builder()
                .title(&record.title)
                .body(&record.body)
                .show();
        }
    }
}

// -- 1. Bybit import -------------------------------------------------------------

/// Poll each exchange on its own interval (`ExchangeProfile.import_interval_sec`,
/// `0` disables it) and stage any new fills for the user to confirm. A vault
/// that keeps no crypto hides its exchanges, so they are not polled either.
fn bybit_tick(app: &AppHandle, memory: &mut Memory) {
    let due: Vec<(String, String, String)> = {
        let state = app.state::<AppState>();
        let Ok(guard) = state.vault.lock() else {
            return;
        };
        let Some(vault) = guard.as_ref().filter(|vault| vault.crypto_enabled()) else {
            return;
        };
        vault
            .import_schedule()
            .into_iter()
            .filter(|(store_id, interval, _, _)| {
                let interval = Duration::from_secs(u64::from(*interval));
                memory
                    .last_bybit
                    .get(store_id)
                    .is_none_or(|previous| previous.elapsed() >= interval)
            })
            .map(|(store_id, _, key, secret)| (store_id, key, secret))
            .collect()
    };
    if due.is_empty() {
        return;
    }
    for (store_id, _, _) in &due {
        memory.last_bybit.insert(store_id.clone(), Instant::now());
    }
    let Ok((staged, confirmed)) = tauri::async_runtime::block_on(poll_bybit_once(app, due)) else {
        return;
    };
    if confirmed > 0 {
        let _ = app.emit("automation-created", serde_json::json!({ "tx_ids": [] }));
    }
    if staged > 0 {
        let _ = app.emit(
            "bybit-import-pending",
            serde_json::json!({ "staged": staged }),
        );
        let ru = russian(app);
        record_and_deliver(
            app,
            vec![notice(
                NoticeCategory::BybitImport,
                format!("bybit:{}", now_secs()),
                notice_title(NoticeCategory::BybitImport, ru),
                if ru {
                    format!("Новых операций на подтверждение: {staged}")
                } else {
                    format!("New operations to confirm: {staged}")
                },
                Some(trade_core::routes::TRANSACTIONS.into()),
                trade_core::model::vault::now_iso(),
            )],
        );
    }
}

/// One polling pass over the given `(store_id, api_key, api_secret)` keys.
/// Returns how many operations were newly staged and how many were booked
/// at once. Each sync, failed ones included, lands in the wallet's history.
async fn poll_bybit_once(
    app: &AppHandle,
    keys: Vec<(String, String, String)>,
) -> Result<(u32, u32), String> {
    let state = app.state::<AppState>();
    let mut staged_total = 0u32;
    let mut confirmed_total = 0u32;
    for (store_id, api_key, api_secret) in keys {
        // Network calls happen with no guard held.
        let run = crate::commands::exchange::sync_bybit(
            &state,
            &store_id,
            &api_key,
            &api_secret,
            500,
            trade_core::model::vault::ImportSource::Auto,
        )
        .await?;
        staged_total += run.staged.saturating_sub(run.confirmed);
        confirmed_total += run.confirmed;
    }
    Ok((staged_total, confirmed_total))
}

// -- 2. Workflows and automations ---------------------------------------------------

/// What a pass did to the automations, worked out under the lock.
struct AutomationNews {
    /// Operations automations made.
    created: Vec<String>,
    /// `(run id, automation id, automation name, planned moment, reason)` of
    /// refused runs.
    rejected: Vec<(String, String, String, String, RejectReason)>,
    /// `(operation id, automation name, amount, asset)` booked approximately.
    estimated: Vec<(String, String, String, String)>,
}

/// Fire the workflows (automations among them) whose triggers are due and
/// show what they told. Automations keep their own events and notices.
fn workflows_tick(app: &AppHandle) {
    let zone = current_zone(app);
    let ru = russian(app);
    let (pass, news, interest) = {
        let state = app.state::<AppState>();
        let Ok(mut guard) = state.vault.lock() else {
            return;
        };
        let Some(vault) = guard.as_mut() else {
            return;
        };
        vault.set_notice_language(ru);
        let Ok(pass) = vault.run_due_workflows(now_secs(), &zone) else {
            return;
        };
        // Savings accounts that ask for it get last month's interest booked.
        let today = trade_core::calendar::Calendar::new(&zone).date_of(now_secs());
        let interest = vault.post_due_interest(today).unwrap_or(0);
        let names: std::collections::HashMap<String, String> = vault
            .automations()
            .into_iter()
            .map(|automation| (automation.id, automation.name))
            .collect();
        let mut news = AutomationNews {
            created: Vec::new(),
            rejected: Vec::new(),
            estimated: Vec::new(),
        };
        for run in pass
            .runs
            .iter()
            .filter(|run| names.contains_key(&run.workflow_id))
        {
            let name = names[&run.workflow_id].clone();
            match &run.outcome {
                RunOutcome::Rejected {
                    reason, planned_at, ..
                } => news.rejected.push((
                    run.id.clone(),
                    run.workflow_id.clone(),
                    name,
                    planned_at.clone().unwrap_or_else(|| run.started_at.clone()),
                    reason.clone(),
                )),
                _ => {
                    for id in &run.created {
                        news.created.push(id.clone());
                        let Some(tx) = vault.doc.transactions.iter().find(|tx| &tx.meta().id == id)
                        else {
                            continue;
                        };
                        if tx.meta().origin.is_estimated() {
                            let (amount, asset) = match tx {
                                Transaction::Topup(inner) => {
                                    (inner.amount, inner.asset.symbol.clone())
                                }
                                Transaction::Payment(inner) => {
                                    (inner.amount, inner.asset.symbol.clone())
                                }
                                Transaction::Transfer(inner) => {
                                    (inner.amount, inner.asset.symbol.clone())
                                }
                                _ => continue,
                            };
                            news.estimated.push((
                                id.clone(),
                                name.clone(),
                                amount.to_string(),
                                asset,
                            ));
                        }
                    }
                }
            }
        }
        (pass, news, interest)
    };
    if !pass.created.is_empty() || interest > 0 {
        let _ = app.emit(
            "automation-created",
            serde_json::json!({ "tx_ids": pass.created }),
        );
    }
    if !pass.runs.is_empty() {
        let _ = app.emit(
            "workflows-ran",
            serde_json::json!({ "runs": pass.runs.len(), "queued": pass.queued }),
        );
    }
    deliver(app, pass.notices, false);
    automation_notices(app, &news, pass.skipped);
}

fn automation_notices(app: &AppHandle, news: &AutomationNews, skipped: usize) {
    let ru = russian(app);
    if !news.created.is_empty() {
        record_and_deliver(
            app,
            vec![notice(
                NoticeCategory::Automation,
                format!("automation-created:{}", news.created[0]),
                notice_title(NoticeCategory::Automation, ru),
                if ru {
                    format!("Созданы автоматические операции: {}", news.created.len())
                } else {
                    format!("Automatic transactions created: {}", news.created.len())
                },
                Some(trade_core::routes::AUTOMATIONS.into()),
                trade_core::model::vault::now_iso(),
            )],
        );
    }
    for (tx_id, name, amount, asset) in &news.estimated {
        let (title, body) = if ru {
            (
                format!("«{name}»: уточните сумму"),
                format!("Записано примерно: ~{amount} {asset}. Внесите сумму, которая пришла."),
            )
        } else {
            (
                format!("“{name}”: put in the amount"),
                format!("Booked approximately: ~{amount} {asset}. Put in the amount that came."),
            )
        };
        let mut record = notice(
            NoticeCategory::Automation,
            format!("automation-estimated:{tx_id}"),
            &title,
            body,
            Some(trade_core::routes::AUTOMATIONS.into()),
            trade_core::model::vault::now_iso(),
        );
        record.title = title;
        record_and_deliver(app, vec![record]);
    }
    if skipped > 0 {
        let _ = app.emit(
            "automation-skipped",
            serde_json::json!({ "count": skipped }),
        );
    }
    for (run_id, automation_id, name, planned_at, reason) in &news.rejected {
        let event = AutomationRejectedEvent {
            run_id: run_id.clone(),
            automation_id: automation_id.clone(),
            name: name.clone(),
            planned_at: planned_at.clone(),
            reason: reason.clone(),
        };
        let _ = app.emit("automation-rejected", &event);
        let date = planned_at.get(..10).unwrap_or_default();
        let (title, body) = rejected_text(ru, name, date, reason);
        let mut record = notice(
            NoticeCategory::Automation,
            format!("automation-rejected:{run_id}"),
            &title,
            body,
            Some(trade_core::routes::AUTOMATIONS.into()),
            planned_at.clone(),
        );
        record.title = title;
        record_and_deliver(app, vec![record]);
    }
}

/// "Rent 2026-10-15 not created: the account has 8000 RUB of 25000 RUB".
fn rejected_text(ru: bool, name: &str, date: &str, reason: &RejectReason) -> (String, String) {
    let title = if ru {
        format!("«{name}» {date} не создана")
    } else {
        format!("“{name}” on {date} was not created")
    };
    let body = match reason {
        RejectReason::InsufficientFunds {
            asset,
            required,
            available,
            ..
        } if ru => format!(
            "Не хватает средств: {available} {sym} из {required} {sym}",
            sym = asset.symbol
        ),
        RejectReason::InsufficientFunds {
            asset,
            required,
            available,
            ..
        } => format!(
            "Not enough funds: {available} {sym} of {required} {sym}",
            sym = asset.symbol
        ),
        RejectReason::CreditLimitExceeded {
            limit, required, ..
        } if ru => format!("Превышен кредитный лимит: нужно {required}, лимит {limit}"),
        RejectReason::CreditLimitExceeded {
            limit, required, ..
        } => format!("Credit limit exceeded: {required} needed, limit {limit}"),
        RejectReason::Invalid { message } => message.clone(),
    };
    (title, body)
}

// -- 3. Price alerts -------------------------------------------------------------

fn alerts_tick(app: &AppHandle, memory: &mut Memory) {
    let state = app.state::<AppState>();
    let assets = {
        let Ok(guard) = state.vault.lock() else {
            return;
        };
        let Some(vault) = guard.as_ref() else {
            return;
        };
        vault.alert_assets()
    };
    if assets.is_empty() {
        return;
    }
    if memory
        .last_alert_refresh
        .is_some_and(|previous| previous.elapsed() < Duration::from_secs(ALERT_REFRESH_SEC))
    {
        return;
    }
    memory.last_alert_refresh = Some(Instant::now());
    let Ok(prices) = tauri::async_runtime::block_on(state.pricing.fetch_prices(&assets)) else {
        return;
    };
    let fired = {
        let Ok(mut guard) = state.vault.lock() else {
            return;
        };
        let Some(vault) = guard.as_mut() else {
            return;
        };
        vault.apply_prices(&prices);
        vault.restamp_missing();
        let _ = vault.persist();
        vault.check_alerts(now_secs()).unwrap_or_default()
    };
    let ru = russian(app);
    let records = fired
        .into_iter()
        .map(|alert| {
            let _ = app.emit("price-alert", serde_json::json!({ "alert_id": alert.id }));
            let word = match (alert.condition, ru) {
                (AlertCondition::Above, true) => "выше",
                (AlertCondition::Below, true) => "ниже",
                (AlertCondition::Above, false) => "above",
                (AlertCondition::Below, false) => "below",
            };
            notice(
                NoticeCategory::PriceAlert,
                format!(
                    "alert:{}:{}",
                    alert.id,
                    alert.triggered_at.clone().unwrap_or_default()
                ),
                notice_title(NoticeCategory::PriceAlert, ru),
                format!(
                    "{} {word} {} {}",
                    alert.asset.symbol,
                    alert.price,
                    alert.currency.to_uppercase()
                ),
                Some(trade_core::routes::AUTOMATIONS_ALERTS.into()),
                trade_core::model::vault::now_iso(),
            )
        })
        .collect();
    record_and_deliver(app, records);
}

// -- 4. Reminders ----------------------------------------------------------------

fn reminders_tick(app: &AppHandle, _memory: &mut Memory) {
    let state = app.state::<AppState>();
    let ru = russian(app);
    let zone = current_zone(app);
    let (fresh, synced) = {
        let Ok(mut guard) = state.vault.lock() else {
            return;
        };
        let Some(vault) = guard.as_mut() else {
            return;
        };
        // Automatic reminders follow the data: a new loan, a moved payment day.
        let synced = vault
            .sync_auto_reminders(now_secs(), &zone, ru)
            .unwrap_or(false);
        // Reminders whose time came, money that will run short, budgets.
        let fresh = vault.due_notices(now_secs(), &zone, ru).unwrap_or_default();
        (fresh, synced)
    };
    if synced {
        let _ = app.emit("reminders-changed", serde_json::json!({}));
    }
    if fresh
        .iter()
        .any(|record| record.key.starts_with("reminder:"))
    {
        let _ = app.emit("reminder-due", serde_json::json!({}));
    }
    deliver(app, fresh, false);
}

#[cfg(test)]
mod tests {
    use super::pick;

    #[test]
    fn prices_refresh_when_visible_or_a_main_serves_a_focused_device() {
        use super::should_refresh_prices;
        use trade_core::ipc_contract::SyncRole::{Client, Main, Searching};
        // (visible, role, any_link_focused, expected)
        let table = [
            (true, Main, false, true),
            (true, Client, false, true),
            (true, Searching, true, true),
            (false, Main, true, true),
            (false, Main, false, false),
            (false, Client, true, false),
            (false, Searching, true, false),
            (false, Client, false, false),
        ];
        for (visible, role, focused, expected) in table {
            assert_eq!(
                should_refresh_prices(visible, role, focused),
                expected,
                "visible={visible} role={role:?} focused={focused}"
            );
        }
    }

    #[test]
    fn pick_follows_the_interface_language() {
        assert_eq!(pick(true, "Блокировка", "Lock"), "Блокировка");
        assert_eq!(pick(false, "Блокировка", "Lock"), "Lock");
    }

    use trade_core::model::planning::{AlertCondition, PriceAlert};
    use trade_core::model::store::AssetRef;
    use trade_core::service::UnlockedVault;

    use super::{merge_findings, take_locked};
    use crate::notify_store::NotifyMemory;

    fn alert_vault() -> (tempfile::TempDir, UnlockedVault) {
        let dir = tempfile::tempdir().unwrap();
        let mut vault = UnlockedVault::create(dir.path().join("a.wallet"), "pw").unwrap();
        vault.doc.price_alerts.push(PriceAlert {
            id: "alert".into(),
            asset: AssetRef::default(),
            condition: AlertCondition::Above,
            price: trade_core::Amount::from(60000u32),
            currency: "usd".into(),
            active: true,
            triggered_at: None,
            store_id: None,
            repeats: false,
        });
        (dir, vault)
    }

    fn told(vault_id: &str) -> NotifyMemory {
        let mut notify = NotifyMemory {
            snapshot: Some(trade_core::service::NotificationSnapshot {
                vault_id: vault_id.into(),
                ..Default::default()
            }),
            ..NotifyMemory::default()
        };
        notify
            .fired_alerts
            .push(("alert".into(), "2026-03-01T10:00:00Z".into()));
        notify
    }

    #[test]
    fn a_client_does_not_merge_a_locked_snapshot() {
        let (_dir, mut vault) = alert_vault();
        let mut notify = told(&vault.doc.id.clone());
        let found = take_locked(&mut notify).expect("something was fired");
        assert!(
            notify.fired_alerts.is_empty(),
            "the memory keeps it forever"
        );
        assert!(!merge_findings(&mut vault, found, false));
        assert!(
            vault.doc.price_alerts[0].triggered_at.is_none(),
            "a client switched an alert off from a locked snapshot"
        );

        // A device that runs the jobs takes it in.
        let mut notify = told(&vault.doc.id.clone());
        let found = take_locked(&mut notify).unwrap();
        assert!(merge_findings(&mut vault, found, true));
        assert!(vault.doc.price_alerts[0].triggered_at.is_some());
    }
}
