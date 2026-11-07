//! Android: sync goes on while the app is in the background. `plugin-net`
//! starts a foreground service (notification, Wi-Fi lock) when the app
//! leaves the screen while sync runs; this side polls it for the
//! notification's buttons and decides when the vault locks in the background:
//! `lock_policy::background_limit` combines `SyncPrefs::background_lock_min`
//! with the autolock (`crate::autolock`). The poll also runs without a sync
//! while the autolock is on, because it is what sees the app leave the screen.

use std::sync::{LazyLock, Mutex, MutexGuard};
use std::time::Duration;

use tauri::AppHandle;
use tauri_plugin_net::Background;
use trade_core::ipc_contract::SyncPhase;

use super::lock_policy::{self, Since};
use crate::platform_net;

const POLL_EVERY: Duration = Duration::from_secs(2);

/// "Lock at once" waits for the background state to hold this long: a window
/// of our own closing gives the foreground back a moment after its guard drops.
const SETTLE: Duration = Duration::from_secs(2);

#[derive(Default)]
struct State {
    /// Sync runs.
    armed: bool,
    /// "Stop" was pressed: sync starts again when the app comes back.
    paused: bool,
    polling: bool,
    /// In the background since.
    since: Option<Since>,
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
        spawn_poller(app.clone());
    }
}

/// Poll even with no sync running (the autolock needs the background state).
pub fn ensure_polling(app: &AppHandle) {
    let start = {
        let mut state = state();
        !std::mem::replace(&mut state.polling, true)
    };
    if start {
        spawn_poller(app.clone());
    }
}

fn spawn_poller(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(POLL_EVERY).await;
            if !tick(&app) {
                break;
            }
        }
    });
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
        // Keeps the CPU awake only while a link is Live.
        wake: armed && linked > 0,
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
            crate::autolock::lock(app, crate::autolock::LockReason::Background);
        }
        Some("stop") => {
            state().paused = true;
            super::stop(app);
        }
        _ => {}
    }
    // A window the app opened itself (a file picker, the biometric prompt)
    // takes it to the background without the user leaving: not a reason to lock.
    // Back in the foreground after a picker the webview opened: its hold ends.
    crate::autolock::note_picker_poll(poll.background);
    let leaves = crate::autolock::counts_as_background(poll.background);
    crate::autolock::set_in_background(leaves);
    let (armed, paused, resume, since) = {
        let mut state = state();
        if poll.background {
            let since = if leaves {
                Some(*state.since.get_or_insert_with(Since::now))
            } else {
                state.since = None;
                None
            };
            (state.armed, state.paused, false, since)
        } else {
            state.since = None;
            let resume = std::mem::take(&mut state.paused);
            (state.armed, false, resume, None)
        }
    };
    if resume {
        super::restart(app);
    }
    let mut last_activity = None;
    if armed {
        let linked = {
            let hub = super::hub(app);
            last_activity = hub.last_activity;
            hub.links
                .values()
                .filter(|link| link.view.phase == SyncPhase::Live)
                .count()
        };
        show(app, linked);
        // Here and not in `status`: see `platform_net::refresh_notifications`.
        if platform_net::refresh_notifications(app) {
            super::emit_status(app);
        }
    }
    // After "Stop" (`paused`) sync is off but the vault still locks.
    let settings = crate::commands::app_settings::current_settings(app);
    let autolock = settings.autolock();
    if let Some(since) = since {
        let limit = lock_policy::background_limit(lock_policy::BackgroundInput {
            sync_armed: armed,
            sync_paused: paused,
            sync_minutes: settings.sync.background_lock_min(),
            autolock_minutes: autolock.enabled.then_some(autolock.minutes),
            has_device_keys: crate::autolock::has_device_keys(app),
        });
        let idle_for = last_activity.map(|at| at.elapsed());
        let background_for = since.elapsed();
        if background_for >= SETTLE
            && lock_policy::background_lock_due(limit, background_for, idle_for)
            && crate::autolock::vault_open(app)
        {
            crate::autolock::lock(app, crate::autolock::LockReason::Background);
            // Nothing left to resume: unlocking starts sync, and polling ends.
            state().paused = false;
        }
    }
    let mut state = state();
    // Without a sync the poll goes on only for the autolock.
    let more = state.armed || state.paused || autolock.enabled;
    state.polling = more;
    more
}
