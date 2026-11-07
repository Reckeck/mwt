//! Autolock: the vault closes after idle time, on sleep and on the screen lock.
//!
//! One decision core for every platform: [`Watch`] reads a [`Clock`] and says
//! why the vault should lock, so it is tested on fake clocks. Around it:
//!
//! - idle: the window reports activity through `autolock_activity` (at most once
//!   per 30 s), the timer is counted here, not in the UI;
//! - sleep: the OS event where there is one (`autolock/linux.rs`, `windows.rs`),
//!   and on desktop a gap between two ticks as the fallback (a machine that
//!   slept does not tick), so sleep locks even where no event reaches us;
//! - screen lock: the OS event (same files);
//! - Android: the app going to the background, decided in `sync/background.rs`
//!   with `sync/lock_policy.rs`; here only the shared pieces (the "own window"
//!   guard, the time-limited picker hold, the foreground flag).
//!
//! Locking flushes first (`commands::auth::lock_now`), drops the open vault and
//! deletes the decrypted attachments opened for other programs.
// Each platform uses a different part (OS events on desktop, the background pieces on
// Android, `OwnWindow` once the pickers hold it), and the tests cover all of them.
#![allow(dead_code)]

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{LazyLock, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime};

use tauri::AppHandle;
use trade_core::ipc_contract::security::AutolockSettings;

use crate::commands::{AppState, app_settings};
use tauri::Manager;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

/// How often the idle and sleep checks run.
const TICK: Duration = Duration::from_secs(5);
/// Two ticks further apart than this mean the machine slept in between.
const SLEEP_GAP: Duration = Duration::from_secs(60);

/// Why the vault locks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockReason {
    Idle,
    Sleep,
    ScreenLock,
    Background,
}

/// A reading of both clocks. The monotonic one can stop in sleep and the wall
/// clock can jump, so idle and gaps use the larger delta of the two.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reading {
    pub mono: Duration,
    pub wall: Duration,
}

impl Reading {
    /// Time from `earlier` to `self`: the larger of the two clocks; a clock that
    /// went backwards counts as zero.
    pub fn since(&self, earlier: &Reading) -> Duration {
        self.mono
            .saturating_sub(earlier.mono)
            .max(self.wall.saturating_sub(earlier.wall))
    }
}

pub trait Clock {
    fn read(&self) -> Reading;
}

/// The real clocks.
pub struct SystemClock;

static START: LazyLock<Instant> = LazyLock::new(Instant::now);

impl Clock for SystemClock {
    fn read(&self) -> Reading {
        Reading {
            mono: Instant::now().saturating_duration_since(*START),
            wall: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default(),
        }
    }
}

/// What one tick of [`Watch`] reads besides the clock.
pub struct TickInput<'a> {
    /// A vault is open.
    pub vault_open: bool,
    pub settings: &'a AutolockSettings,
    /// Android: the app is in the background; `background.rs` decides then.
    pub in_background: bool,
    /// Treat a long gap between ticks as a sleep (desktop).
    pub detect_sleep: bool,
}

/// The idle timer and the sleep detector.
#[derive(Default)]
pub struct Watch {
    last_activity: Option<Reading>,
    last_tick: Option<Reading>,
    was_open: bool,
}

impl Watch {
    /// The user did something.
    pub fn touch(&mut self, clock: &impl Clock) {
        self.last_activity = Some(clock.read());
    }

    /// Idle time as of now; zero before any activity.
    pub fn idle_for(&self, clock: &impl Clock) -> Duration {
        self.last_activity
            .map_or(Duration::ZERO, |at| clock.read().since(&at))
    }

    /// One look. `Some` = lock now.
    pub fn tick(&mut self, clock: &impl Clock, input: &TickInput) -> Option<LockReason> {
        let now = clock.read();
        let previous_tick = self.last_tick.replace(now);
        if !input.vault_open {
            self.was_open = false;
            return None;
        }
        if !self.was_open {
            // A vault just opened: the idle time starts here.
            self.was_open = true;
            self.last_activity = Some(now);
            return None;
        }
        if input.detect_sleep
            && let Some(previous) = previous_tick
            && now.since(&previous) > SLEEP_GAP
        {
            return Some(LockReason::Sleep);
        }
        if input.in_background || !input.settings.enabled {
            return None;
        }
        let limit = Duration::from_secs(u64::from(input.settings.minutes) * 60);
        let idle = self
            .last_activity
            .map_or(Duration::ZERO, |at| now.since(&at));
        (idle >= limit).then_some(LockReason::Idle)
    }

