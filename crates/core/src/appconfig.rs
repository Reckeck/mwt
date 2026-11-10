//! Application-level settings — theme, language, base currency and the
//! favourite currencies shown next to a price.
//!
//! These live *outside* the encrypted vault, as plain JSON in the app config
//! directory, so a second vault starts with the preferences the user already
//! configured instead of asking again. Nothing here is secret, so there is no
//! reason to pay for encryption or to keep a copy in every vault.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::Amount;

use crate::error::{Error, Result};
use crate::ipc_contract::security::AutolockSettings;
use crate::jsonstore;

/// Settings file name inside the app config directory.
pub const SETTINGS_FILE: &str = "app_settings.json";
/// Cache of the currency catalog fetched from the public providers.
pub const CURRENCIES_FILE: &str = "currencies.json";

/// A cached catalog younger than this is served without asking the network:
/// currency lists change rarely, and the pickers should open instantly.
pub const CURRENCIES_FRESH_FOR: std::time::Duration = std::time::Duration::from_secs(24 * 3600);

/// Portfolio base until the user picks another one.
pub const DEFAULT_BASE_CURRENCY: &str = "usd";

/// Last-resort fiat catalog, used only when the providers are unreachable *and*
/// no successful fetch was ever cached. Keeps the pickers usable offline.
pub const FALLBACK_FIAT: &[&str] = &[
    "usd", "eur", "rub", "cny", "gbp", "jpy", "chf", "cad", "aud", "inr", "brl", "krw", "try",
    "mxn", "sgd", "hkd", "nzd", "sek", "nok", "pln", "czk", "huf", "ron", "bgn",
];

/// Last-resort crypto catalog, same reasoning as [`FALLBACK_FIAT`].
pub const FALLBACK_CRYPTO: &[&str] = &[
    "btc", "eth", "sol", "usdt", "usdc", "bnb", "xrp", "ada", "doge", "trx", "ton", "avax", "link",
    "dot", "matic", "ltc", "bch", "shib", "uni", "aave", "arb", "op",
];

// -- enums ---------------------------------------------------------------------

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    #[default]
    Auto,
    Light,
    Dark,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum Locale {
    #[default]
    Auto,
    En,
    Ru,
}

// -- settings ------------------------------------------------------------------

/// Preferences that follow the *user*, not the vault: they must survive opening
/// a different vault, so they are stored next to the app, not inside the doc.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct AppSettings {
    pub theme: Theme,
    pub locale: Locale,
    /// ISO 4217 code the whole portfolio is valued in (e.g. `usd`, `rub`).
    pub base_currency: String,
    /// The user has picked the base currency: the first create or open of a
    /// vault stops asking. A file from before the field reads as `true`, so
    /// nobody who already uses the app is asked again.
    #[serde(default = "chosen_before_the_field")]
    pub base_currency_chosen: bool,
    /// Currencies offered on hover next to any price. Empty = no tooltip.
    #[serde(default)]
    pub favorite_currencies: Vec<String>,
    /// IANA zone days and months are cut in (plan §7.6); `None` = the OS zone.
    #[serde(default)]
    pub timezone: Option<String>,
    /// Annual inflation, percent, by lowercase currency code (plan §6.5).
    /// Read only by `adopt_flat_inflation`: the rates live in the vault since
    /// stage P and this column is no longer written.
    #[serde(default = "default_inflation")]
    pub inflation: BTreeMap<String, Amount>,
    /// Consent to send transaction hashes to public explorers (plan §8.2);
    /// `None` = not asked yet.
    #[serde(default)]
    pub enrichment_opt_in: Option<bool>,
    /// System-wide key combination that opens "+ Transaction", in the
    /// `CmdOrCtrl+Shift+Space` form; `None` = off, so it never fights the
    /// window manager's own bindings unless the user asks for it.
    #[serde(default)]
    pub quick_add_shortcut: Option<String>,
    /// Guided tours already shown (by page), so a tour starts by itself only once.
    #[serde(default)]
    pub completed_tours: Vec<String>,
    /// "What's new" entries already shown after an update.
    #[serde(default)]
    pub seen_whats_new: Vec<String>,
    /// This installation's id for vault sync; made once by [`Self::ensure_device_id`].
    /// It lives here, not in the vault, so a copied vault file never shares it.
    #[serde(default)]
    pub device_id: String,
    /// Name other devices see when pairing; `None` = the host name.
    #[serde(default)]
    pub device_name: Option<String>,
    /// Device-to-device vault sync over the local network.
    #[serde(default)]
    pub sync: SyncPrefs,
    /// Hide to the tray on close. `None` until taken over from the vault
    /// that used to keep it ([`Self::adopt_device_settings`]).
    #[serde(default)]
    pub minimize_to_tray: Option<bool>,
    /// Check for app updates on start and in the background; `None` = on.
    #[serde(default)]
    pub auto_update: Option<bool>,
    /// When the last update check finished (RFC 3339).
    #[serde(default)]
    pub last_update_check: Option<String>,
    /// How often prices and rates refresh here, seconds; `None` = not taken over yet.
    #[serde(default)]
    pub fx_refresh_interval_sec: Option<u32>,
    /// Decimal places balances render with on this screen; `None` = not taken over yet.
    #[serde(default)]
    pub display_decimals: Option<u32>,
    /// Day this installation first ran (`YYYY-MM-DD`): "What's new" skips what
    /// came before it. `None` for installs older than the field.
    #[serde(default)]
    pub installed_at: Option<String>,
    /// The dashboard's "Today" strip is folded to one line on this screen.
    #[serde(default)]
    pub dashboard_today_collapsed: bool,
    /// A screen offers its tour on the first visit (a banner, never a window
    /// that opens by itself). On for everyone until "Don't offer again".
    #[serde(default = "tour_offers_on")]
    pub tour_offers: bool,
    /// Screens whose tour offer was waved off with "Not now": not offered again.
    #[serde(default)]
    pub declined_tours: Vec<String>,
    /// Devices a received vault copy must trust once it is unlocked. Kept here,
    /// not in memory, so a process kill (Android) between receiving and the
    /// first unlock does not lose the pairing.
    #[serde(default)]
    #[ts(skip)]
    pub pending_trusts: Vec<PendingTrustPref>,
    /// Autolock of this device (vault protection, `crate::ipc_contract::security`);
    /// `None` = the defaults ([`Self::autolock`]).
    #[serde(default)]
    #[ts(optional)]
    pub autolock: Option<AutolockSettings>,
    /// Wrong PINs in a row per level-2 vault file (the path), for the delay
    /// before the next try. Kept here so a restart does not reset it.
    #[serde(default)]
    #[ts(skip)]
    pub pin_failures: BTreeMap<String, PinFailures>,
}

