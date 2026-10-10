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

/// Settings file name inside the app config directory.
pub const SETTINGS_FILE: &str = "app_settings.json";
/// Cache of the currency catalog fetched from the public providers.
pub const CURRENCIES_FILE: &str = "currencies.json";

/// Portfolio base until the user picks another one.
pub const DEFAULT_BASE_CURRENCY: &str = "usd";

/// Last-resort fiat catalog, used only when the providers are unreachable *and*
/// no successful fetch was ever cached. Keeps the pickers usable offline.
pub const FALLBACK_FIAT: &[&str] = &[
    "usd", "eur", "rub", "cny", "gbp", "jpy", "chf", "cad", "aud", "inr", "brl", "krw", "try",
    "mxn", "sgd", "hkd", "nzd", "sek", "nok", "pln", "czk", "huf", "ron", "bgn", "try",
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
    /// Currencies offered on hover next to any price. Empty = no tooltip.
    #[serde(default)]
    pub favorite_currencies: Vec<String>,
    /// IANA zone days and months are cut in (plan §7.6); `None` = the OS zone.
    #[serde(default)]
    pub timezone: Option<String>,
    /// Annual inflation, percent, by lowercase currency code (plan §6.5).
    /// Used only by analytics, for «in today's money»; marked as reference.
    #[serde(default = "default_inflation")]
    pub inflation: BTreeMap<String, Amount>,
    /// Consent to send transaction hashes to public explorers (plan §8.2);
    /// `None` = not asked yet.
    #[serde(default)]
    pub enrichment_opt_in: Option<bool>,
}

/// Reference annual inflation the user starts with, percent. Rough recent
/// figures; the settings screen says so and lets the user change them.
pub fn default_inflation() -> BTreeMap<String, Amount> {
    [("rub", "8"), ("usd", "3"), ("eur", "2.5"), ("thb", "1")]
        .into_iter()
        .filter_map(|(code, rate)| Some((code.to_string(), Amount::parse(rate).ok()?)))
        .collect()
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: Theme::Auto,
            locale: Locale::Auto,
            base_currency: DEFAULT_BASE_CURRENCY.into(),
            favorite_currencies: Vec::new(),
            timezone: None,
            inflation: default_inflation(),
            enrichment_opt_in: None,
        }
    }
}

impl AppSettings {
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
        }
        if let Some(favorites) = &patch.favorite_currencies {
            self.favorite_currencies = normalize_codes(favorites);
        }
        if let Some(zone) = &patch.timezone {
            self.timezone = normalize_zone(zone);
        }
        if let Some(rates) = &patch.inflation {
            self.inflation = normalize_inflation(rates);
        }
        if let Some(consent) = patch.enrichment_opt_in {
            self.enrichment_opt_in = Some(consent);
        }
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
fn normalize_inflation(raw: &BTreeMap<String, Amount>) -> BTreeMap<String, Amount> {
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

/// Uppercase, keep three-letter ISO codes only, drop the base currency itself:
/// a favourite equal to the base would show the same number twice.
fn normalize_codes(raw: &[String]) -> Vec<String> {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for code in raw {
        let code = code.trim().to_uppercase();
        if code.len() == 3 && code.chars().all(|ch| ch.is_ascii_alphabetic()) {
            seen.insert(code);
        }
    }
    seen.into_iter().collect()
}

/// Partial update: only the fields present in the payload are touched.
#[derive(Default, Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
pub struct UpdateAppSettingsInput {
    pub theme: Option<Theme>,
    pub locale: Option<Locale>,
    pub base_currency: Option<String>,
    pub favorite_currencies: Option<Vec<String>>,
    /// IANA zone; an empty string goes back to the OS zone.
    #[serde(default)]
    pub timezone: Option<String>,
    /// Replaces the whole inflation table.
    #[serde(default)]
    pub inflation: Option<BTreeMap<String, Amount>>,
    #[serde(default)]
    pub enrichment_opt_in: Option<bool>,
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
        Self {
            fiat: FALLBACK_FIAT
                .iter()
                .map(|code| (*code).to_string())
                .collect(),
            crypto: FALLBACK_CRYPTO
                .iter()
                .map(|code| (*code).to_string())
                .collect(),
        }
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
/// unreadable. A corrupt file must never keep the app from starting.
pub fn load_settings(config_dir: &Path) -> AppSettings {
    let path = settings_path(config_dir);
    let mut settings: AppSettings = std::fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default();
    settings.normalize();
    settings
}

/// Write the settings, creating the config directory when needed.
pub fn save_settings(config_dir: &Path, settings: &AppSettings) -> Result<()> {
    write_json(&settings_path(config_dir), settings)
}

/// Read the last successfully fetched catalog, if there is one.
pub fn load_currencies(config_dir: &Path) -> Option<SupportedCurrencies> {
    let path = currencies_path(config_dir);
    let mut catalog: SupportedCurrencies = std::fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())?;
    catalog.normalize();
    (!catalog.is_empty()).then_some(catalog)
}

/// Cache the catalog so the next offline start still offers real currencies.
pub fn save_currencies(config_dir: &Path, catalog: &SupportedCurrencies) -> Result<()> {
    write_json(&currencies_path(config_dir), catalog)
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
) -> SupportedCurrencies {
    let cached = load_currencies(config_dir);
    let fiat = match pricing.fx_rates().await {
        Ok(rates) => {
            let mut codes: Vec<String> = rates.into_keys().collect();
            codes.sort();
            Some(codes)
        }
        Err(_) => None,
    };
    let crypto = pricing.coin_symbols().await.ok();
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
        return SupportedCurrencies::fallback();
    }
    // Only a catalog worth remembering is written back; caching a half-empty
    // result would freeze the gap in place on every later offline start.
    if fetched_something {
        let _ = save_currencies(config_dir, &catalog);
    }
    catalog
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| Error::Io(format!("cannot create {}: {err}", parent.display())))?;
    }
    let body =
        serde_json::to_string_pretty(value).map_err(|err| Error::Serialize(err.to_string()))?;
    std::fs::write(path, body)
        .map_err(|err| Error::Io(format!("cannot write {}: {err}", path.display())))
}

#[cfg(test)]
mod tests {
    use super::*;

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
                "us".into(),
                "123".into(),
                String::new(),
            ]),
            ..Default::default()
        });
        assert_eq!(settings.base(), "rub");
        // Uppercase, three letters only, deduplicated.
        assert_eq!(settings.favorite_currencies, vec!["EUR", "RUB"]);
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
            favorite_currencies: vec!["usd".into(), "nope".into(), "gbp".into()],
            ..Default::default()
        };
        settings.normalize();
        assert_eq!(settings.base_currency, DEFAULT_BASE_CURRENCY);
        assert_eq!(settings.favorite_currencies, vec!["GBP", "USD"]);
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
            favorite_currencies: vec!["RUB".into()],
            timezone: Some("Asia/Bangkok".into()),
            inflation: default_inflation(),
            enrichment_opt_in: Some(true),
        };
        save_settings(&dir, &settings).expect("save");
        assert_eq!(load_settings(&dir), settings);
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
}