    /// Back in the foreground: the user is here, the idle time starts over.
    pub fn resumed(&mut self, clock: &impl Clock) {
        self.touch(clock);
    }
}

static WATCH: LazyLock<Mutex<Watch>> = LazyLock::new(Mutex::default);

fn watch() -> MutexGuard<'static, Watch> {
    match WATCH.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// `autolock_activity`: the user is active.
pub fn touch() {
    watch().touch(&SystemClock);
}

/// Android: the app came back to the foreground.
pub fn resumed() {
    watch().resumed(&SystemClock);
}

/// Whether this device holds exchange keys (they only live on the device).
/// Stub until H2 keeps them: no keys, so the autolock can be turned off.
pub fn has_device_keys(_app: &AppHandle) -> bool {
    false
}

/// Windows the app opened itself (the file picker, the biometric prompt) take
/// it to the background without the user leaving. While one is open the app
/// does not count as being in the background, or the backup step of the wizard
/// would lock the vault from under itself.
static OWN_WINDOWS: AtomicU32 = AtomicU32::new(0);

/// Held while the app's own window is open.
pub struct OwnWindow(());

impl OwnWindow {
    pub fn open() -> Self {
        OWN_WINDOWS.fetch_add(1, Ordering::SeqCst);
        Self(())
    }
}

impl Drop for OwnWindow {
    fn drop(&mut self) {
        let _ = OWN_WINDOWS.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |open| {
            Some(open.saturating_sub(1))
        });
    }
}

/// How long a system picker opened by the webview itself (a plain file input,
/// the camera) may keep the app from counting as backgrounded. A security
/// bound: a picker left open forever must not keep the vault open.
const PICKER_HOLD: Duration = Duration::from_secs(5 * 60);

/// Deadline of the picker hold; `None` = no hold. Unlike [`OwnWindow`] there is
/// no Rust code around such a picker to drop a guard, so it is time-limited and
/// cleared when the app is seen in the foreground again.
static PICKER_HOLD_UNTIL: Mutex<Option<PickerHold>> = Mutex::new(None);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PickerHold {
    until: Instant,
    /// The app has been seen in the background since the hold was set. The
    /// picker opens a moment after the call, so a foreground poll before that
    /// must not end the hold; only coming back from the background does.
    left: bool,
}

fn picker_hold() -> MutexGuard<'static, Option<PickerHold>> {
    match PICKER_HOLD_UNTIL.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// A hold protects only while its deadline is in the future.
fn hold_protects(until: Option<Instant>, now: Instant) -> bool {
    until.is_some_and(|deadline| now < deadline)
}

/// `autolock_hold_for_picker`: a webview file input or camera is about to open.
pub fn hold_for_system_picker() {
    *picker_hold() = Some(PickerHold {
        until: Instant::now() + PICKER_HOLD,
        left: false,
    });
}

/// Every background poll: the hold ends when the app comes back to the
/// foreground after the picker took it away.
pub fn note_picker_poll(background: bool) {
    let mut hold = picker_hold();
    *hold = next_picker_hold(*hold, background);
}

fn next_picker_hold(hold: Option<PickerHold>, background: bool) -> Option<PickerHold> {
    let hold = hold?;
    match (background, hold.left) {
        (true, _) => Some(PickerHold { left: true, ..hold }),
        (false, true) => None,
        (false, false) => Some(hold),
    }
}

#[cfg(test)]
fn clear_picker_hold() {
    *picker_hold() = None;
}

/// Is the app in the background for the purposes of locking? The platform says
/// it is, and no window of its own (a guard or an unexpired picker hold)
/// explains that.
pub fn counts_as_background(platform_background: bool) -> bool {
    counts_as_background_at(
        platform_background,
        OWN_WINDOWS.load(Ordering::SeqCst),
        picker_hold().map(|hold| hold.until),
        Instant::now(),
    )
}