/// Autolock range and default, minutes of idle time.
pub const AUTOLOCK_MIN_MINUTES: u32 = 5;
pub const AUTOLOCK_MAX_MINUTES: u32 = 60;
pub const AUTOLOCK_DEFAULT_MINUTES: u32 = 15;

/// What a device that never chose gets: on, after 15 minutes.
pub fn default_autolock() -> AutolockSettings {
    AutolockSettings {
        enabled: true,
        minutes: AUTOLOCK_DEFAULT_MINUTES,
    }
}

/// Check what the user asked for. The minutes stay inside 5-60 even when the
/// lock is off (the field keeps its place); turning it off is allowed only
/// while the device holds no exchange keys.
pub fn validate_autolock(
    input: &AutolockSettings,
    has_device_keys: bool,
) -> Result<AutolockSettings> {
    if !(AUTOLOCK_MIN_MINUTES..=AUTOLOCK_MAX_MINUTES).contains(&input.minutes) {
        return Err(Error::InvalidInput(format!(
            "autolock minutes must be {AUTOLOCK_MIN_MINUTES}-{AUTOLOCK_MAX_MINUTES}"
        )));
    }
    if !input.enabled && has_device_keys {
        return Err(Error::InvalidInput(
            "autolock cannot be turned off while the device holds exchange keys".into(),
        ));
    }
    Ok(input.clone())
}

/// Wrong PINs typed in a row for one vault, and when the last one was.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PinFailures {
    pub count: u32,
    /// Unix seconds of the last wrong PIN.
    pub last_failed_at_secs: u64,
}

/// Wrong PINs that cost nothing; the delays after them grow.
pub const PIN_FREE_ATTEMPTS: u32 = 5;

/// The wait after `count` wrong PINs in a row: the first five tries are free,
/// then 30 s, 1 min, 5 min, and 15 min from then on. It slows a person at the
/// unlock screen; it does not stop anyone holding a copy of the file.
pub fn pin_retry_delay_secs(count: u32) -> u64 {
    if count < PIN_FREE_ATTEMPTS {
        return 0;
    }
    match count - PIN_FREE_ATTEMPTS {
        0 => 30,
        1 => 60,
        2 => 300,
        _ => 900,
    }
}

/// Seconds left before the next PIN may be tried at `now_secs`; 0 = now. A
/// clock moved back never makes the wait longer than the delay itself.
pub fn pin_retry_after_secs(failures: Option<&PinFailures>, now_secs: u64) -> u64 {
    let Some(failures) = failures else {
        return 0;
    };
    let delay = pin_retry_delay_secs(failures.count);
    let since = now_secs.saturating_sub(failures.last_failed_at_secs);
    delay.saturating_sub(since).min(delay)
}

/// The device a just-received vault copy came from, to be trusted at its first
/// unlock. Keyed by the local file, the sender's id and its public key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingTrustPref {
    pub path: String,
    pub device_id: String,
    pub device_name: String,
    pub device_key: String,
    /// Unix milliseconds after which the entry is dropped unused.
    pub expires_ms: u64,
}

/// Vault sync preferences of this device. Everything is off until the user
/// turns it on: an enabled device listens on the local network.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct SyncPrefs {
    #[serde(default)]
    pub enabled: bool,
    /// Show what will change before the first batch of a session is applied.
    #[serde(default)]
    pub confirm_first: bool,
    /// Take the main role whenever this device is around.
    #[serde(default)]
    pub prefer_main: bool,
    /// Android: minutes in the background without a sync before the vault
    /// locks itself and background sync stops; `0` = never, `None` = default.
    #[serde(default)]
    pub background_lock_min: Option<u32>,
}

/// Default of [`SyncPrefs::background_lock_min`].
pub const DEFAULT_BACKGROUND_LOCK_MIN: u32 = 30;

impl SyncPrefs {
    pub fn background_lock_min(&self) -> u32 {
        self.background_lock_min
            .unwrap_or(DEFAULT_BACKGROUND_LOCK_MIN)
    }
}

/// Defaults of the device settings that used to live in the vault.
pub const DEFAULT_MINIMIZE_TO_TRAY: bool = true;
pub const DEFAULT_FX_REFRESH_SEC: u32 = 300;
pub const DEFAULT_DISPLAY_DECIMALS: u32 = 8;

/// Reference annual inflation the user starts with, percent. Rough recent
/// figures; the settings screen says so and lets the user change them.
pub fn default_inflation() -> BTreeMap<String, Amount> {
    [("rub", "8"), ("usd", "3"), ("eur", "2.5"), ("thb", "1")]
        .into_iter()
        .filter_map(|(code, rate)| Some((code.to_string(), Amount::parse(rate).ok()?)))
        .collect()
}

fn chosen_before_the_field() -> bool {
    true
}

fn tour_offers_on() -> bool {
    true
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: Theme::Auto,
            locale: Locale::Auto,
            base_currency: DEFAULT_BASE_CURRENCY.into(),
            base_currency_chosen: false,
            favorite_currencies: Vec::new(),
            timezone: None,
            inflation: default_inflation(),
            enrichment_opt_in: None,
            quick_add_shortcut: None,
            completed_tours: Vec::new(),
            seen_whats_new: Vec::new(),
            device_id: String::new(),
            device_name: None,
            sync: SyncPrefs::default(),
            minimize_to_tray: None,
            auto_update: None,
            last_update_check: None,
            fx_refresh_interval_sec: None,
            display_decimals: None,
            installed_at: None,
            dashboard_today_collapsed: false,
            tour_offers: true,
            declined_tours: Vec::new(),
            pending_trusts: Vec::new(),
            autolock: None,
            pin_failures: BTreeMap::new(),
        }
    }
}

