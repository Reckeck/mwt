use serde::{Deserialize, Serialize};

use crate::Amount;
use crate::model::chain::ChainInfo;
use crate::model::store::{AssetRef, Holding, Store};
use crate::model::transaction::Transaction;
use crate::sim::{SimRun, Strategy};

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct Settings {
    /// ISO currency code used as the portfolio base (e.g. `usd`, `rub`, `eur`).
    pub base_currency: String,
    pub theme: Theme,
    pub locale: Locale,
    /// Decimal places used to render balances.
    pub display_decimals: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            base_currency: "usd".into(),
            theme: Theme::Auto,
            locale: Locale::Auto,
            display_decimals: 8,
        }
    }
}

/// A cached price: one unit of an asset in USD.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct PricePoint {
    /// `AssetRef::key()`.
    pub asset_key: String,
    pub value_usd: Amount,
    /// ISO-8601 UTC.
    pub ts: String,
    pub source: String,
}

/// A cached FX rate: units of `currency` per 1 USD.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct FxPoint {
    pub currency: String,
    pub per_usd: Amount,
    pub ts: String,
    pub source: String,
}

/// Simulation state kept inside the vault (paper accounts & runs).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct SimState {
    pub strategies: Vec<Strategy>,
    pub runs: Vec<SimRun>,
}

/// Read-only exchange API credentials.
///
/// This is the only secret stored in the vault, so it gets extra care:
/// - `Debug` is hand-written to redact the secret and mask the key, so it can
///   never leak through a `{:?}` in a log or an error message.
/// - both fields are zeroized when the value is dropped.
/// - `ts(skip)` on the `VaultDoc` field keeps it out of the generated
///   TypeScript bindings entirely.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExchangeKeys {
    pub api_key: String,
    pub api_secret: String,
}

impl ExchangeKeys {
    /// Short, non-secret identifier safe to show in the UI (e.g. `abcd…wxyz`).
    pub fn hint(&self) -> String {
        let k = self.api_key.as_str();
        if k.len() <= 8 {
            return "*".repeat(k.len());
        }
        let head: String = k.chars().take(4).collect();
        let tail: String = k.chars().skip(k.chars().count() - 4).collect();
        format!("{head}…{tail}")
    }
}

impl std::fmt::Debug for ExchangeKeys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExchangeKeys")
            .field("api_key", &self.hint())
            .field("api_secret", &"<redacted>")
            .finish()
    }
}

impl Drop for ExchangeKeys {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.api_key.zeroize();
        self.api_secret.zeroize();
    }
}

/// Full plaintext vault document. Serialized as JSON (dev) or bincode (prod).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct VaultDoc {
    pub version: u32,
    pub id: String,
    pub created_at: String,
    pub settings: Settings,
    pub stores: Vec<Store>,
    pub holdings: Vec<Holding>,
    pub transactions: Vec<Transaction>,
    pub categories: Vec<String>,
    pub price_cache: Vec<PricePoint>,
    pub fx_cache: Vec<FxPoint>,
    #[ts(flatten)]
    pub sim: SimState,
    /// Exchange credentials, added in payload v2.
    ///
    /// MUST stay the last field: `storage::decode` relies on every v2 field
    /// being appended after the frozen v1 prefix so that a v1 snapshot still
    /// decodes field-for-field.
    #[serde(default)]
    #[ts(skip)]
    pub bybit: Option<ExchangeKeys>,
}

impl Default for VaultDoc {
    fn default() -> Self {
        Self {
            version: crate::VAULT_FORMAT_VERSION,
            id: uuid::Uuid::new_v4().to_string(),
            created_at: now_iso(),
            settings: Settings::default(),
            stores: Vec::new(),
            holdings: Vec::new(),
            transactions: Vec::new(),
            categories: vec![
                "salary".into(),
                "rewards".into(),
                "trading".into(),
                "staking".into(),
                "defi".into(),
                "other".into(),
            ],
            price_cache: Vec::new(),
            fx_cache: Vec::new(),
            sim: SimState::default(),
            bybit: None,
        }
    }
}