fn counts_as_background_at(
    platform_background: bool,
    own_windows: u32,
    hold_until: Option<Instant>,
    now: Instant,
) -> bool {
    platform_background && own_windows == 0 && !hold_protects(hold_until, now)
}

static IN_BACKGROUND: AtomicBool = AtomicBool::new(false);

/// Android: `background.rs` reports the state it polled. A return to the
/// foreground starts the idle time over.
pub fn set_in_background(background: bool) {
    if IN_BACKGROUND.swap(background, Ordering::SeqCst) && !background {
        resumed();
    }
}

/// An OS event as the logind / screensaver signals describe it: interface,
/// member and the first boolean argument, if any.
pub fn classify_signal(interface: &str, member: &str, flag: Option<bool>) -> Option<LockReason> {
    match (interface, member, flag) {
        ("org.freedesktop.login1.Manager", "PrepareForSleep", Some(true)) => {
            Some(LockReason::Sleep)
        }
        ("org.freedesktop.login1.Session", "Lock", _) => Some(LockReason::ScreenLock),
        ("org.freedesktop.ScreenSaver" | "org.gnome.ScreenSaver", "ActiveChanged", Some(true)) => {
            Some(LockReason::ScreenLock)
        }
        _ => None,
    }
}

/// A vault is open and has a password: one without has nothing to lock with
/// (the shell hides `Lock` for it too).
pub fn vault_open(app: &AppHandle) -> bool {
    app.state::<AppState>()
        .vault
        .lock()
        .map(|guard| guard.as_ref().is_some_and(|vault| vault.is_encrypted()))
        .unwrap_or(false)
}

/// Close the vault and delete what was decrypted for other programs.
pub fn lock(app: &AppHandle, _reason: LockReason) {
    crate::commands::auth::lock_now(app);
    crate::commands::attachments::wipe_opened(app);
}

/// Start the idle and sleep watch and, on desktop, the OS event listeners.
pub fn spawn(app: AppHandle) {
    // Android: the foreground state is polled by `sync/background.rs`, which
    // also runs the background rule; make sure it polls without a sync.
    #[cfg(target_os = "android")]
    crate::sync::background::ensure_polling(&app);
    #[cfg(target_os = "linux")]
    linux::listen(app.clone());
    #[cfg(windows)]
    windows::listen(app.clone());
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(TICK).await;
            let settings = app_settings::current_settings(&app).autolock();
            let open = vault_open(&app);
            let reason = watch().tick(
                &SystemClock,
                &TickInput {
                    vault_open: open,
                    settings: &settings,
                    in_background: IN_BACKGROUND.load(Ordering::SeqCst),
                    detect_sleep: cfg!(desktop),
                },
            );
            if let Some(reason) = reason {
                lock(&app, reason);
            }
        }
    });
}