impl AppSettings {
    /// Seconds before the next PIN for the vault at `path` may be tried.
    pub fn pin_retry_after(&self, path: &str, now_secs: u64) -> u64 {
        pin_retry_after_secs(self.pin_failures.get(path), now_secs)
    }

    /// Count a wrong PIN for the vault at `path`.
    pub fn record_pin_failure(&mut self, path: &str, now_secs: u64) {
        let entry = self.pin_failures.entry(path.to_string()).or_default();
        entry.count = entry.count.saturating_add(1);
        entry.last_failed_at_secs = now_secs;
    }

    /// Forget the wrong PINs for `path` after it opened; `true` if any were kept.
    pub fn reset_pin_failures(&mut self, path: &str) -> bool {
        self.pin_failures.remove(path).is_some()
    }

    /// The autolock in force: the stored one, else the defaults.
    pub fn autolock(&self) -> AutolockSettings {
        self.autolock.clone().unwrap_or_else(default_autolock)
    }

    /// Base currency, lowercased and never empty.
    ///
    /// `normalize`/`apply` are the only ways to change the field and both keep
    /// it lowercase and non-empty, so this needs no fallbacks.
    pub fn base(&self) -> &str {
        &self.base_currency
    }

    /// Apply a partial update in place, normalizing what it touches.
    pub fn apply(&mut self, patch: &UpdateAppSettingsInput) {
        if let Some(theme) = patch.theme {
            self.theme = theme;
        }
        if let Some(locale) = patch.locale {
            self.locale = locale;
        }
        if let Some(base) = &patch.base_currency
            && !base.trim().is_empty()
        {
            self.base_currency = base.trim().to_lowercase();
            self.base_currency_chosen = true;
        }
        if let Some(chosen) = patch.base_currency_chosen {
            self.base_currency_chosen = chosen;
        }
        if let Some(favorites) = &patch.favorite_currencies {
            self.favorite_currencies = normalize_codes(favorites);
        }
        if let Some(zone) = &patch.timezone {
            self.timezone = normalize_zone(zone);
        }
        if let Some(consent) = patch.enrichment_opt_in {
            self.enrichment_opt_in = Some(consent);
        }
        if let Some(tours) = &patch.completed_tours {
            self.completed_tours = dedup(tours);
        }
        if let Some(entries) = &patch.seen_whats_new {
            self.seen_whats_new = dedup(entries);
        }
        if let Some(name) = &patch.device_name {
            let name: String = name.trim().chars().take(MAX_DEVICE_NAME).collect();
            self.device_name = (!name.is_empty()).then_some(name);
        }
        if let Some(sync) = patch.sync {
            self.sync = sync;
        }
        if let Some(minimize) = patch.minimize_to_tray {
            self.minimize_to_tray = Some(minimize);
        }
        if let Some(auto_update) = patch.auto_update {
            self.auto_update = Some(auto_update);
        }
        if let Some(stamp) = &patch.last_update_check {
            self.last_update_check = Some(stamp.clone());
        }
        if let Some(interval) = patch.fx_refresh_interval_sec {
            self.fx_refresh_interval_sec = Some(interval.clamp(30, 3600));
        }
        if let Some(offers) = patch.tour_offers {
            self.tour_offers = offers;
        }
        if let Some(tours) = &patch.declined_tours {
            self.declined_tours = dedup(tours);
        }
        if let Some(collapsed) = patch.dashboard_today_collapsed {
            self.dashboard_today_collapsed = collapsed;
        }
        if let Some(decimals) = patch.display_decimals {
            self.display_decimals = Some(decimals.min(18));
        }
    }

    /// Give this installation its sync id if it has none yet; `true` = changed.
    pub fn ensure_device_id(&mut self) -> bool {
        if uuid::Uuid::parse_str(&self.device_id).is_ok() {
            return false;
        }
        self.device_id = uuid::Uuid::new_v4().to_string();
        true
    }

    /// Give this device a friendly name if it has none yet (an adjective and
    /// a fruit, in Russian when `ru`); `true` = changed.
    pub fn ensure_device_name(&mut self, ru: bool) -> bool {
        if self
            .device_name
            .as_ref()
            .is_some_and(|name| !name.trim().is_empty())
        {
            return false;
        }
        self.device_name = Some(crate::sync::alias::generate(ru, &mut rand::thread_rng()));
        true
    }

    /// Take over the device settings a vault used to keep, for each one not
    /// set here yet; `true` = something was taken. Settings already set here
    /// win: they belong to this device, the vault's copy to whoever saved last.
    pub fn adopt_device_settings(&mut self, vault: &crate::vault::Settings) -> bool {
        let before = (
            self.minimize_to_tray,
            self.fx_refresh_interval_sec,
            self.display_decimals,
        );
        self.minimize_to_tray.get_or_insert(vault.minimize_to_tray);
        self.fx_refresh_interval_sec
            .get_or_insert(vault.fx_refresh_interval_sec.clamp(30, 3600));
        self.display_decimals
            .get_or_insert(vault.display_decimals.min(18));
        before
            != (
                self.minimize_to_tray,
                self.fx_refresh_interval_sec,
                self.display_decimals,
            )
    }

    pub fn auto_update(&self) -> bool {
        self.auto_update.unwrap_or(true)
    }

    pub fn minimize_to_tray(&self) -> bool {
        self.minimize_to_tray.unwrap_or(DEFAULT_MINIMIZE_TO_TRAY)
    }

    pub fn fx_refresh_interval(&self) -> u32 {
        self.fx_refresh_interval_sec
            .unwrap_or(DEFAULT_FX_REFRESH_SEC)
    }

    /// Annual inflation of a currency, percent, if the user has one for it.
    pub fn inflation_of(&self, currency: &str) -> Option<Amount> {
        self.inflation.get(&currency.trim().to_lowercase()).copied()
    }

    /// The zone to cut days in: the chosen one, else `system` (the OS zone
    /// the app layer detected), else UTC.
    pub fn zone_or(&self, system: Option<&str>) -> String {
        self.timezone
            .clone()
            .or_else(|| system.and_then(normalize_zone))
            .unwrap_or_else(|| "UTC".into())
    }

