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
use trade_core::model::planning::{AlertCondition, AutomationOutcome, RejectReason};

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
    announced_reminders: HashSet<String>,
}

pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || {
        let mut memory = Memory::default();
        loop {
            std::thread::sleep(Duration::from_secs(SCHEDULER_TICK_SEC));
            if !is_unlocked(&app) {
                memory.announced_reminders.clear();
                continue;
            }
            bybit_tick(&app, &mut memory);
            automations_tick(&app);
            alerts_tick(&app, &mut memory);
            reminders_tick(&app, &mut memory);
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

fn notify(app: &AppHandle, title: &str, body: &str) {
    let _ = app.notification().builder().title(title).body(body).show();
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
        let title = if ru {
            "Созданы автоматические операции"
        } else {
            "Automatic transactions created"
        };
        notify(app, title, &pass.created.len().to_string());
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
        notify(app, &title, &body);
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
    for alert in fired {
        let _ = app.emit("price-alert", serde_json::json!({ "alert_id": alert.id }));
        let word = match (alert.condition, ru) {
            (AlertCondition::Above, true) => "выше",
            (AlertCondition::Below, true) => "ниже",
            (AlertCondition::Above, false) => "above",
            (AlertCondition::Below, false) => "below",
        };
        let body = format!(
            "{} {word} {} {}",
            alert.asset.symbol,
            alert.price,
            alert.currency.to_uppercase()
        );
        notify(
            app,
            if ru {
                "Ценовой алерт"
            } else {
                "Price alert"
            },
            &body,
        );
    }
}

// -- 4. Reminders ----------------------------------------------------------------

fn reminders_tick(app: &AppHandle, memory: &mut Memory) {
    let state = app.state::<AppState>();
    let ru = russian(app);
    let zone = current_zone(app);
    let (due, synced) = {
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
        (vault.due_reminders(now_secs()), synced)
    };
    if synced {
        let _ = app.emit("reminders-changed", serde_json::json!({}));
    }
    for reminder in due {
        if !memory.announced_reminders.insert(reminder.id.clone()) {
            continue;
        }
        let _ = app.emit("reminder-due", serde_json::json!({ "id": reminder.id }));
        notify(
            app,
            if ru {
                "Напоминание"
            } else {
                "Reminder"
            },
            &reminder.text,
        );
    }
}