/// The OS event arrived: lock when a vault is open.
pub fn on_os_event(app: &AppHandle, reason: LockReason) {
    if vault_open(app) {
        lock(app, reason);
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    struct FakeClock(Cell<Reading>);

    impl FakeClock {
        fn new() -> Self {
            Self(Cell::new(Reading {
                mono: Duration::from_secs(1000),
                wall: Duration::from_secs(1_700_000_000),
            }))
        }

        /// Time passes on both clocks.
        fn advance(&self, by: Duration) {
            let now = self.0.get();
            self.0.set(Reading {
                mono: now.mono + by,
                wall: now.wall + by,
            });
        }

        /// A sleep: the monotonic clock stops, the wall clock goes on.
        fn sleep(&self, by: Duration) {
            let now = self.0.get();
            self.0.set(Reading {
                mono: now.mono,
                wall: now.wall + by,
            });
        }
    }

    impl Clock for FakeClock {
        fn read(&self) -> Reading {
            self.0.get()
        }
    }

    const MINUTE: Duration = Duration::from_secs(60);

    /// The counter and the picker hold are process-wide: tests touching them take turns.
    static GLOBAL_STATE: Mutex<()> = Mutex::new(());

    fn global_state() -> MutexGuard<'static, ()> {
        GLOBAL_STATE
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn settings(enabled: bool, minutes: u32) -> AutolockSettings {
        AutolockSettings { enabled, minutes }
    }

    fn input(settings: &AutolockSettings) -> TickInput<'_> {
        TickInput {
            vault_open: true,
            settings,
            in_background: false,
            detect_sleep: false,
        }
    }

    fn with_sleep_detection(settings: &AutolockSettings) -> TickInput<'_> {
        TickInput {
            detect_sleep: true,
            ..input(settings)
        }
    }

    /// A watch with the vault just opened: the first tick starts the idle time.
    fn opened(clock: &FakeClock, settings: &AutolockSettings) -> Watch {
        let mut watch = Watch::default();
        assert_eq!(watch.tick(clock, &input(settings)), None);
        watch
    }

    #[test]
    fn idle_locks_after_the_minutes_and_not_before() {
        let clock = FakeClock::new();
        let config = settings(true, 15);
        let mut watch = opened(&clock, &config);
        clock.advance(MINUTE * 14);
        assert_eq!(watch.tick(&clock, &input(&config)), None);
        clock.advance(MINUTE);
        assert_eq!(watch.tick(&clock, &input(&config)), Some(LockReason::Idle));
    }

    #[test]
    fn activity_starts_the_idle_time_over() {
        let clock = FakeClock::new();
        let config = settings(true, 5);
        let mut watch = opened(&clock, &config);
        for _ in 0..10 {
            clock.advance(MINUTE * 4);
            watch.touch(&clock);
            assert_eq!(watch.tick(&clock, &input(&config)), None);
        }
        clock.advance(MINUTE * 5);
        assert_eq!(watch.tick(&clock, &input(&config)), Some(LockReason::Idle));
    }

    #[test]
    fn disabled_autolock_never_locks_on_idle() {
        let clock = FakeClock::new();
        let config = settings(false, 5);
        let mut watch = opened(&clock, &config);
        clock.advance(MINUTE * 600);
        assert_eq!(watch.tick(&clock, &input(&config)), None);
    }

    #[test]
    fn a_gap_between_ticks_is_a_sleep_even_with_the_autolock_off() {
        let clock = FakeClock::new();
        let config = settings(false, 15);
        let mut watch = opened(&clock, &config);
        clock.advance(TICK);
        assert_eq!(watch.tick(&clock, &with_sleep_detection(&config)), None);
        clock.sleep(MINUTE * 10);
        assert_eq!(
            watch.tick(&clock, &with_sleep_detection(&config)),
            Some(LockReason::Sleep)
        );
    }

    #[test]
    fn a_sleep_on_a_monotonic_clock_that_keeps_running_is_found_too() {
        // Windows: the monotonic clock counts the sleep, the wall clock too.
        let clock = FakeClock::new();
        let config = settings(true, 60);
        let mut watch = opened(&clock, &config);
        clock.advance(MINUTE * 3);
        assert_eq!(
            watch.tick(&clock, &with_sleep_detection(&config)),
            Some(LockReason::Sleep)
        );
    }

    #[test]
    fn normal_ticks_are_not_a_sleep() {
        let clock = FakeClock::new();
        let config = settings(true, 60);
        let mut watch = opened(&clock, &config);
        for _ in 0..100 {
            clock.advance(TICK);
            assert_eq!(watch.tick(&clock, &with_sleep_detection(&config)), None);
        }
    }

    #[test]
    fn a_closed_vault_resets_the_timer_for_the_next_one() {
        let clock = FakeClock::new();
        let config = settings(true, 5);
        let mut watch = opened(&clock, &config);
        clock.advance(MINUTE * 4);
        let mut closed = input(&config);
        closed.vault_open = false;
        assert_eq!(watch.tick(&clock, &closed), None);
        // Unlocked later: the old activity does not count.
        clock.advance(MINUTE * 30);
        let reopened = input(&config);
        assert_eq!(watch.tick(&clock, &reopened), None);
        clock.advance(MINUTE * 4);
        assert_eq!(watch.tick(&clock, &reopened), None);
    }

    #[test]
    fn in_the_background_the_idle_timer_does_not_lock() {
        let clock = FakeClock::new();
        let config = settings(true, 5);
        let mut watch = opened(&clock, &config);
        let mut background = input(&config);
        background.in_background = true;
        clock.advance(MINUTE * 40);
        assert_eq!(watch.tick(&clock, &background), None);
    }

    #[test]
    fn coming_back_to_the_foreground_restarts_the_idle_time() {
        let clock = FakeClock::new();
        let config = settings(true, 5);
        let mut watch = opened(&clock, &config);
        clock.advance(MINUTE * 40);
        watch.resumed(&clock);
        assert_eq!(watch.tick(&clock, &input(&config)), None);
        assert!(watch.idle_for(&clock) < MINUTE);
    }

    #[test]
    fn a_wall_clock_going_backwards_does_not_lock() {
        let clock = FakeClock::new();
        let config = settings(true, 5);
        let mut watch = opened(&clock, &config);
        let now = clock.0.get();
        clock.0.set(Reading {
            mono: now.mono + TICK,
            wall: now.wall - MINUTE * 30,
        });
        assert_eq!(watch.tick(&clock, &with_sleep_detection(&config)), None);
    }

    #[test]
    fn the_os_signals_map_to_reasons() {
        assert_eq!(
            classify_signal(
                "org.freedesktop.login1.Manager",
                "PrepareForSleep",
                Some(true)
            ),
            Some(LockReason::Sleep)
        );
        // Waking up is not a reason.
        assert_eq!(
            classify_signal(
                "org.freedesktop.login1.Manager",
                "PrepareForSleep",
                Some(false)
            ),
            None
        );
        assert_eq!(
            classify_signal("org.freedesktop.login1.Session", "Lock", None),
            Some(LockReason::ScreenLock)
        );
        assert_eq!(
            classify_signal("org.gnome.ScreenSaver", "ActiveChanged", Some(true)),
            Some(LockReason::ScreenLock)
        );
        assert_eq!(
            classify_signal("org.gnome.ScreenSaver", "ActiveChanged", Some(false)),
            None
        );
        assert_eq!(classify_signal("org.example", "Lock", None), None);
    }

    /// One test: the counter is process-wide.
    #[test]
    fn a_window_of_our_own_is_not_the_background() {
        let _turn = global_state();
        clear_picker_hold();
        assert!(counts_as_background(true));
        assert!(!counts_as_background(false));
        {
            let _picker = OwnWindow::open();
            let _prompt = OwnWindow::open();
            assert!(!counts_as_background(true));
        }
        assert!(counts_as_background(true));
        // An extra drop never underflows.
        drop(OwnWindow(()));
        assert!(counts_as_background(true));
    }

    #[test]
    fn a_picker_hold_protects_until_its_deadline_only() {
        let now = Instant::now();
        let later = now + Duration::from_secs(60);
        assert!(!counts_as_background_at(true, 0, Some(later), now));
        // Expired (and exactly at the deadline): never protects.
        assert!(counts_as_background_at(true, 0, Some(now), now));
        assert!(counts_as_background_at(true, 0, Some(now), later));
        assert!(counts_as_background_at(true, 0, None, now));
        // Not in the background: nothing to decide.
        assert!(!counts_as_background_at(false, 0, None, now));
        // An own window still protects without a hold.
        assert!(!counts_as_background_at(true, 1, None, now));
    }

    /// One test: the hold is process-wide (and the counter test above reads it).
    #[test]
    fn the_picker_hold_is_set_cleared_and_bounded() {
        let _turn = global_state();
        clear_picker_hold();
        assert!(counts_as_background(true));
        hold_for_system_picker();
        let until = picker_hold().expect("hold set").until;
        let now = Instant::now();
        assert!(hold_protects(Some(until), now));
        assert!(until <= now + PICKER_HOLD);
        assert!(!counts_as_background(true));
        clear_picker_hold();
        assert!(counts_as_background(true));
    }

    #[test]
    fn the_picker_hold_ends_only_on_return_from_the_background() {
        let set = Some(PickerHold {
            until: Instant::now() + PICKER_HOLD,
            left: false,
        });
        // Foreground before the picker has opened: the hold stays.
        assert_eq!(next_picker_hold(set, false), set);
        let away = next_picker_hold(set, true);
        assert!(away.is_some_and(|hold| hold.left));
        assert_eq!(next_picker_hold(away, true), away);
        // Back from the picker: the hold ends.
        assert_eq!(next_picker_hold(away, false), None);
        assert_eq!(next_picker_hold(None, true), None);
    }
}