    /// Drop values we could not use anyway, so a hand-edited file cannot leave
    /// the UI rendering `UNDEFINED` as a currency.
    pub fn normalize(&mut self) {
        self.base_currency = {
            let trimmed = self.base_currency.trim().to_lowercase();
            if trimmed.is_empty() {
                DEFAULT_BASE_CURRENCY.into()
            } else {
                trimmed
            }
        };
        self.favorite_currencies = normalize_codes(&self.favorite_currencies);
        self.timezone = self.timezone.as_deref().and_then(normalize_zone);
    }
}

/// Lowercase codes, rates within ±1000 %; anything else is dropped.
pub(crate) fn normalize_inflation(raw: &BTreeMap<String, Amount>) -> BTreeMap<String, Amount> {
    let bound = Amount::parse("1000").unwrap_or_default();
    raw.iter()
        .filter(|(code, rate)| !code.trim().is_empty() && rate.abs() <= bound)
        .map(|(code, rate)| (code.trim().to_lowercase(), *rate))
        .collect()
}

/// A known IANA zone name, trimmed; empty or unknown = none.
fn normalize_zone(raw: &str) -> Option<String> {
    let zone = raw.trim();
    crate::calendar::Calendar::is_valid_zone(zone).then(|| zone.to_string())
}

/// Longest currency or coin ticker a favourite may hold (`1INCH`, `MATIC`, …).
const MAX_CODE_LEN: usize = 10;

/// Uppercase, deduplicated, in the user's order: the order ranks the
/// conversions shown under an amount. Fiat ISO codes and crypto tickers both
/// pass (2–10 letters or digits, at least one letter). The base currency is
/// kept: the UI hides it, and a later base change must not lose it.
fn normalize_codes(raw: &[String]) -> Vec<String> {
    let mut codes: Vec<String> = Vec::new();
    for code in raw {
        let code = code.trim().to_uppercase();
        if is_ticker(&code) && !codes.contains(&code) {
            codes.push(code);
        }
    }
    codes
}

fn is_ticker(code: &str) -> bool {
    (2..=MAX_CODE_LEN).contains(&code.len())
        && code.chars().all(|ch| ch.is_ascii_alphanumeric())
        && code.chars().any(|ch| ch.is_ascii_alphabetic())
}

/// Partial update: only the fields present in the payload are touched.
#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct UpdateAppSettingsInput {
    pub theme: Option<Theme>,
    pub locale: Option<Locale>,
    pub base_currency: Option<String>,
    /// Marks the base currency as picked without changing it.
    #[serde(default)]
    pub base_currency_chosen: Option<bool>,
    pub favorite_currencies: Option<Vec<String>>,
    /// IANA zone; an empty string goes back to the OS zone.
    #[serde(default)]
    pub timezone: Option<String>,
    /// Ignored since the flat rates moved into the vault
    /// (`UpdateSettingsInput.inflation_manual`); kept so old callers still parse.
    #[serde(default)]
    pub inflation: Option<BTreeMap<String, Amount>>,
    #[serde(default)]
    pub enrichment_opt_in: Option<bool>,
    /// Replaces the list of tours already shown; `[]` shows them all again.
    #[serde(default)]
    pub completed_tours: Option<Vec<String>>,
    /// Replaces the list of "What's new" entries already shown.
    #[serde(default)]
    pub seen_whats_new: Option<Vec<String>>,
    /// An empty name goes back to the host name.
    #[serde(default)]
    pub device_name: Option<String>,
    #[serde(default)]
    pub sync: Option<SyncPrefs>,
    #[serde(default)]
    pub minimize_to_tray: Option<bool>,
    #[serde(default)]
    pub auto_update: Option<bool>,
    /// RFC 3339 time of the last update check.
    #[serde(default)]
    pub last_update_check: Option<String>,
    #[serde(default)]
    pub fx_refresh_interval_sec: Option<u32>,
    #[serde(default)]
    pub display_decimals: Option<u32>,
    #[serde(default)]
    pub dashboard_today_collapsed: Option<bool>,
    /// Whether screens offer their tour on the first visit.
    #[serde(default)]
    pub tour_offers: Option<bool>,
    /// Replaces the list of screens whose offer was declined.
    #[serde(default)]
    pub declined_tours: Option<Vec<String>>,
}

/// Longest device name, characters; it is shown in a pairing dialog.
pub const MAX_DEVICE_NAME: usize = 40;

/// Trimmed, without blanks and repeats, in order.
fn dedup(items: &[String]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    items
        .iter()
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty() && seen.insert(item.clone()))
        .collect()
}

// -- currency catalog ----------------------------------------------------------

/// Currencies the pickers may offer.
///
/// Built from the public providers at runtime and cached on disk; see
/// [`fallback`] for the degraded case.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct SupportedCurrencies {
    /// ISO 4217 codes, lowercase.
    #[serde(default)]
    pub fiat: Vec<String>,
    /// Crypto ticker symbols, lowercase.
    #[serde(default)]
    pub crypto: Vec<String>,
}

impl SupportedCurrencies {
    /// Built-in catalog for a machine that has never reached the network.
    pub fn fallback() -> Self {
        let mut catalog = Self {
            fiat: FALLBACK_FIAT
                .iter()
                .map(|code| (*code).to_string())
                .collect(),
            crypto: FALLBACK_CRYPTO
                .iter()
                .map(|code| (*code).to_string())
                .collect(),
        };
        catalog.normalize();
        catalog
    }

    /// The same catalog with the built-in majors added, so a half the network
    /// failed to deliver still offers BTC, USDT, EUR…
    fn with_fallback(mut self) -> Self {
        let builtin = Self::fallback();
        self.fiat.extend(builtin.fiat);
        self.crypto.extend(builtin.crypto);
        self.normalize();
        self
    }

    /// Both halves present: worth caching and serving without a refetch.
    pub fn is_complete(&self) -> bool {
        !self.fiat.is_empty() && !self.crypto.is_empty()
    }

    /// Union of both catalogs, sorted and deduplicated — what a picker renders.
    pub fn all(&self) -> Vec<String> {
        let mut codes: BTreeSet<String> = BTreeSet::new();
        codes.extend(self.fiat.iter().cloned());
        codes.extend(self.crypto.iter().cloned());
        codes.into_iter().collect()
    }

    /// Trim, lowercase and dedupe a catalog coming from the network.
    pub fn normalize(&mut self) {
        self.fiat = normalize_list(std::mem::take(&mut self.fiat));
        self.crypto = normalize_list(std::mem::take(&mut self.crypto));
    }

