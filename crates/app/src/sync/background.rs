//! Android: sync goes on while the app is in the background. `plugin-net`
//! starts a foreground service (notification, Wi-Fi lock) when the app
//! leaves the screen while sync runs; this side polls it for the
//! notification's buttons and locks the vault after
//! `SyncPrefs::background_lock_min` minutes in the background without a sync.

use std::sync::{LazyLock, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use tauri::AppHandle;
use tauri_plugin_net::Background;
use trade_core::ipc_contract::SyncPhase;

use crate::platform_net;

const POLL_EVERY: Duration = Duration::from_secs(2);

#[derive(Default)]
struct State {
    /// Sync runs.
    armed: bool,
    /// "Stop" was pressed: sync starts again when the app comes back.
    paused: bool,
    polling: bool,
    /// In the background since.
    since: Option<Instant>,
    shown: Background,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(Mutex::default);

fn state() -> MutexGuard<'static, State> {
    match STATE.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// Sync started (`true`) or stopped here.
pub fn arm(app: &AppHandle, on: bool) {
    let start_polling = {
        let mut state = state();
        state.armed = on;
        if on {
            state.paused = false;
        }
        let start = (state.armed || state.paused) && !state.polling;
        state.polling |= start;
        start
    };
    show(app, 0);
    if start_polling {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            loop {
                tokio::time::sleep(POLL_EVERY).await;
                if !tick(&app) {
                    break;
                }
            }
        });
    }
}

/// The notification's texts, in the app's language.
fn texts(app: &AppHandle, armed: bool, linked: usize) -> Background {
    let ru = crate::scheduler::russian(app);
    let pick = |ru_text: &str, en_text: &str| crate::scheduler::pick(ru, ru_text, en_text);
    let text = if linked == 0 {
        pick(
            "Ищу устройства с этим vault",
            "Looking for devices with this vault",
        )
    } else {
        format!(
            "{}{linked}",
            pick("Связано устройств: ", "Devices linked: ")
        )
    };
    Background {
        armed,
        channel: pick("Синхронизация", "Sync"),
        title: pick("Синхронизация vault в фоне", "Vault sync in the background"),
        text,
        lock: pick("Заблокировать", "Lock"),
        stop: pick("Остановить", "Stop"),
    }
}

/// Send the notification's state when it changed.
fn show(app: &AppHandle, linked: usize) {
    let armed = state().armed;
    let next = texts(app, armed, linked);
    {
        let mut state = state();
        if state.shown == next {
            return;
        }
        state.shown = next.clone();
    }
    platform_net::background(app, &next);
}

/// One poll; `false` = stop polling.
fn tick(app: &AppHandle) -> bool {
    let Some(poll) = platform_net::poll(app) else {
        return true;
    };
    match poll.action.as_deref() {
        Some("lock") => {
            crate::commands::auth::lock_now(app);
        }
        Some("stop") => {
            state().paused = true;
            super::stop(app);
        }
        _ => {}
    }
    let now = Instant::now();
    let (armed, resume, since) = {
        let mut state = state();
        if poll.background {
            let since = *state.since.get_or_insert(now);
            (state.armed, false, Some(since))
        } else {
            state.since = None;
            let resume = std::mem::take(&mut state.paused);
            (state.armed, resume, None)
        }
    };
    if resume {
        super::restart(app);
    }
    if armed {
        let (linked, last_activity) = {
            let hub = super::hub(app);
            let linked = hub
                .links
                .values()
                .filter(|link| link.view.phase == SyncPhase::Live)
                .count();
            (linked, hub.last_activity)
        };
        show(app, linked);
        let minutes = crate::commands::app_settings::current_settings(app)
            .sync
            .background_lock_min();
        if let Some(since) = since
            && minutes > 0
        {
            let idle_from = last_activity.map_or(since, |at| at.max(since));
            if now.duration_since(idle_from) >= Duration::from_secs(u64::from(minutes) * 60) {
                crate::commands::auth::lock_now(app);
            }
        }
    }
    let mut state = state();
    let more = state.armed || state.paused;
    state.polling = more;
    more
}
