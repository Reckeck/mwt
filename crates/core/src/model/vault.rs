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
pub struct DashboardWidgetCfg {
    pub id: String,
    pub visible: bool,
    pub order: u32,
}

impl Default for DashboardWidgetCfg {
    fn default() -> Self {
        Self {
            id: "total".into(),
            visible: true,
            order: 0,
        }
    }
}

fn default_fx_interval() -> u32 {
    300
}

fn default_minimize_to_tray() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct Settings {
    /// ISO currency code used as the portfolio base (e.g. `usd`, `rub`, `eur`).
    pub base_currency: String,
    pub theme: Theme,
    pub locale: Locale,
    /// Decimal places used to render balances.
    pub display_decimals: u32,
    /// Список валют для быстрого отображения (ISO 4217)
    #[serde(default)]
    pub favorite_currencies: Vec<String>,
    #[serde(default)]
    pub dashboard_widgets: Vec<DashboardWidgetCfg>,
    #[serde(default = "default_fx_interval")]
    pub fx_refresh_interval_sec: u32,
    #[serde(default = "default_minimize_to_tray")]
    pub minimize_to_tray: bool,
    /// Background Bybit import interval in seconds. 0 = disabled. Default 900 (15 min).
    #[serde(default = "default_bybit_import_interval")]
    pub bybit_import_interval_sec: u32,
}

fn default_bybit_import_interval() -> u32 {
    900
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            base_currency: "usd".into(),
            theme: Theme::Auto,
            locale: Locale::Auto,
            display_decimals: 8,
            favorite_currencies: vec![],
            dashboard_widgets: vec![],
            fx_refresh_interval_sec: 300,
            minimize_to_tray: true,
            bybit_import_interval_sec: 900,
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
    pub store_id: String,
    pub api_key: String,
    pub api_secret: String,
}

impl ExchangeKeys {
    /// Short, non-secret identifier safe to show in the UI (e.g. `abcd…wxyz`).
    pub fn hint(&self) -> String {
        let api_key = self.api_key.as_str();
        if api_key.len() <= 8 {
            return "*".repeat(api_key.len());
        }
        let head: String = api_key.chars().take(4).collect();
        let tail: String = api_key.chars().skip(api_key.chars().count() - 4).collect();
        format!("{head}…{tail}")
    }
}

impl std::fmt::Debug for ExchangeKeys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExchangeKeys")
            .field("store_id", &self.store_id)
            .field("api_key", &self.hint())
            .field("api_secret", &"<redacted>")
            .finish()
    }
}

impl Drop for ExchangeKeys {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.store_id.zeroize();
        self.api_key.zeroize();
        self.api_secret.zeroize();
    }
}

/// A staged Bybit fill awaiting user confirmation before becoming a trade.
///
/// Fills land here first and never touch `transactions` until the user
/// confirms them, so a background import can never silently rewrite history.
/// Deduplication key is Bybit's `execId`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub struct PendingImport {
    /// Bybit's execution ID (`execId`) — the deduplication key.
    pub exec_id: String,
    /// Store the fill belongs to.
    pub store_id: String,
    /// Product category: `spot` | `linear` | `inverse`.
    pub category: String,
    /// Pair symbol as Bybit reports it (e.g. `BTCUSDT`).
    pub symbol: String,
    /// `"buy"` or `"sell"`.
    pub side: String,
    /// Asset received.
    pub asset_bought: String,
    pub qty_bought: Amount,
    /// Asset given up.
    pub asset_sold: String,
    pub qty_sold: Amount,
    /// Fill price of one unit of the base asset.
    pub price: Amount,
    pub fee: Amount,
    /// Fee denom; empty when Bybit reported none.
    pub fee_asset: String,
    /// ISO-8601 UTC.
    pub occurred_at: String,
    /// Provenance note shown in the staging UI.
    pub comment: String,
}

impl PendingImport {
    /// Shape a Bybit fill for staging. Returns `None` for fills we cannot make
    /// sense of (no execution id, or an unparseable quantity/price) — a garbled
    /// fill must never reach the user's transactions.
    pub fn from_execution(
        store_id: &str,
        category: &str,
        exec: &crate::exchange::Execution,
    ) -> Option<Self> {
        let exec_id = exec.exec_id.trim();
        if exec_id.is_empty() {
            return None;
        }
        let prefill = exec.to_prefill();
        // Both legs must be fully known: a trade with an unknown quote leg
        // cannot be turned into balanced holdings.
        let asset_bought = nonempty(prefill.asset_bought?);
        let asset_sold = nonempty(prefill.asset_sold?);
        let qty_bought = Amount::parse(&nonempty(prefill.qty_bought?)).ok()?;
        let qty_sold = Amount::parse(&nonempty(prefill.qty_sold?)).ok()?;
        let price = Amount::parse(&nonempty(prefill.price?)).ok()?;
        if !qty_bought.is_positive() || !qty_sold.is_positive() || !price.is_positive() {
            return None;
        }
        Some(Self {
            exec_id: exec_id.to_string(),
            store_id: store_id.to_string(),
            category: category.to_string(),
            symbol: nonempty(prefill.symbol),
            side: nonempty(prefill.side),
            asset_bought,
            qty_bought,
            asset_sold,
            qty_sold,
            price,
            fee: prefill
                .fee
                .as_deref()
                .and_then(|amount| Amount::parse(amount).ok())
                .unwrap_or(Amount::ZERO),
            fee_asset: prefill.fee_asset.map(nonempty).unwrap_or_default(),
            occurred_at: nonempty(prefill.occurred_at.unwrap_or_default()),
            comment: prefill.comment.unwrap_or_default(),
        })
    }

    /// Assets referenced by this fill, for the staging UI summary.
    pub fn symbols(&self) -> [String; 2] {
        [self.asset_sold.clone(), self.asset_bought.clone()]
    }
}

fn nonempty(raw: String) -> String {
    raw.trim().to_string()
}

/// Exchange credentials grouped by exchange name.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Exchanges {
    #[serde(default)]
    pub bybit: Vec<ExchangeKeys>,
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
    /// Exchange credentials, added in payload v1.
    #[serde(default)]
    #[ts(skip)]
    pub exchanges: Exchanges,
    /// Staged Bybit fills awaiting user confirmation (D2.4).
    #[serde(default)]
    pub pending_imports: Vec<PendingImport>,
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
            exchanges: Exchanges::default(),
            pending_imports: Vec::new(),
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
            .map(|chain| chain.info())
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
        .map(|parsed| (parsed.unix_timestamp_nanos() / 1_000_000) as i64)
        .unwrap_or(0)
}

/// Unix timestamp (seconds) to ISO-8601 (RFC3339) UTC.
/// Returns an empty string for negative/absurd input rather than panicking.
pub fn unix_to_iso(secs: i64) -> String {
    use time::format_description::well_known::Rfc3339;
    time::OffsetDateTime::from_unix_timestamp(secs)
        .ok()
        .and_then(|stamp| stamp.format(&Rfc3339).ok())
        .unwrap_or_default()
}