    /// True when there is nothing to offer and the caller should fall back.
    pub fn is_empty(&self) -> bool {
        self.fiat.is_empty() && self.crypto.is_empty()
    }
}

fn normalize_list(raw: Vec<String>) -> Vec<String> {
    let mut codes: BTreeSet<String> = BTreeSet::new();
    for code in raw {
        let code = code.trim().to_lowercase();
        if !code.is_empty() {
            codes.insert(code);
        }
    }
    codes.into_iter().collect()
}

// -- persistence ---------------------------------------------------------------

/// Absolute path of the settings file.
pub fn settings_path(config_dir: &Path) -> PathBuf {
    config_dir.join(SETTINGS_FILE)
}

/// Absolute path of the cached currency catalog.
pub fn currencies_path(config_dir: &Path) -> PathBuf {
    config_dir.join(CURRENCIES_FILE)
}

/// Read the settings, falling back to defaults when the file is missing or
/// unreadable. A corrupt file must never keep the app from starting; its
/// bytes are kept next to it (see [`crate::jsonstore`]).
pub fn load_settings(config_dir: &Path) -> AppSettings {
    read_settings(config_dir).1
}

/// [`load_settings`], also telling whether there was no file at all — the
/// only case that is a first run. A file that would not parse is not one:
/// the user did have settings, and they are kept as `.broken` instead.
fn read_settings(config_dir: &Path) -> (bool, AppSettings) {
    let load = jsonstore::read::<AppSettings>(&settings_path(config_dir));
    let first_run = matches!(load, jsonstore::Load::Missing);
    let mut settings = load.or_default_logged("app settings");
    settings.normalize();
    (first_run, settings)
}

/// [`load_settings`], but a first run (no settings file yet) is stamped with
/// `today` and saved, so later reads know what came before the install.
pub fn load_or_init_settings(config_dir: &Path, today: &str) -> AppSettings {
    let (first_run, mut settings) = read_settings(config_dir);
    if first_run {
        settings.installed_at = Some(today.to_string());
        // Unsaved, the next start is a "first run" again: harmless.
        let _ = save_settings(config_dir, &settings);
    }
    settings
}

/// Write the settings, creating the config directory when needed.
pub fn save_settings(config_dir: &Path, settings: &AppSettings) -> Result<()> {
    jsonstore::write(&settings_path(config_dir), settings)
}

/// Read the last successfully fetched catalog, if there is one.
pub fn load_currencies(config_dir: &Path) -> Option<SupportedCurrencies> {
    let mut catalog = jsonstore::read::<SupportedCurrencies>(&currencies_path(config_dir))
        .or_default_logged("currency cache");
    catalog.normalize();
    // An empty catalog is no catalog: a missing file, and a broken one, both
    // send the caller to the network or the built-in list.
    (!catalog.is_empty()).then_some(catalog)
}

/// Cache the catalog so the next offline start still offers real currencies.
pub fn save_currencies(config_dir: &Path, catalog: &SupportedCurrencies) -> Result<()> {
    jsonstore::write(&currencies_path(config_dir), catalog)
}

/// Where a currency catalog came from — drives the notice next to the pickers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogSource {
    /// At least one provider answered just now.
    Network,
    /// A recent successful fetch, reused without a network round trip.
    Fresh,
    /// Every provider failed; the last successful fetch was reused.
    Cache,
    /// Nothing was ever fetched; the built-in list.
    Builtin,
}

impl CatalogSource {
    /// Machine code for the UI notice; `None` when the list is fresh.
    pub fn notice(self) -> Option<&'static str> {
        match self {
            Self::Network | Self::Fresh => None,
            Self::Cache => Some("cached"),
            Self::Builtin => Some("builtin"),
        }
    }
}

/// Build the currency catalog for the settings pickers.
///
/// Degradation ladder, in order:
/// 1. both halves fetched from the public providers — the catalog is cached on
///    disk so the next offline start is still complete;
/// 2. whatever the last successful fetch left on disk (a half that failed this
///    time is kept from the cache rather than dropped);
/// 3. the built-in [`SupportedCurrencies::fallback`] list.
///
/// It never fails: a picker with a short list beats a picker that will not open.
pub async fn build_currency_catalog(
    config_dir: &Path,
    pricing: &crate::pricing::PricingService,
) -> (SupportedCurrencies, CatalogSource) {
    let cached = load_currencies(config_dir);
    // A cache missing a half (an older build wrote those) is refetched.
    if let Some(catalog) = cached.as_ref()
        && catalog.is_complete()
        && is_fresh(&currencies_path(config_dir), CURRENCIES_FRESH_FOR)
    {
        return (catalog.clone(), CatalogSource::Fresh);
    }
    let fiat = match pricing.fx_rates().await {
        Ok(rates) => {
            let mut codes: Vec<String> = rates.into_keys().collect();
            codes.sort();
            Some(codes)
        }
        Err(_) => None,
    };
    let crypto = pricing.coin_symbols().await.ok();
    let (catalog, source, complete) = merge_catalog(fiat, crypto, cached);
    // Only a catalog worth remembering is written back; caching a half-empty
    // result would freeze the gap in place on every later offline start.
    if source == CatalogSource::Network && complete {
        let _ = save_currencies(config_dir, &catalog);
    }
    (catalog, source)
}

/// Whether a file was written less than `max_age` ago; unknown mtime is stale.
fn is_fresh(path: &Path, max_age: std::time::Duration) -> bool {
    std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|modified| modified.elapsed().ok())
        .is_some_and(|age| age < max_age)
}

