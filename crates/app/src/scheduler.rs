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
use trade_core::ipc_contract::AutomationRejectedEvent;
use trade_core::model::notify::{NoticeCategory, NoticeRecord};
use trade_core::model::planning::{AlertCondition, AutomationOutcome, RejectReason};
use trade_core::service::{notice, notice_title};

use crate::commands::app_settings::current_zone;
use crate::commands::{AppState, err_to_string};

/// How often the scheduler wakes up.
const SCHEDULER_TICK_SEC: u64 = 30;
/// Least time between two price refreshes for alerts.
const ALERT_REFRESH_SEC: u64 = 5 * 60;

/// What the scheduler remembers between ticks.
#[derive(Default)]
struct Memory {
    /// Last import poll per exchange store.
    last_bybit: HashMap<String, Instant>,
    last_alert_refresh: Option<Instant>,
}

/// How often the snapshot for a locked vault is taken while it is open.
const SNAPSHOT_REFRESH_SEC: u64 = 60;

pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || {
        let mut memory = Memory::default();
        loop {
            std::thread::sleep(Duration::from_secs(SCHEDULER_TICK_SEC));
            if !is_unlocked(&app) {
                locked_tick(&app);
                continue;
            }
            // First take in what was told while locked, so nothing repeats.
            merge_locked(&app);
            bybit_tick(&app, &mut memory);
            automations_tick(&app);
            alerts_tick(&app, &mut memory);
            reminders_tick(&app, &mut memory);
            snapshot_tick(&app, false);
        }
    });
}

fn is_unlocked(app: &AppHandle) -> bool {
    let state = app.state::<AppState>();
    state.vault.lock().is_ok_and(|guard| guard.is_some())
}

/// Whether texts shown outside the window should be in Russian.
pub fn russian(app: &AppHandle) -> bool {
    let Ok(dir) = app.path().app_config_dir() else {
        return false;
    };
    match load_settings(&dir).locale {
        Locale::Ru => true,
        Locale::En => false,
        Locale::Auto => std::env::var("LANG").is_ok_and(|lang| lang.starts_with("ru")),
    }
}

/// Show notices: each goes to the window as a `notification` event (the bell
/// and a toast) and, when the user wants them, to the system as well.
/// `force` shows a system notice even when those are off (the test button).
pub fn deliver(app: &AppHandle, records: Vec<NoticeRecord>, force: bool) {
    if records.is_empty() {
        return;
    }
    let system = force || {
        let state = app.state::<AppState>();
        state.vault.lock().ok().is_none_or(|guard| {
            guard
                .as_ref()
                .is_none_or(|vault| vault.doc.settings.notifications.system)
        })
    };
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
fn record_and_deliver(app: &AppHandle, records: Vec<NoticeRecord>) {
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
        vault.notification_snapshot(now_secs(), &zone, ru)
    };
    if let Ok(mut notify) = state.notify.lock() {
        notify.snapshot = Some(snapshot);
        notify.last_snapshot = Some(Instant::now());
        crate::notify_store::save(app, &mut notify);
    }
}

/// Merge what was shown while locked into the vault's log (same vault only).
fn merge_locked(app: &AppHandle) {
    let state = app.state::<AppState>();
    let (vault_id, shown, fired) = {
        let Ok(mut notify) = state.notify.lock() else {
            return;
        };
        if notify.shown.is_empty() && notify.fired_alerts.is_empty() {
            return;
        }
        (
            notify
                .snapshot
                .as_ref()
                .map(|snapshot| snapshot.vault_id.clone())
                .unwrap_or_default(),
            std::mem::take(&mut notify.shown),
            std::mem::take(&mut notify.fired_alerts),
        )
    };
    {
        let Ok(mut guard) = state.vault.lock() else {
            return;
        };
        let Some(vault) = guard.as_mut() else {
            return;
        };
        if vault.doc.id == vault_id {
            let _ = vault.apply_locked_deliveries(shown, &fired);
        }
    }
    if let Ok(mut notify) = state.notify.lock() {
        crate::notify_store::save(app, &mut notify);
    }
    let _ = app.emit("reminders-changed", serde_json::json!({}));
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
/// `0` disables it) and stage any new fills for the user to confirm.
fn bybit_tick(app: &AppHandle, memory: &mut Memory) {
    let due: Vec<(String, String, String)> = {
        let state = app.state::<AppState>();
        let Ok(guard) = state.vault.lock() else {
            return;
        };
        let Some(vault) = guard.as_ref() else {
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
    let Ok(staged) = tauri::async_runtime::block_on(poll_bybit_once(app, due)) else {
        return;
    };
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
                Some("transactions".into()),
                trade_core::model::vault::now_iso(),
            )],
        );
    }
}