impl VaultDoc {
    /// ISO-8601 UTC now (RFC3339, lexicographically sortable).
    pub fn timestamp() -> String {
        now_iso()
    }

    pub fn fiat_asset(symbol: &str, name: &str) -> AssetRef {
        AssetRef::fiat(symbol, name)
    }

    pub fn chain_list() -> Vec<ChainInfo> {
        crate::model::chain::ALL_CHAINS
            .iter()
            .map(|c| c.info())
            .collect()
    }
}

/// ISO-8601 (RFC3339) UTC timestamp, e.g. `2026-09-28T21:47:00.123Z`.
pub fn now_iso() -> String {
    use time::format_description::well_known::Rfc3339;
    let now = time::OffsetDateTime::now_utc();
    now.format(&Rfc3339).unwrap_or_default()
}

pub fn iso_to_millis(iso: &str) -> i64 {
    use time::format_description::well_known::Rfc3339;
    time::OffsetDateTime::parse(iso, &Rfc3339)
        .map(|d| (d.unix_timestamp_nanos() / 1_000_000) as i64)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Frozen v1 shapes
//
// These exist ONLY to read payload-v1 snapshots. They must never be extended,
// reordered or removed without a matching bump of `VAULT_FORMAT_VERSION`.
//
// The bincode encoding is positional, so a v1 payload decodes into these types
// byte-for-byte. `VaultDocV1`'s field list must stay an exact prefix of
// `VaultDoc`'s — that is precisely what lets `storage::decode` read a v2
// payload as `VaultDocV1` and notice the leftover bytes.
// ---------------------------------------------------------------------------

/// Frozen v1 shape of [`Settings`].
#[derive(Serialize, Deserialize)]
pub struct SettingsV1 {
    pub base_currency: String,
    pub theme: Theme,
    pub locale: Locale,
    pub display_decimals: u32,
}

/// Frozen v1 shape of [`VaultDoc`].
///
/// `Serialize` exists purely so tests can synthesise genuine legacy payloads;
/// nothing in production ever writes one.
#[derive(Serialize, Deserialize)]
pub struct VaultDocV1 {
    pub version: u32,
    pub id: String,
    pub created_at: String,
    pub settings: SettingsV1,
    pub stores: Vec<Store>,
    pub holdings: Vec<Holding>,
    pub transactions: Vec<Transaction>,
    pub categories: Vec<String>,
    pub price_cache: Vec<PricePoint>,
    pub fx_cache: Vec<FxPoint>,
    pub sim: SimState,
}

impl VaultDocV1 {
    /// Lift a v1 document to the current schema.
    ///
    /// v1 -> v2 only appended an optional field, so there is nothing to
    /// restructure: every v1 value carries over untouched and `bybit` starts
    /// out absent.
    pub fn upgrade(self) -> VaultDoc {
        VaultDoc {
            version: crate::VAULT_FORMAT_VERSION,
            id: self.id,
            created_at: self.created_at,
            settings: Settings {
                base_currency: self.settings.base_currency,
                theme: self.settings.theme,
                locale: self.settings.locale,
                display_decimals: self.settings.display_decimals,
            },
            stores: self.stores,
            holdings: self.holdings,
            transactions: self.transactions,
            categories: self.categories,
            price_cache: self.price_cache,
            fx_cache: self.fx_cache,
            sim: self.sim,
            bybit: None,
        }
    }
}

/// Unix timestamp (seconds) to ISO-8601 (RFC3339) UTC.
/// Returns an empty string for negative/absurd input rather than panicking.
pub fn unix_to_iso(secs: i64) -> String {
    use time::format_description::well_known::Rfc3339;
    time::OffsetDateTime::from_unix_timestamp(secs)
        .ok()
        .and_then(|d| d.format(&Rfc3339).ok())
        .unwrap_or_default()
}