/// Combine this run's fetch results with the cache. A half that failed is
/// taken from the cache; the source reflects whether the network answered at
/// all, not whether the list changed. The built-in majors are always mixed in;
/// the flag says whether both halves came from the network or the cache.
fn merge_catalog(
    fiat: Option<Vec<String>>,
    crypto: Option<Vec<String>>,
    cached: Option<SupportedCurrencies>,
) -> (SupportedCurrencies, CatalogSource, bool) {
    let fetched_something = fiat.is_some() || crypto.is_some();
    let mut catalog = SupportedCurrencies {
        fiat: fiat
            .or_else(|| cached.as_ref().map(|previous| previous.fiat.clone()))
            .unwrap_or_default(),
        crypto: crypto
            .or_else(|| cached.as_ref().map(|previous| previous.crypto.clone()))
            .unwrap_or_default(),
    };
    catalog.normalize();

    if catalog.is_empty() {
        return (
            SupportedCurrencies::fallback(),
            CatalogSource::Builtin,
            false,
        );
    }
    let complete = catalog.is_complete();
    let source = if fetched_something {
        CatalogSource::Network
    } else {
        CatalogSource::Cache
    };
    (catalog.with_fallback(), source, complete)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn five_wrong_pins_are_free_then_the_wait_grows() {
        let delays: Vec<u64> = (0..=10).map(pin_retry_delay_secs).collect();
        assert_eq!(delays, [0, 0, 0, 0, 0, 30, 60, 300, 900, 900, 900]);
    }

    #[test]
    fn the_pin_delay_survives_a_restart_and_resets_on_success() {
        let dir = temp_dir("pin-delay");
        let mut settings = AppSettings::default();
        for _ in 0..5 {
            settings.record_pin_failure("/v.wallet", 1_000);
        }
        assert_eq!(settings.pin_retry_after("/v.wallet", 1_000), 30);
        assert_eq!(settings.pin_retry_after("/v.wallet", 1_020), 10);
        assert_eq!(settings.pin_retry_after("/v.wallet", 1_030), 0);
        assert_eq!(settings.pin_retry_after("/other.wallet", 1_000), 0);
        // A clock moved back does not make the wait longer than the delay.
        assert_eq!(settings.pin_retry_after("/v.wallet", 10), 30);
        save_settings(&dir, &settings).unwrap();
        let mut loaded = load_settings(&dir);
        assert_eq!(loaded.pin_retry_after("/v.wallet", 1_000), 30);
        loaded.record_pin_failure("/v.wallet", 1_100);
        assert_eq!(loaded.pin_retry_after("/v.wallet", 1_100), 60);
        assert!(loaded.reset_pin_failures("/v.wallet"));
        assert_eq!(loaded.pin_retry_after("/v.wallet", 1_100), 0);
        assert!(!loaded.reset_pin_failures("/v.wallet"));
        std::fs::remove_dir_all(dir).ok();
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("tradeapp-appconfig-{tag}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    #[test]
    fn defaults_are_sane() {
        let settings = AppSettings::default();
        assert_eq!(settings.base(), DEFAULT_BASE_CURRENCY);
        assert_eq!(settings.theme, Theme::Auto);
        assert_eq!(settings.locale, Locale::Auto);
        assert!(settings.favorite_currencies.is_empty());
    }

    #[test]
    fn base_currency_is_asked_only_on_a_fresh_install() {
        assert!(!AppSettings::default().base_currency_chosen);
        let old: AppSettings =
            serde_json::from_str(r#"{"theme":"auto","locale":"auto","base_currency":"rub"}"#)
                .expect("settings from before the field");
        assert!(old.base_currency_chosen);
    }

    #[test]
    fn picking_a_base_currency_marks_it_chosen() {
        let mut settings = AppSettings::default();
        settings.apply(&UpdateAppSettingsInput {
            theme: Some(Theme::Dark),
            ..Default::default()
        });
        assert!(!settings.base_currency_chosen);
        settings.apply(&UpdateAppSettingsInput {
            base_currency: Some("eur".into()),
            ..Default::default()
        });
        assert!(settings.base_currency_chosen);
    }

    #[test]
    fn patch_touches_only_given_fields() {
        let mut settings = AppSettings::default();
        settings.apply(&UpdateAppSettingsInput {
            theme: Some(Theme::Dark),
            ..Default::default()
        });
        assert_eq!(settings.theme, Theme::Dark);
        assert_eq!(settings.locale, Locale::Auto);
        assert_eq!(settings.base(), DEFAULT_BASE_CURRENCY);
    }

    #[test]
    fn patch_normalizes_currency_codes() {
        let mut settings = AppSettings::default();
        settings.apply(&UpdateAppSettingsInput {
            base_currency: Some("  RuB ".into()),
            favorite_currencies: Some(vec![
                " rub ".into(),
                "eur".into(),
                "RUB".into(),
                "usdt".into(),
                "u$".into(),
                "123".into(),
                String::new(),
            ]),
            ..Default::default()
        });
        assert_eq!(settings.base(), "rub");
        // Uppercase, deduplicated, tickers only, in the order given.
        assert_eq!(settings.favorite_currencies, vec!["RUB", "EUR", "USDT"]);
    }

    #[test]
    fn crypto_tickers_survive_as_favorites() {
        let mut settings = AppSettings::default();
        settings.apply(&UpdateAppSettingsInput {
            favorite_currencies: Some(
                ["btc", "USDT", "doge", "1inch", "matic", "ton"]
                    .map(String::from)
                    .to_vec(),
            ),
            ..Default::default()
        });
        assert_eq!(
            settings.favorite_currencies,
            vec!["BTC", "USDT", "DOGE", "1INCH", "MATIC", "TON"]
        );
    }

    #[test]
    fn numeric_only_codes_are_dropped() {
        let codes = normalize_codes(&["123".into(), "1".into(), "x".into(), "ABCDEFGHIJK".into()]);
        assert!(codes.is_empty());
    }

    #[test]
    fn favorites_keep_the_user_order() {
        let codes = normalize_codes(&["usd".into(), "btc".into(), "eur".into(), "USD".into()]);
        assert_eq!(codes, vec!["USD", "BTC", "EUR"]);
    }

    #[test]
    fn empty_base_currency_is_refused() {
        let mut settings = AppSettings::default();
        settings.apply(&UpdateAppSettingsInput {
            base_currency: Some("   ".into()),
            ..Default::default()
        });
        assert_eq!(settings.base(), DEFAULT_BASE_CURRENCY);
    }

    #[test]
    fn normalize_repairs_hand_edited_values() {
        let mut settings = AppSettings {
            base_currency: "  ".into(),
            favorite_currencies: vec!["usd".into(), "!!".into(), "gbp".into()],
            ..Default::default()
        };
        settings.normalize();
        assert_eq!(settings.base_currency, DEFAULT_BASE_CURRENCY);
        assert_eq!(settings.favorite_currencies, vec!["USD", "GBP"]);
    }

    #[test]
    fn timezone_keeps_known_zones_and_empty_means_the_os_zone() {
        let mut settings = AppSettings::default();
        settings.apply(&UpdateAppSettingsInput {
            timezone: Some(" Asia/Bangkok ".into()),
            ..Default::default()
        });
        assert_eq!(settings.zone_or(Some("Europe/Moscow")), "Asia/Bangkok");
        settings.apply(&UpdateAppSettingsInput {
            timezone: Some(String::new()),
            ..Default::default()
        });
        assert_eq!(settings.timezone, None);
        assert_eq!(settings.zone_or(Some("Europe/Moscow")), "Europe/Moscow");
        assert_eq!(settings.zone_or(Some("Nowhere/Land")), "UTC");
    }

    #[test]
    fn settings_round_trip_on_disk() {
        let dir = temp_dir("roundtrip");
        let settings = AppSettings {
            theme: Theme::Light,
            locale: Locale::Ru,
            base_currency: "eur".into(),
            base_currency_chosen: true,
            favorite_currencies: vec!["RUB".into()],
            timezone: Some("Asia/Bangkok".into()),
            inflation: default_inflation(),
            enrichment_opt_in: Some(true),
            quick_add_shortcut: Some("CmdOrCtrl+Shift+Space".into()),
            completed_tours: vec!["dashboard".into()],
            seen_whats_new: vec!["2026-10-round-13".into()],
            device_id: uuid::Uuid::new_v4().to_string(),
            device_name: Some("Laptop".into()),
            sync: SyncPrefs {
                enabled: true,
                confirm_first: true,
                prefer_main: false,
                background_lock_min: Some(15),
            },
            minimize_to_tray: Some(false),
            auto_update: Some(false),
            last_update_check: Some("2026-11-03T10:00:00Z".into()),
            fx_refresh_interval_sec: Some(900),
            display_decimals: Some(2),
            installed_at: Some("2026-10-19".into()),
            dashboard_today_collapsed: true,
            tour_offers: false,
            declined_tours: vec!["wallet".into()],
            pending_trusts: Vec::new(),
            autolock: Some(AutolockSettings {
                enabled: false,
                minutes: 30,
            }),
            pin_failures: BTreeMap::from([(
                "/v.wallet".to_string(),
                PinFailures {
                    count: 6,
                    last_failed_at_secs: 1_700_000_000,
                },
            )]),
        };
        save_settings(&dir, &settings).expect("save");
        assert_eq!(load_settings(&dir), settings);
        std::fs::remove_dir_all(&dir).ok();
    }

    fn autolock_of(enabled: bool, minutes: u32) -> AutolockSettings {
        AutolockSettings { enabled, minutes }
    }

    #[test]
    fn autolock_defaults_to_on_after_fifteen_minutes() {
        let settings = AppSettings::default();
        assert_eq!(settings.autolock(), autolock_of(true, 15));
        // A file from before the field reads as the default too.
        let mut raw = serde_json::to_value(&settings).expect("to json");
        raw.as_object_mut().expect("object").remove("autolock");
        let old: AppSettings = serde_json::from_value(raw).expect("old file");
        assert_eq!(old.autolock(), autolock_of(true, 15));
    }

    #[test]
    fn autolock_range_is_five_to_sixty_minutes() {
        for minutes in [0, 4, 61, 90] {
            assert!(validate_autolock(&autolock_of(true, minutes), false).is_err());
        }
        for minutes in [5, 15, 60] {
            assert_eq!(
                validate_autolock(&autolock_of(true, minutes), false).expect("in range"),
                autolock_of(true, minutes)
            );
        }
    }

    #[test]
    fn autolock_off_needs_a_device_without_keys() {
        assert!(validate_autolock(&autolock_of(false, 15), true).is_err());
        assert!(validate_autolock(&autolock_of(false, 15), false).is_ok());
        // The minutes are checked when the lock is off, too.
        assert!(validate_autolock(&autolock_of(false, 3), false).is_err());
        assert!(validate_autolock(&autolock_of(true, 15), true).is_ok());
    }

    #[test]
    fn first_load_stamps_the_install_day() {
        let dir = temp_dir("install-day");
        let settings = load_or_init_settings(&dir, "2026-10-19");
        assert_eq!(settings.installed_at.as_deref(), Some("2026-10-19"));
        // The second start keeps the first day.
        let again = load_or_init_settings(&dir, "2026-12-01");
        assert_eq!(again.installed_at.as_deref(), Some("2026-10-19"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn an_existing_file_without_install_day_reads_as_none() {
        let dir = temp_dir("install-day-old");
        save_settings(&dir, &AppSettings::default()).expect("save");
        let settings = load_or_init_settings(&dir, "2026-10-19");
        assert_eq!(settings.installed_at, None);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_settings_file_yields_defaults() {
        let dir = temp_dir("missing");
        assert_eq!(load_settings(&dir), AppSettings::default());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn corrupt_settings_file_yields_defaults() {
        let dir = temp_dir("corrupt");
        std::fs::write(settings_path(&dir), "{ not json").expect("write");
        assert_eq!(load_settings(&dir), AppSettings::default());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A corrupt file is not a first run: its bytes are kept next to it, and
    /// the install day is not stamped as if the app had just arrived.
    #[test]
    fn a_corrupt_settings_file_is_kept_aside_and_is_no_first_run() {
        let dir = temp_dir("corrupt-kept");
        let path = settings_path(&dir);
        std::fs::write(&path, "{ not json").expect("write");

        let settings = load_or_init_settings(&dir, "2026-11-10");
        assert_eq!(settings.installed_at, None);
        assert_eq!(
            std::fs::read_to_string(jsonstore::broken_path(&path)).expect("kept copy"),
            "{ not json"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn catalog_unions_and_sorts() {
        let catalog = SupportedCurrencies {
            fiat: vec!["usd".into(), "eur".into()],
            crypto: vec!["btc".into(), "eur".into()],
        };
        assert_eq!(catalog.all(), vec!["btc", "eur", "usd"]);
    }

    #[test]
    fn empty_catalog_is_not_offered() {
        assert!(SupportedCurrencies::default().is_empty());
        assert!(load_currencies(Path::new("/nonexistent-tradeapp-dir")).is_none());
    }

    #[test]
    fn catalog_round_trip_on_disk() {
        let dir = temp_dir("catalog");
        assert!(load_currencies(&dir).is_none());
        let catalog = SupportedCurrencies {
            fiat: vec!["USD".into(), " usd ".into()],
            crypto: vec!["BTC".into()],
        };
        save_currencies(&dir, &catalog).expect("save");
        let loaded = load_currencies(&dir).expect("cached catalog");
        assert_eq!(loaded.fiat, vec!["usd"]);
        assert_eq!(loaded.crypto, vec!["btc"]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn fallback_catalog_is_usable() {
        let catalog = SupportedCurrencies::fallback();
        assert!(!catalog.is_empty());
        assert!(catalog.all().contains(&DEFAULT_BASE_CURRENCY.to_string()));
    }

    #[test]
    fn unchanged_fetch_is_not_reported_as_offline() {
        let cached = SupportedCurrencies {
            fiat: vec!["usd".into()],
            crypto: vec!["btc".into()],
        };
        let (catalog, source, complete) = merge_catalog(
            Some(vec!["usd".into()]),
            Some(vec!["btc".into()]),
            Some(cached.clone()),
        );
        assert_eq!(catalog, cached.with_fallback());
        assert_eq!(source, CatalogSource::Network);
        assert_eq!(source.notice(), None);
        assert!(complete);
    }

    #[test]
    fn first_fetch_is_fresh_not_builtin() {
        let (_, source, complete) = merge_catalog(Some(vec!["eur".into()]), None, None);
        assert_eq!(source, CatalogSource::Network);
        // The crypto half failed: not worth caching.
        assert!(!complete);
    }

    #[test]
    fn fallback_catalog_has_no_repeats() {
        let catalog = SupportedCurrencies::fallback();
        let mut fiat = catalog.fiat.clone();
        fiat.dedup();
        assert_eq!(fiat, catalog.fiat);
        assert_eq!(catalog.fiat.iter().filter(|code| *code == "try").count(), 1);
    }

    #[test]
    fn network_catalog_always_contains_the_fallback_majors() {
        let (catalog, _, _) = merge_catalog(Some(vec!["thb".into()]), Some(Vec::new()), None);
        for code in ["btc", "usdt", "ton"] {
            assert!(catalog.crypto.contains(&code.to_string()), "{code}");
        }
        assert!(catalog.fiat.contains(&"thb".to_string()));
        assert!(catalog.fiat.contains(&"usd".to_string()));
    }

    #[test]
    fn a_half_empty_cache_is_not_fresh() {
        let half = SupportedCurrencies {
            fiat: vec!["usd".into()],
            crypto: Vec::new(),
        };
        assert!(!half.is_complete());
        let full = SupportedCurrencies {
            fiat: vec!["usd".into()],
            crypto: vec!["btc".into()],
        };
        assert!(full.is_complete());
    }

    #[test]
    fn failed_fetch_uses_cache_then_builtin() {
        let cached = SupportedCurrencies {
            fiat: vec!["usd".into()],
            crypto: Vec::new(),
        };
        let (catalog, source, _) = merge_catalog(None, None, Some(cached.clone()));
        assert_eq!(catalog, cached.with_fallback());
        assert_eq!(source.notice(), Some("cached"));

        let (catalog, source, _) = merge_catalog(None, None, None);
        assert_eq!(catalog, SupportedCurrencies::fallback());
        assert_eq!(source.notice(), Some("builtin"));
    }

    #[test]
    fn device_id_is_made_once() {
        let mut settings = AppSettings::default();
        assert!(settings.ensure_device_id());
        let id = settings.device_id.clone();
        assert!(!settings.ensure_device_id());
        assert_eq!(settings.device_id, id);
    }

    #[test]
    fn device_settings_are_taken_from_the_vault_once() {
        let vault = crate::vault::Settings {
            minimize_to_tray: false,
            fx_refresh_interval_sec: 900,
            display_decimals: 4,
            ..Default::default()
        };
        let mut settings = AppSettings {
            display_decimals: Some(2),
            ..Default::default()
        };
        assert!(settings.adopt_device_settings(&vault));
        assert!(!settings.minimize_to_tray());
        assert_eq!(settings.fx_refresh_interval(), 900);
        // Set here already: the device's own value stays.
        assert_eq!(settings.display_decimals, Some(2));
        assert!(!settings.adopt_device_settings(&crate::vault::Settings::default()));
        assert!(!settings.minimize_to_tray());
    }

    #[test]
    fn old_settings_file_reads_without_device_fields() {
        let dir = temp_dir("old");
        std::fs::write(
            settings_path(&dir),
            r#"{"theme":"dark","locale":"ru","base_currency":"rub"}"#,
        )
        .expect("write");
        let settings = load_settings(&dir);
        assert_eq!(settings.theme, Theme::Dark);
        assert!(settings.device_id.is_empty());
        assert!(!settings.sync.enabled);
        assert!(settings.minimize_to_tray());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn old_settings_file_reads_without_update_fields() {
        let settings: AppSettings =
            serde_json::from_str(r#"{"theme":"auto","locale":"auto","base_currency":"usd"}"#)
                .expect("old file");
        assert_eq!(settings.auto_update, None);
        assert!(settings.auto_update());
        assert_eq!(settings.last_update_check, None);
    }

    #[test]
    fn tour_offers_are_on_for_a_file_from_before_the_field() {
        let settings: AppSettings =
            serde_json::from_str(r#"{"theme":"auto","locale":"auto","base_currency":"usd"}"#)
                .expect("old file");
        assert!(settings.tour_offers);
        assert!(settings.declined_tours.is_empty());
    }

    #[test]
    fn completed_tours_are_deduplicated_and_reset() {
        let mut settings = AppSettings::default();
        settings.apply(&UpdateAppSettingsInput {
            completed_tours: Some(vec![
                "dashboard".into(),
                " dashboard ".into(),
                "".into(),
                "wallet".into(),
            ]),
            ..Default::default()
        });
        assert_eq!(settings.completed_tours, vec!["dashboard", "wallet"]);
        settings.apply(&UpdateAppSettingsInput {
            completed_tours: Some(Vec::new()),
            ..Default::default()
        });
        assert!(settings.completed_tours.is_empty());
    }
}