/// One polling pass over the given `(store_id, api_key, api_secret)` keys.
/// Returns how many fills were newly staged.
async fn poll_bybit_once(
    app: &AppHandle,
    keys: Vec<(String, String, String)>,
) -> Result<u32, String> {
    let state = app.state::<AppState>();
    let client = trade_core::exchange::BybitClient::default();
    let mut staged_total = 0u32;

    for (store_id, api_key, api_secret) in keys {
        // Network call happens with no guard held.
        let execs = client
            .all_executions(&api_key, &api_secret, 500)
            .await
            .map_err(err_to_string)?;

        // Same sweep as the manual import: moves between accounts and Earn
        // payouts too, each optional for a key without those permissions.
        let transfers = client
            .inter_transfers(&api_key, &api_secret, 500)
            .await
            .unwrap_or_default();
        let yields = client
            .earn_yields(&api_key, &api_secret, 500)
            .await
            .unwrap_or_default();
        let imports = trade_core::model::vault::PendingImport::bybit_batch(
            &store_id, &execs, &transfers, &yields,
        );

        let mut guard = state.vault.lock().map_err(|err| err.to_string())?;
        let Some(vault) = guard.as_mut() else {
            return Err("REQUIRES_UNLOCK: vault was locked mid-poll".into());
        };
        staged_total += vault.stage_imports(imports).map_err(err_to_string)?;
    }

    Ok(staged_total)
}

// -- 2. Automations --------------------------------------------------------------

fn automations_tick(app: &AppHandle) {
    let zone = current_zone(app);
    let state = app.state::<AppState>();
    let (pass, names) = {
        let Ok(mut guard) = state.vault.lock() else {
            return;
        };
        let Some(vault) = guard.as_mut() else {
            return;
        };
        let Ok(pass) = vault.run_due_automations(now_secs(), &zone) else {
            return;
        };
        // Savings accounts that ask for it get last month's interest booked.
        let today = trade_core::calendar::Calendar::new(&zone).date_of(now_secs());
        if let Ok(posted) = vault.post_due_interest(today)
            && posted > 0
        {
            let _ = app.emit("automation-created", serde_json::json!({ "tx_ids": [] }));
        }
        let names: std::collections::HashMap<String, String> = vault
            .doc
            .automations
            .iter()
            .map(|automation| (automation.id.clone(), automation.name.clone()))
            .collect();
        (pass, names)
    };
    let ru = russian(app);
    if !pass.created.is_empty() {
        let _ = app.emit(
            "automation-created",
            serde_json::json!({ "tx_ids": pass.created }),
        );
        record_and_deliver(
            app,
            vec![notice(
                NoticeCategory::Automation,
                format!("automation-created:{}", pass.created[0]),
                notice_title(NoticeCategory::Automation, ru),
                if ru {
                    format!("Созданы автоматические операции: {}", pass.created.len())
                } else {
                    format!("Automatic transactions created: {}", pass.created.len())
                },
                Some("planning-automations".into()),
                trade_core::model::vault::now_iso(),
            )],
        );
    }
    if pass.skipped > 0 {
        let _ = app.emit(
            "automation-skipped",
            serde_json::json!({ "count": pass.skipped }),
        );
    }
    for run in pass.rejected {
        let AutomationOutcome::Rejected { reason } = &run.outcome else {
            continue;
        };
        let name = names.get(&run.automation_id).cloned().unwrap_or_default();
        let event = AutomationRejectedEvent {
            run_id: run.id.clone(),
            automation_id: run.automation_id.clone(),
            name: name.clone(),
            planned_at: run.planned_at.clone(),
            reason: reason.clone(),
        };
        let _ = app.emit("automation-rejected", &event);
        let date = run.planned_at.get(..10).unwrap_or_default();
        let (title, body) = rejected_text(ru, &name, date, reason);
        let mut record = notice(
            NoticeCategory::Automation,
            format!("automation-rejected:{}", run.id),
            &title,
            body,
            Some("planning-automations".into()),
            run.planned_at.clone(),
        );
        record.title = title;
        record_and_deliver(app, vec![record]);
    }
}

/// «Аренда 2026-10-15 не создана: на счёте 8000 RUB из 25000 RUB».
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
                Some("planning-alerts".into()),
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
